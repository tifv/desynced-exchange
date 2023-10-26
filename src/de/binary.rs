use std::marker::PhantomData;

use serde::{
    de::{self, Visitor as Vi},
    Deserialize as De,
    Deserializer as Der, };

use crate::{
    string::Str,
    table::{ self,
        Key, Item, InsertItem,
        Table, DeserializeMode as TableMode,
    },
};
use super::Error;

impl From<std::str::Utf8Error> for Error {
    fn from(value: std::str::Utf8Error) -> Self {
        Self::from(Str::from(value.to_string()))
    }
}

struct Reader<'data> {
    start: *const u8,
    end: *const u8,
    lifetime: PhantomData<&'data [u8]>
}

#[allow(clippy::undocumented_unsafe_blocks)]
#[allow(clippy::multiple_unsafe_ops_per_block)]
impl<'data> Reader<'data> {
    fn new(slice: &'data [u8]) -> Self {
        let std::ops::Range{start, end} = slice.as_ptr_range();
        Self{start, end, lifetime: PhantomData}
    }
    fn peek_byte(&self) -> Option<u8> {
        unsafe {
            let offset = self.end.offset_from(self.start);
            if offset < 0 { std::hint::unreachable_unchecked(); }
            if offset == 0 {
                return None;
            }
            Some(*self.start)
        }
    }
    fn next_byte(&mut self) -> Option<u8> {
        unsafe {
            let offset = self.end.offset_from(self.start);
            if offset < 0 { std::hint::unreachable_unchecked(); }
            if offset == 0 {
                return None;
            }
            let next = *self.start;
            self.start = self.start.add(1);
            Some(next)
        }
    }
    fn next_slice(&mut self, len: usize) -> Option<&'data [u8]> {
        unsafe {
            let offset = self.end.offset_from(self.start);
            if offset < 0 { std::hint::unreachable_unchecked(); }
            if (offset as usize) < len {
                return None;
            }
            let slice = std::slice::from_raw_parts(self.start, len);
            self.start = self.start.add(len);
            Some(slice)
        }
    }
    fn next_array<const N: usize>(&mut self) -> Option<[u8; N]> {
        unsafe {
            let offset = self.end.offset_from(self.start);
            if offset < 0 { std::hint::unreachable_unchecked(); }
            if (offset as usize) < N {
                return None;
            }
            let array: *const [u8; N] = self.start.cast();
            self.start = self.start.add(N);
            Some(*array)
        }
    }
}

pub(super) struct Deserializer<'de>
{
    source: Reader<'de>,
}

impl<'de> Deserializer<'de> {
    fn error_unexpected() -> Error {
        Error::from("unexpected byte")
    }
    fn error_unexpected_end() -> Error {
        Error::from("unexpected end of data")
    }
    fn peek_byte(&mut self) -> Result<u8, Error> {
        self.source.peek_byte()
            .ok_or_else(Deserializer::error_unexpected_end)
    }
    fn next_byte(&mut self) -> Result<u8, Error> {
        self.source.next_byte()
            .ok_or_else(Deserializer::error_unexpected_end)
    }
    fn next_slice(&mut self, len: usize) -> Result<&'de [u8], Error> {
        self.source.next_slice(len)
            .ok_or_else(Deserializer::error_unexpected_end)
    }
    fn next_array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        self.source.next_array()
            .ok_or_else(Deserializer::error_unexpected_end)
    }
    fn deserialize_none<V: Vi<'de>>( &mut self,
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.next_byte()? {
            0xC0 => visitor.visit_none(),
            _ => Err(Deserializer::error_unexpected()),
        }
    }
}

impl<'s, 'de> Der<'de> for &'s mut Deserializer<'de>
where 'de : 's
{
    type Error = Error;

    fn deserialize_any<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.peek_byte()? {
            0xC0 =>
                self.deserialize_none(visitor),
            0xC2 | 0xC3 =>
                self.deserialize_bool(visitor),
            0xC5 => // dead key
                Err(Error::from("unexpected dead key marker")),
            0x00 ..= 0x7F | 0xE0 ..= 0xFF |
            0xCC | 0xCD | 0xCE |
            0xD0 | 0xD1 | 0xD2 =>
                self.deserialize_i32(visitor),
            0xCB =>
                self.deserialize_f64(visitor),
            0xA0 ..= 0xBF | 0xD9 | 0xDA =>
                self.deserialize_str(visitor),
            0x80 ..= 0x9F | 0xDC =>
                self.deserialize_map(visitor),
            _ => Err(Deserializer::error_unexpected()),
        }
    }

    fn deserialize_bool<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.next_byte()? {
            0xC2 => visitor.visit_bool(false),
            0xC3 => visitor.visit_bool(true),
            _ => Err(Deserializer::error_unexpected()),
        }
    }

    fn deserialize_i8<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_i32(visitor)
    }

    fn deserialize_i16<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_i32(visitor)
    }

    fn deserialize_i32<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        #![allow(clippy::cast_lossless)]
        match self.next_byte()? {
            value @ ( 0x00 ..= 0x7F | 0xE0 ..= 0xFF ) =>
                visitor.visit_i32(i8::from_le_bytes([value]) as i32 ),
            0xCC => visitor.visit_i32(
                u8::from_le_bytes(self.next_array::<1>()?) as i32 ),
            0xCD => visitor.visit_i32(
                u16::from_le_bytes(self.next_array::<2>()?) as i32 ),
            0xCE => visitor.visit_i32(
                i32::from_le_bytes(self.next_array::<4>()?) ),
            0xD0 => visitor.visit_i32(
                i8::from_le_bytes(self.next_array::<1>()?) as i32 ),
            0xD1 => visitor.visit_i32(
                i16::from_le_bytes(self.next_array::<2>()?) as i32 ),
            0xD2 => visitor.visit_i32(
                i32::from_le_bytes(self.next_array::<4>()?) ),
            _ => Err(Deserializer::error_unexpected()),
        }
    }

    fn deserialize_i64<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_i32(visitor)
    }

    fn deserialize_i128<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_i32(visitor)
    }

    fn deserialize_u8<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_i32(visitor)
    }

    fn deserialize_u16<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_i32(visitor)
    }

    fn deserialize_u32<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_i32(visitor)
    }

    fn deserialize_u64<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_i32(visitor)
    }

    fn deserialize_u128<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_i32(visitor)
    }

    fn deserialize_f32<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_f64(visitor)
    }

    fn deserialize_f64<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.next_byte()? {
            0xCB => visitor.visit_f64(
                f64::from_le_bytes(self.next_array::<8>()?) ),
            _ => Err(Deserializer::error_unexpected()),
        }
    }

    fn deserialize_char<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(Error::from("char is not supported"))
    }

    fn deserialize_str<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        let len = match self.next_byte()? {
            next @ 0xA0 ..= 0xBF => (next & 0x1F) as usize,
            0xD9 => u8::from_le_bytes(self.next_array::<1>()?) as usize,
            0xDA => u16::from_le_bytes(self.next_array::<2>()?) as usize,
            _ => return Err(Deserializer::error_unexpected()),
        };
        visitor.visit_borrowed_str(
            std::str::from_utf8(self.next_slice(len)?)? )
    }

    fn deserialize_string<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(Error::from("bytes is not supported"))
    }

    fn deserialize_byte_buf<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_unit<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_none(visitor)
    }

    fn deserialize_unit_struct<V: Vi<'de>>( self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_newtype_struct<V: Vi<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_tuple<V: Vi<'de>>( self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_tuple_struct<V: Vi<'de>>( self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(Error::from("tuple struct is not supported"))
    }

    fn deserialize_map<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        todo!()
    }

    fn deserialize_struct<V: Vi<'de>>( self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Vi<'de>>( self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        // visitor.visit_enum(todo!())
        todo!()
    }

    fn deserialize_identifier<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V: Vi<'de>>( self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_any(visitor)
    }

}
