use std::{collections::hash_map::{self, HashMap}, mem::MaybeUninit};

use serde::{ser, Serialize as Se, Serializer as Ser};

use crate::{
    string::Str,
    table::{ self,
        Key, Item, InsertItem,
        Table, SerializeMode as TableMode,
    },
};
use super::Error;

impl From<std::num::TryFromIntError> for Error {
    fn from(value: std::num::TryFromIntError) -> Self {
        Self::from(Str::from(value.to_string()))
    }
}

pub(super) struct Serializer {
    output: Vec<u8>,
}

impl Serializer {
    pub(super) fn new() -> Self {
        Self::from_buffer(Vec::new())
    }
    fn from_buffer(buffer: Vec<u8>) -> Self {
        Self{output: buffer}
    }
    pub(super) fn into_output(self) -> Vec<u8> {
        let Self{output} = self;
        output
    }
    #[inline]
    fn serialize_constant( &mut self,
        name: &'static str,
        error: impl Fn() -> Error,
    ) -> Result<(), Error> {
        match name {
            "Inf" | "Infinity" => self.serialize_i32(i32::MIN),
            "Signal" => self.serialize_i8(-4),
            "Visual" => self.serialize_i8(-3),
            "Store"  => self.serialize_i8(-2),
            "Goto"   => self.serialize_i8(-1),
            _ => Err(error())
        }
    }
    #[allow(clippy::unnecessary_wraps)]
    fn serialize_dead(&mut self) -> Result<(), Error> {
        self.output.push(0xC5);
        Ok(())
    }
}

impl<'s> Ser for &'s mut Serializer {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = TableSerializer<'s>;
    type SerializeTuple = Self::SerializeSeq;
    type SerializeTupleStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Self::Error>;

    type SerializeMap = TableSerializer<'s>;
    type SerializeStruct = Self::SerializeMap;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.output.push(0xC2 | u8::from(v));
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.into())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.into())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        match v {
            -0x20 ..= 0x3F => {
                self.output.push(v as u8);
            },
            0x40 ..= 0xFF => {
                self.output.push(0xCC);
                self.output.push(v as u8);
            },
            0x_0100 ..= 0x_FFFF => {
                self.output.push(0xCD);
                self.output.extend::<[u8;2]>((v as i16).to_le_bytes());
            },
            0x_0001_0000 ..= 0x_7FFF_FFFF => {
                self.output.push(0xCE);
                self.output.extend::<[u8;4]>(v.to_le_bytes());
            },
            -0x7F ..= -0x21 => {
                self.output.push(0xD0);
                self.output.push(v as u8);
            },
            -0x_7FFF ..= -0x_0080 => {
                self.output.push(0xD1);
                self.output.extend::<[u8;2]>((v as i16).to_le_bytes());
            },
            -0x_8000_0000 ..= -0x_0000_8000 => {
                self.output.push(0xD2);
                self.output.extend::<[u8;4]>(v.to_le_bytes());
            },
        }
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.into())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.into())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(v.into())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.output.push(0xCB);
        self.output.extend::<[u8;8]>(v.to_le_bytes());
        Ok(())
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("char is not supported"))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        match v.len() {
            0 ..= 0x1F => {
                self.output.push(0xA0 | (v.len() as u8));
                self.output.extend(v.as_bytes());
            },
            0x20 ..= 0xFF => {
                self.output.push(0xD9);
                self.output.push(v.len() as u8);
                self.output.extend(v.as_bytes());
            },
            0x_0100 ..= 0x_FFFF => {
                self.output.push(0xDA);
                self.output.extend::<[u8;2]>((v.len() as u16).to_le_bytes());
                self.output.extend(v.as_bytes());
            },
            _ => return Err(Error::from("too long string")),
        }
        Ok(())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("byte array is not supported"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.output.push(0xC0);
        Ok(())
    }

    fn serialize_some<T: Se + ?Sized>( self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    fn serialize_unit_struct( self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(name)
    }

    fn serialize_unit_variant( self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_constant( variant,
            || Error::from("unit variant is not supported"))
        // for map keys, a different choice is made
    }

    fn serialize_newtype_struct<T: Se + ?Sized>( self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
        // for map keys, a different choice is made
    }

    fn serialize_newtype_variant<T: Se + ?Sized>( self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("newtype variant is not supported"))
    }

    fn serialize_seq( self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(TableSerializer::new(self))
    }

    fn serialize_tuple( self,
        _len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(TableSerializer::new(self))
    }

    fn serialize_tuple_struct( self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::from("named tuple is not supported"))
    }

    fn serialize_tuple_variant( self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::from("tuple variant is not supported"))
    }

    fn serialize_map( self,
        _len: Option<usize>
    ) -> Result<Self::SerializeMap, Self::Error> {
        Ok(TableSerializer::new(self))
    }

    fn serialize_struct( self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(TableSerializer::new(self))
    }

    fn serialize_struct_variant( self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::from("struct variant is not supported"))
    }

    fn is_human_readable(&self) -> bool {
        false
    }

}

#[derive(Clone, PartialEq, Eq, Hash)]
enum InsertKey {
    Live(Key),
    Dead{position: u32},
}

impl InsertKey {
    fn index(index: i32) -> Self {
        Self::Live(Key::Index(index))
    }
    fn as_index(&self) -> Option<i32> {
        if let &Self::Live(Key::Index(index)) = self {
            Some(index)
        } else { None }
    }
    fn string(name: &str) -> Self {
        Self::Live(Key::Name(Str::from(name)))
    }
    fn name(name: &'static str) -> Self {
        Self::Live(Key::Name(Str::Name(name)))
    }
    fn dead(position: u32) -> Self {
        Self::Dead{position}
    }
    fn is_dead(&self) -> bool {
        matches!(self, Self::Dead{..})
    }
}

impl<V> InsertItem<V> {
    fn from_key_value(key: InsertKey, value: Option<V>) -> Self {
        match (key, value) {
            (InsertKey::Dead{position}, None) =>
                Self::Dead{position},
            (InsertKey::Live(key), Some(value)) =>
                Self::Live{key, value},
            _ => unreachable!(),
        }
    }
}

type SerializedValue = Box<[u8]>;

enum OctoItem<'a> {
    ArrayValue{value: &'a SerializedValue},
    AssocDead{link: i32},
    AssocItem{key: &'a Key, value: &'a SerializedValue, link: i32},
}

struct OctoInserter<'a> {
    ser: &'a mut Serializer,
    values: [Option<OctoItem<'a>>; OctoInserter::MASK_LEN],
    len: u8,
    mask: u8,
}

impl<'a> OctoInserter<'a> {
    const MASK_LEN: usize = u8::BITS as usize;
    fn new(ser: &'a mut Serializer) -> Self {
        Self{
            ser,
            values: [(); Self::MASK_LEN].map(|()| None),
            len: 0,
            mask: 0,
        }
    }
    fn push(&mut self, item: Option<OctoItem<'a>>) -> Result<(), Error> {
        assert!(self.len < Self::MASK_LEN as u8);
        if item.is_none() {
            self.mask |= 1 << self.len;
        }
        if std::mem::replace(
            &mut self.values[self.len as usize],
            item,
        ).is_some() {
            unreachable!();
        }
        self.len += 1;
        if self.len == Self::MASK_LEN as u8 {
            self.pop()?
        }
        Ok(())
    }
    fn encode_link(link: i32) -> Option<u8> {
        match link {
            0 => Some(0),
            1 ..= 0b_0011_1111 => Some((link << 2) as u8),
            -0b_0011_1111 ..= -1 => Some(((-link) << 2) as u8 + 0x02),
            _ => None,
        }
    }
    fn pop(&mut self) -> Result<(), Error> {
        assert!(self.len > 0);
        self.ser.output.push(self.mask);
        for item in &mut self.values[..self.len as usize] {
            let Some(item) = item.take() else { continue };
            let (key, value, link) = match item {
                OctoItem::ArrayValue{value} => {
                    self.ser.output.extend::<&[u8]>(value);
                    continue;
                },
                OctoItem::AssocDead{link} => (None, None, link),
                OctoItem::AssocItem{key, value, link} =>
                    (Some(key), Some(value), link),
            };
            if let Some(value) = value {
                self.ser.output.extend::<&[u8]>(value);
            } else {
                <&mut Serializer as Ser>::serialize_none(self.ser)?;
            }
            match key {
                // None => self.ser.serialize_dead()?,
                None => Serializer::serialize_dead(self.ser)?,
                Some(&Key::Index(index)) =>
                    <&mut Serializer as Ser>::serialize_i32(self.ser, index)?,
                Some( Key::Name(name)) =>
                    <&mut Serializer as Ser>::serialize_str(self.ser, name)?,
            }
            self.ser.output.push(match Self::encode_link(link) {
                Some(code) => code,
                None => return Err(Error::from("unsupported table size"))
            });
        }
        self.len = 0;
        self.mask = 0;
        Ok(())
    }
    fn end(mut self) -> Result<(), Error> {
        if self.len > 0 {
            self.pop()?
        }
        Ok(())
    }
}

pub(super) struct TableSerializer<'s> {
    ser: &'s mut Serializer,
    buffer: Vec<u8>,
    map: HashMap<InsertKey,Option<SerializedValue>>,
    max_cont_index: i32,
    next_key: Option<InsertKey>,
}

struct KeySerializer<'t, 's: 't> (
    &'t mut TableSerializer<'s>,
);

impl<'s> TableSerializer<'s> {
    fn new(ser: &'s mut Serializer) -> Self {
        Self{ ser,
            buffer: Vec::new(),
            map: HashMap::new(),
            max_cont_index: 0,
            next_key: None,
        }
    }
    fn inordered_error() -> Error {
        Error::from("key must be followed by a value")
    }
    fn insert_value<T: Se + ?Sized>( &mut self,
        key: InsertKey, value: &T,
    ) -> Result<(), Error> {
        if key.is_dead() {
            value.serialize(DeadValueSerializer)?;
            self.map.insert(key, None);
            return Ok(());
        }
        let mut value_ser = Serializer::from_buffer(
            std::mem::take(&mut self.buffer) );
        value.serialize(&mut value_ser)?;
        let mut output = value_ser.into_output();
        self.map.insert(key, Some(output[..].into()));
        output.clear();
        self.buffer = output;
        Ok(())
    }
}

impl<'s> ser::SerializeMap for TableSerializer<'s> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Se + ?Sized>( &mut self,
        key: &T,
    ) -> Result<(), Self::Error> {
        let key = key.serialize(KeySerializer(self))?;
        if let Some(index) = key.as_index() {
            if Some(index) == self.max_cont_index.checked_add(1) {
                self.max_cont_index = index;
            }
        }
        if self.next_key.replace(key).is_some() {
            return Err(Self::inordered_error());
        };
        Ok(())
    }

    fn serialize_value<T: Se + ?Sized>( &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        let Some(key) = self.next_key.take() else {
            return Err(Self::inordered_error());
        };
        self.insert_value(key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let Self{
            ser,
            mut map, max_cont_index,
            next_key, ..
        } = self;
        if next_key.is_some() {
            return Err(Self::inordered_error());
        }
        if max_cont_index < 0 {
            unreachable!();
        }
        let total_len = map.len();
        let max_possible_index = total_len.try_into()
            .unwrap_or(i32::MAX)
            .saturating_mul(2);
        let mut array = Vec::<Option<SerializedValue>>::with_capacity(
            max_cont_index.try_into().unwrap_or(0_usize) );
        let mut array_some_count: usize = 0;
        for index in 1 ..= max_possible_index {
            let Some(value) = map.remove(&InsertKey::index(index)) else {
                continue;
            };
            let Some(value) = value else {
                unreachable!("non-dead keys should not correspond to None");
            };
            let index = index as usize;
            if array.len() < index {
                array.resize_with(index, || None);
            }
            assert!(array[index - 1].is_none());
            array[index - 1] = Some(value);
            array_some_count += 1;
        }
        while array_some_count.saturating_mul(2) < array.len() {
            let Some(Some(value)) = array.pop() else {
                unreachable!("last element should exist and be Some")
            };
            array_some_count -= 1;
            map.insert(InsertKey::index((array.len() + 1) as i32), Some(value));
            while array.last().is_some_and(Option::is_none) {
                array.pop();
            }
        }
        let assoc_logsize = table::ilog2_ceil(map.len());
        let assoc = assoc_logsize.map(|logsize| {
            let mut assoc =
                Table::<SerializedValue, TableMode>::with_logsize(logsize);
            for (k, v) in map {
                assoc.insert(InsertItem::from_key_value(k, v));
            }
            assoc
        });
        match (array.len() as u32, assoc.as_ref().map(|t| (t.logsize(), t))) {
            (len @ 0 ..= 0xF, None) => {
                ser.output.push(0x90 | (len as u8));
            },
            (len @ 0x_0010 ..= 0x_FFFF, None) => {
                ser.output.push(0xDC);
                ser.output.extend((len as u16).to_le_bytes());
            },
            (0, Some((logsize @ 0 ..= 0x7, assoc))) => {
                ser.output.push(0x80 | ((logsize << 1) as u8));
                ser.output.push((assoc.get_last_free() << 1) as u8);
            },
            (len @ 0x01 ..= 0x7F, Some((logsize @ 0 ..= 0x5, assoc))) => {
                ser.output.push(0x81 | ((logsize << 1) as u8));
                ser.output.push((len << 1) as u8);
                ser.output.push((assoc.get_last_free() << 1) as u8);
            },
            (len @ 0x_0080 ..= 0x_3FFF, Some((logsize @ 0 ..= 0x5, assoc))) => {
                ser.output.push(0x81 | ((logsize << 1) as u8));
                ser.output.push(((len & ((1 <<  7) - (1 << 0))) << 1) as u8 + 1);
                ser.output.push(((len & ((1 << 14) - (1 << 7))) >> 6) as u8);
                ser.output.push((assoc.get_last_free() << 1) as u8);
            },
            (0x1_0000 ..= u32::MAX, None) |
            (0x4000 ..= u32::MAX, Some(_)) |
            (_, Some((0x6 ..= u16::MAX, _))) =>
                return Err(Error::from("unsupported table size")),
        }
        let mut inserter = OctoInserter::new(ser);
        for item in &array {
            inserter.push( item.as_ref()
                .map(|value| OctoItem::ArrayValue{value}) )?;
        }
        for item in assoc.iter().flatten() {
            inserter.push( match *item {
                Item::Free => None,
                Item::Dead{link} => Some(OctoItem::AssocDead{link}),
                Item::Live{ref value, ref key, link} =>
                    Some(OctoItem::AssocItem{value, key, link}),
            } )?;
        }
        inserter.end()?;
        Ok(())
    }

}

impl<'s> ser::SerializeStruct for TableSerializer<'s> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Se + ?Sized>( &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.insert_value(InsertKey::name(key), value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeMap::end(self)
    }
}

impl<'s> ser::SerializeSeq for TableSerializer<'s> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Se + ?Sized>( &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        let Some(index) = self.max_cont_index.checked_add(1) else {
            return Err(Error::from("array index overflow"));
        };
        self.max_cont_index = index;
        self.insert_value(InsertKey::index(index), value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeMap::end(self)
    }
}

impl<'s> ser::SerializeTuple for TableSerializer<'s> {
    type Ok = ();

    type Error = Error;

    fn serialize_element<T: Se + ?Sized>( &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<'t, 's: 't> Ser for KeySerializer<'t, 's> {
    type Ok = InsertKey;
    type Error = Error;

    type SerializeSeq = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("bool is not supported as a map key"))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.into())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.into())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(InsertKey::index(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.into())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.into())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v.try_into()?)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("f32 is not supported as a map key"))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("f64 is not supported as a map key"))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("char is not supported"))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(InsertKey::string(v))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("byte array is not supported"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("option is not supported as a map key"))
    }

    fn serialize_some<T: Se + ?Sized>( self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("option is not supported as a map key"))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("unit is not supported as a map key"))
    }

    fn serialize_unit_struct( self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(InsertKey::name(name))
    }

    fn serialize_unit_variant( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("unit variant is not supported as a map key"))
    }

    fn serialize_newtype_struct<T: Se + ?Sized>( self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        if name == "Dead" {
            return value.serialize(DeadKeySerializer).map(InsertKey::dead);
        }
        Err(Error::from("this newtype struct is not supported as a map key"))
    }

    fn serialize_newtype_variant<T: Se + ?Sized>( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Error::from("newtype variant is not supported"))
    }

    fn serialize_seq( self,
        len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::from("seq is not supported as a map key"))
    }

    fn serialize_tuple( self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Error::from("tuple is not supported as a map key"))
    }

    fn serialize_tuple_struct( self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::from("named tuple is not supported"))
    }

    fn serialize_tuple_variant( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::from("tuple variant is not supported"))
    }

    fn serialize_map( self,
        len: Option<usize>
    ) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::from("map is not supported as a map key"))
    }

    fn serialize_struct( self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::from("struct is not supported as a map key"))
    }

    fn serialize_struct_variant( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::from("struct variant is not supported"))
    }

    fn is_human_readable(&self) -> bool {
        false
    }

}

struct DeadKeySerializer;

impl DeadKeySerializer {
    fn error_invalid_type() -> Error {
        Error::from("Dead key can only be an str or u32")
    }
}

impl Ser for DeadKeySerializer {
    type Ok = u32;
    type Error = Error;

    type SerializeSeq = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Ok(v)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(crate::table::str_table_hash(v))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_some<T: Se + ?Sized>( self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_unit_struct( self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_unit_variant( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Se + ?Sized>( self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Se + ?Sized>( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_seq( self,
        len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_tuple( self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_tuple_struct( self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_tuple_variant( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_map( self,
        len: Option<usize>
    ) -> Result<Self::SerializeMap, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_struct( self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_struct_variant( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn is_human_readable(&self) -> bool {
        false
    }

}

struct DeadValueSerializer;

impl DeadValueSerializer {
    fn error_invalid_type() -> Error {
        Error::from("Dead value can only be none or unit")
    }
}

impl Ser for DeadValueSerializer {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_some<T: Se + ?Sized>( self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    fn serialize_unit_struct( self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_unit_variant( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Se + ?Sized>( self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Se + ?Sized>( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_seq( self,
        len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_tuple( self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_tuple_struct( self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_tuple_variant( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_map( self,
        len: Option<usize>
    ) -> Result<Self::SerializeMap, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_struct( self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn serialize_struct_variant( self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Self::error_invalid_type())
    }

    fn is_human_readable(&self) -> bool {
        false
    }

}

