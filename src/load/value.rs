use std::{io::Read, marker::PhantomData};

use crate::table::{TableItem, AssocItem, TableSize, iexp2};
use super::{
    Error,
    LoadKey, Load,
    Loader as LL, KeyLoader as KLL,
    LoadTableIterator,
};

pub(super) struct Loader<R: Read> {
    reader: R,
}

fn error_unexpected() -> Error {
    Error::from("unexpected byte")
}

impl<R: Read> Loader<R> {

    fn reborrow(&mut self) -> &mut Self {
        self
    }

    fn read_byte(&mut self) -> Result<u8, Error> {
        let mut byte = [0; 1];
        self.reader.read_exact(&mut byte)?;
        Ok(byte[0])
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let mut bytes = [0; N];
        self.reader.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn load_nil(&mut self, head: u8) -> Result<(), Error> {
        #![allow(clippy::unused_self)]
        match head {
            0xC0 => Ok(()),
            _ => Err(error_unexpected()),
        }
    }

    fn load_boolean(&mut self, head: u8) -> Result<bool, Error> {
        #![allow(clippy::unused_self)]
        match head {
            0xC2 => Ok(false),
            0xC3 => Ok(true),
            _ => Err(error_unexpected()),
        }
    }

    fn load_integer(&mut self, head: u8) -> Result<i32, Error> {
        #![allow(clippy::cast_lossless)]
        match head {
            value @ ( 0x00 ..= 0x7F | 0xE0 ..= 0xFF ) =>
                Ok(i8::from_le_bytes([value]) as i32),
            0xCC =>
                Ok(u8::from_le_bytes(self.read_array::<1>()?) as i32),
            0xCD =>
                Ok(u16::from_le_bytes(self.read_array::<2>()?) as i32),
            0xCE =>
                Ok(i32::from_le_bytes(self.read_array::<4>()?)),
            0xD0 =>
                Ok(i8::from_le_bytes(self.read_array::<1>()?) as i32),
            0xD1 =>
                Ok(i16::from_le_bytes(self.read_array::<2>()?) as i32),
            0xD2 =>
                Ok(i32::from_le_bytes(self.read_array::<4>()?)),
            _ => Err(error_unexpected()),
        }
    }

    fn load_float(&mut self, head: u8) -> Result<f64, Error> {
        match head {
            0xCB => Ok(f64::from_le_bytes(self.read_array::<8>()?)),
            _ => Err(error_unexpected()),
        }
    }

    fn load_string( &mut self,
        head: u8,
    ) -> Result<(u32, impl Read + '_), Error> {
        #![allow(clippy::cast_lossless)]
        let len = match head {
            head @ 0xA0 ..= 0xBF => (head & 0x1F) as u32,
            0xD9 => u8::from_le_bytes(self.read_array::<1>()?) as u32,
            0xDA => u16::from_le_bytes(self.read_array::<2>()?) as u32,
            _ => return Err(error_unexpected()),
        };
        Ok((len, self.reader.by_ref().take(len as u64)))
    }

    fn load_table_header( &mut self,
        head: u8,
    ) -> Result<(u32, Option<u16>, u32), Error> {
        #![allow(clippy::cast_lossless)]
        Ok(match head {
            head @ 0x80 ..= 0x8F => {
                let mut array_len = 0_u32;
                if head & 0x01 > 0 {
                    let mut k = 1_u32;
                    for i in [0, 1] {
                        let b = self.read_byte()?;
                        array_len += ((b >> 1) as u32) * k;
                        if b & 0x01 > 0 {
                            k <<= 7;
                        } else { break }
                    }
                }
                (
                    array_len,
                    Some(((head & 0x0F) >> 1) as u16),
                    self.read_byte()? as u32,
                )
            },
            head @ 0x90 ..= 0x9F =>
                ((head & 0x0F) as u32, None, 0),
            0xDC => (
                u16::from_le_bytes(self.read_array::<2>()?) as u32,
                None, 0
            ),
            _ => return Err(error_unexpected()),
        })
    }

}

impl<R: Read> LL for &mut Loader<R> {
    fn load_value<B: super::Builder>( self,
        builder: B,
    ) -> Result<B::Value, Error> {
        let head = self.read_byte()?;
        match head {
            0xC0 => {
                self.load_nil(head)?;
                builder.build_nil()
            },
            0xC2 | 0xC3 => builder.build_boolean(
                self.load_boolean(head)? ),
            0xC5 => // dead key
                Err(Error::from("unexpected dead key marker")),
            0x00 ..= 0x7F | 0xE0 ..= 0xFF |
            0xCC | 0xCD | 0xCE |
            0xD0 | 0xD1 | 0xD2 => builder.build_integer(
                self.load_integer(head)? ),
            0xCB => builder.build_float(
                self.load_float(head)? ),
            0xA0 ..= 0xBF | 0xD9 | 0xDA => {
                let (len, reader) = self.load_string(head)?;
                builder.build_string(len, reader)
            },
            0x80 ..= 0x8F | 0x90 ..= 0x9F | 0xDC => {
                let (array_len, assoc_loglen, assoc_last_free) =
                    self.load_table_header(head)?;
                builder.build_table(SerialReader::new(
                    self,
                    array_len,
                    assoc_loglen, assoc_last_free,
                ))
            },
            _ => Err(error_unexpected()),
        }
    }

}

impl<R: Read> KLL for &mut Loader<R> {
    fn load_key<B: super::KeyBuilder>( self,
        builder: B,
    ) -> Result<Option<B::Value>, Error> {
        let head = self.read_byte()?;
        match head {
            0xC5 => Ok(None),
            0x00 ..= 0x7F | 0xE0 ..= 0xFF |
            0xCC | 0xCD | 0xCE |
            0xD0 | 0xD1 | 0xD2 => builder.build_integer(
                self.load_integer(head)?
            ).map(Some),
            0xA0 ..= 0xBF | 0xD9 | 0xDA => {
                let (len, reader) = self.load_string(head)?;
                builder.build_string(len, reader).map(Some)
            },
            _ => Err(error_unexpected()),
        }
    }
}

struct SerialReader<'l, R: Read, K: LoadKey, V: Load> {
    loader: &'l mut Loader<R>,
    array_len: u32,
    assoc_loglen: Option<u16>,
    assoc_last_free: u32,
    assoc_len: u32,
    mask: u8, mask_len: u8,
    output: PhantomData<TableItem<K, V>>,
}

impl<'l, R, K, V> SerialReader<'l, R, K, V>
where R: Read, K: LoadKey, V: Load,
{
    fn new(
        loader: &'l mut Loader<R>,
        array_len: u32,
        assoc_loglen: Option<u16>, assoc_last_free: u32,
    ) -> Self {
        Self{
            loader,
            array_len,
            assoc_loglen, assoc_last_free,
            assoc_len: iexp2(assoc_loglen),
            mask: 0, mask_len: 0,
            output: PhantomData,
        }
    }
    #[inline]
    fn next_is_masked(&mut self) -> Result<bool, Error> {
        if self.mask_len == 0 {
            self.mask = self.loader.read_byte()?;
            self.mask_len = 8;
        }
        let is_masked = (self.mask & 0x01) > 0;
        self.mask >>= 1;
        self.mask_len -= 1;
        Ok(is_masked)
    }
    fn read_array_item(&mut self) -> Result<Option<TableItem<K, V>>, Error> {
        if self.next_is_masked()? {
            return Ok(None);
        }
        let value = V::load(self.loader.reborrow())?;
        Ok(Some(TableItem::Array(value)))
    }
    fn read_assoc_item(&mut self) -> Result<Option<TableItem<K, V>>, Error> {
        if self.next_is_masked()? {
            return Ok(None);
        }
        let value = V::load(self.loader.reborrow())?;
        let key = K::load_key(self.loader.reborrow())?;
        let link_code = self.loader.read_byte()?;
        if link_code & 0x01 > 0 {
            return Err(Error::from("unexpected link code"));
        }
        let mut link: i32 = (link_code >> 2).into();
        if link_code & 0x02 > 0 {
            link = -link;
        }
        if let Some(key) = key {
            Ok(Some(TableItem::Assoc(AssocItem::Live{ value, key, link })))
        } else {
            if !value.is_nil() {
                return Err(Error::from(
                    "empty key should correspond to nil value" ))
            }
            Ok(Some(TableItem::Assoc(AssocItem::Dead{link})))
        }
    }
}

impl<'l, R, K, V> TableSize for SerialReader<'l, R, K, V>
where R: Read, K: LoadKey, V: Load,
{
    fn array_len(&self) -> u32 {
        self.array_len
    }
    fn assoc_loglen(&self) -> Option<u16> {
        self.assoc_loglen
    }
    fn assoc_last_free(&self) -> u32 {
        self.assoc_last_free
    }
}

impl<'l, R, K, V> Iterator for SerialReader<'l, R, K, V>
where R: Read, K: LoadKey, V: Load,
{
    type Item = Result<Option<TableItem<K, V>>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.array_len > 0 {
            self.array_len -= 1;
            return Some(self.read_array_item());
        }
        if self.assoc_len > 0 {
            self.assoc_len -= 1;
            return Some(self.read_assoc_item());
        }
        None
    }
}

impl<'l, R, K, V> LoadTableIterator for SerialReader<'l, R, K, V>
where R: Read, K: LoadKey, V: Load,
{
    type Key = K;
    type Value = V;
}

