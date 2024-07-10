use std::marker::PhantomData;

use crate::{
    error::LoadError as Error,
    common::{
        u32_to_usize, LogSize, iexp2,
        byteseq::Read,
    },
    table_iter::{
        TableItem, AssocItem,
        TableSize,
    },
    load::{
        KeyLoad, Load,
        KeyBuilder, Builder,
        Loader as LoaderTr, TableLoader
    },
    Exchange
};

mod decompress;

pub fn load_blueprint<P, B, E>(exchange: &str)
-> Result<Exchange<Option<P>, Option<B>>, Error>
where P: Load, B: Load,
{
    let encoded_data = decompress::decompress(exchange)?;
    encoded_data.as_deref().map(decode, decode).transpose()
}

fn decode<V: Load>(data: &[u8]) -> Result<Option<V>, Error>
{
    V::load(&mut Loader::new(data))
}


struct Loader<R: Read<u8>> {
    reader: R,
    max_array_len: u32,
}

#[cold]
fn error_unexpected(byte: u8) -> Error {
    Error::from(format!("unexpected byte {byte:X}"))
}

#[cold]
fn error_eof() -> Error {
    Error::from("unexpected end of data")
}

#[cold]
fn error_bad_size() -> Error {
    Error::from(
        "Table size is too large to be correct" )
}

#[cold]
fn error_unsupported_size() -> Error {
    Error::from(
        "Table size is unsupported" )
}

struct TableHeader {
    array_len: u32,
    assoc_loglen: Option<LogSize>,
    assoc_last_free: u32,
}

impl TableHeader {
    fn array(array_len: u32) -> Self {
        Self {
            array_len,
            assoc_loglen: None,
            assoc_last_free: 0,
        }
    }
}

impl<R: Read<u8>> Loader<R> {

    #[must_use]
    fn new(reader: R) -> Self {
        // The most compact representation of an array element
        // is bitmask, which is eight (nil) elements per one byte.
        let max_array_len = u32::try_from(reader.len())
            .unwrap_or(u32::MAX)
            .saturating_mul(8);
        Self {
            reader,
            max_array_len,
        }
    }

    fn read_byte(&mut self) -> Result<u8, Error> {
        self.reader.read_byte()
            .ok_or_else(error_eof)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        self.reader.read_array()
            .ok_or_else(error_eof)
    }

    fn read_slice(&mut self, len: usize) -> Result<&[u8], Error> {
        self.reader.read_slice(len)
            .ok_or_else(error_eof)
    }

    fn read_ext_uint(&mut self) -> Result<u32, Error> {
        let mut value = 0;
        let mut shift = 0;
        loop {
            let mut next_shift = shift + 8;
            let mut byte = self.read_byte()?;
            let continued = (byte & 0x01) > 0;
            byte >>= 1; next_shift -= 1;
            if next_shift > 21 {
                return Err(Error::from("unexpectedly large index"));
            }
            value += u32::from(byte) << shift;
            if !continued {
                break;
            }
            shift = next_shift;
        }
        Ok(value)
    }

    fn read_ext_sint(&mut self) -> Result<i32, Error> {
        let mut value = 0;
        let mut negative = None;
        let mut shift = 0;
        loop {
            let mut next_shift = shift + 8;
            let mut byte = self.read_byte()?;
            let continued = (byte & 0x01) > 0;
            byte >>= 1; next_shift -= 1;
            if negative.is_none() {
                negative = Some(byte & 0x01 > 0);
                byte >>= 1; next_shift -= 1;
            }
            if next_shift > 20 {
                return Err(Error::from("unexpectedly large index"));
            }
            value += u32::from(byte) << shift;
            if !continued {
                break;
            }
            shift = next_shift;
        }
        let Some(negative) = negative else { unreachable!() };
        let value = value as i32;
        Ok(if !negative { value } else { -value })
    }

    fn load_nil(&mut self, head: u8) -> Result<(), Error> {
        #![allow(clippy::unused_self)]
        match head {
            0xC0 => Ok(()),
            _ => Err(error_unexpected(head)),
        }
    }

    fn load_boolean(&mut self, head: u8) -> Result<bool, Error> {
        #![allow(clippy::unused_self)]
        match head {
            0xC2 => Ok(false),
            0xC3 => Ok(true),
            _ => Err(error_unexpected(head)),
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
            _ => Err(error_unexpected(head)),
        }
    }

    fn load_float(&mut self, head: u8) -> Result<f64, Error> {
        match head {
            0xCB => Ok(f64::from_le_bytes(self.read_array::<8>()?)),
            _ => Err(error_unexpected(head)),
        }
    }

    fn load_string( &mut self,
        head: u8,
    ) -> Result<&str, Error> {
        #![allow(clippy::cast_lossless)]
        let len = match head {
            head @ 0xA0 ..= 0xBF => (head & 0x1F) as u32,
            0xD9 => u8::from_le_bytes(self.read_array::<1>()?) as u32,
            0xDA => u16::from_le_bytes(self.read_array::<2>()?) as u32,
            _ => return Err(error_unexpected(head)),
        };
        let len = u32_to_usize(len);
        Ok(std::str::from_utf8(self.read_slice(len)?)?)
    }

    fn load_table_header( &mut self,
        head: u8,
    ) -> Result<TableHeader, Error> {
        #![allow(clippy::cast_lossless)]
        Ok(match head {
            0x90 ..= 0x9F =>
                TableHeader::array((head & 0x0F) as u32),
            0xDC => TableHeader::array(
                u16::from_le_bytes(self.read_array::<2>()?) as u32,
            ),
            0xDD => TableHeader::array(
                u32::from_le_bytes(self.read_array::<4>()?),
            ),
            0x80 ..= 0x8F => {
                let has_array_part = head & 0x01 > 0;
                let array_len = if has_array_part {
                    self.read_ext_uint()?
                } else { 0_u32 };
                let assoc_loglen = Some((head & 0x0F) >> 1);
                let assoc_last_free = self.read_ext_uint()?;
                TableHeader {
                    array_len,
                    assoc_loglen,
                    assoc_last_free,
                }
            },
            0xDE => {
                let (has_array_part, assoc_loglen) = {
                    let byte = self.read_byte()?;
                    (byte & 0x01 > 0, Some(byte >> 1))
                };
                if let byte @ 0x01.. = self.read_byte()? {
                    return Err(error_unexpected(byte));
                };
                let array_len = if has_array_part {
                    self.read_ext_uint()?
                } else { 0_u32 };
                let assoc_last_free = self.read_ext_uint()?;
                TableHeader {
                    array_len,
                    assoc_loglen,
                    assoc_last_free,
                }
            },
            _ => return Err(error_unexpected(head)),
        })
    }

}

impl<R: Read<u8>> LoaderTr for &mut Loader<R> {
    type Error = Error;

    fn load_value<B>(self, builder: B)
    -> Result<Option<B::Output>, Error>
    where B: Builder
    {
        let head = self.read_byte()?;
        match head {
            0xC0 => {
                self.load_nil(head)?;
                builder.build_nil()
            },
            0xC2 | 0xC3 => builder.build_boolean(
                self.load_boolean(head)? ),
            0xC5 => Err(Error::from("unexpected dead key marker")),
            0x00 ..= 0x7F | 0xE0 ..= 0xFF |
            0xCC | 0xCD | 0xCE |
            0xD0 | 0xD1 | 0xD2 => builder.build_integer(
                self.load_integer(head)? ),
            0xCB => builder.build_float(
                self.load_float(head)? ),
            0xA0 ..= 0xBF | 0xD9 | 0xDA => {
                builder.build_string(self.load_string(head)?)
            },
            0x80 ..= 0x8F | 0x90 ..= 0x9F | 0xDC | 0xDE => {
                let TableHeader { array_len, assoc_loglen, assoc_last_free } =
                    self.load_table_header(head)?;
                self.max_array_len = match
                    self.max_array_len.checked_sub(array_len)
                {
                    None => return Err(error_bad_size()),
                    Some(rest) => rest,
                };
                if let Some(assoc_loglen) = assoc_loglen {
                    if assoc_loglen > crate::MAX_ASSOC_LOGLEN {
                        return Err(error_unsupported_size());
                    }
                    self.max_array_len = match
                        self.max_array_len.checked_sub(iexp2(Some(assoc_loglen)))
                    {
                        None => return Err(error_bad_size()),
                        Some(rest) => rest,
                    };
                }
                builder.build_table(SerialReader::new(
                    self,
                    array_len,
                    assoc_loglen, assoc_last_free,
                ))
            },
            _ => Err(error_unexpected(head)),
        }
    }

    fn load_key<KB>(self, builder: KB)
    -> Result<Option<KB::Output>, Error>
    where KB: KeyBuilder
    {
        let head = self.read_byte()?;
        match head {
            0xC5 => Ok(None),
            0x00 ..= 0x7F | 0xE0 ..= 0xFF |
            0xCC | 0xCD | 0xCE |
            0xD0 | 0xD1 | 0xD2 => Ok(Some(
                builder.build_integer::<Error>(self.load_integer(head)?)?
            )),
            0xA0 ..= 0xBF | 0xD9 | 0xDA => Ok(Some(
                builder.build_string::<Error>(self.load_string(head)?)?
            )),
            _ => Err(error_unexpected(head)),
        }
    }

}

struct SerialReader<'l, R, K, V>
where R: Read<u8>, K: KeyLoad, V: Load
{
    loader: &'l mut Loader<R>,
    array_len: u32,
    assoc_loglen: Option<LogSize>,
    assoc_last_free: u32,
    assoc_len: u32,
    mask: u8, mask_len: u8,
    output: PhantomData<TableItem<K, V>>,
}

impl<'l, R, K, V> SerialReader<'l, R, K, V>
where R: Read<u8>, K: KeyLoad, V: Load
{
    fn new(
        loader: &'l mut Loader<R>,
        array_len: u32,
        assoc_loglen: Option<LogSize>, assoc_last_free: u32,
    ) -> Self {
        Self {
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
        let value = V::load(&mut *self.loader)?;
        Ok(value.map(TableItem::Array))
    }
    fn read_assoc_item(&mut self) -> Result<Option<TableItem<K, V>>, Error> {
        if self.next_is_masked()? {
            return Ok(None);
        }
        let value = V::load(&mut *self.loader)?;
        let key = K::load_key(&mut *self.loader)?;
        let link = self.loader.read_ext_sint()?;
        if let Some(key) = key {
            Ok(Some(TableItem::Assoc(AssocItem::Live { value, key, link })))
        } else {
            if value.is_some() {
                return Err(Error::from(
                    "empty key should correspond to nil value" ))
            }
            Ok(Some(TableItem::Assoc(AssocItem::Dead { link })))
        }
    }
}

impl<'l, R, K, V> TableSize for SerialReader<'l, R, K, V>
where R: Read<u8>, K: KeyLoad, V: Load
{
    fn array_len(&self) -> u32 {
        self.array_len
    }
    fn assoc_loglen(&self) -> Option<LogSize> {
        self.assoc_loglen
    }
    fn assoc_last_free(&self) -> u32 {
        self.assoc_last_free
    }
}

impl<'l, R, K, V> Iterator for SerialReader<'l, R, K, V>
where R: Read<u8>, K: KeyLoad, V: Load
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

impl<'l, R, K, V> TableLoader for SerialReader<'l, R, K, V>
where R: Read<u8>, K: KeyLoad, V: Load
{
    type Key = K;
    type Value = V;
    type Error = Error;
}

