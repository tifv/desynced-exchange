use std::marker::PhantomData;

use crate::{
    error::LoadError as Error,
    common::{iexp2, u32_to_usize},
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

pub(crate) mod decompress;

mod reader;
use reader::Reader;

pub fn load_blueprint<P, B, E>(exchange: &str)
-> Result<Exchange<Option<P>, Option<B>>, Error>
where P: Load, B: Load,
{
    decode_blueprint(decompress::decompress(exchange)?)
}

pub(crate) fn decode_blueprint<P, B>(encoded_data: Exchange<Vec<u8>,Vec<u8>>)
-> Result<Exchange<Option<P>, Option<B>>, Error>
where P: Load, B: Load,
{
    Ok(match encoded_data {
        Exchange::Blueprint(encoded_body) =>
            Exchange::Blueprint(P::load(
                &mut Loader::new(&encoded_body)
            )?),
        Exchange::Behavior (encoded_body) =>
            Exchange::Behavior (B::load(
                &mut Loader::new(&encoded_body)
            )?),
    })
}

pub struct Loader<'data> {
    reader: Reader<'data>,
    max_array_len: u32,
}

#[cold]
fn error_unexpected() -> Error {
    Error::from("unexpected byte")
}

#[cold]
fn error_eof() -> Error {
    Error::from("unexpected end of data")
}

struct TableHeader {
    pub array_len: u32,
    pub assoc_loglen: Option<u16>,
    pub assoc_last_free: u32,
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

impl<'data> Loader<'data> {

    pub fn new(data: &'data [u8]) -> Self {
        // The most compact representation of an array element
        // is bitmask, which is eight (nil) elements per one byte.
        let reader = Reader::from_slice(data);
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

    fn read_slice(&mut self, len: usize) -> Result<&'data [u8], Error> {
        self.reader.read_slice(len)
            .ok_or_else(error_eof)
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
    ) -> Result<&'data str, Error> {
        #![allow(clippy::cast_lossless)]
        let len = match head {
            head @ 0xA0 ..= 0xBF => (head & 0x1F) as u32,
            0xD9 => u8::from_le_bytes(self.read_array::<1>()?) as u32,
            0xDA => u16::from_le_bytes(self.read_array::<2>()?) as u32,
            _ => return Err(error_unexpected()),
        };
        let len = u32_to_usize(len);
        Ok(std::str::from_utf8(self.read_slice(len)?)?)
    }

    fn load_table_header( &mut self,
        head: u8,
    ) -> Result<TableHeader, Error> {
        #![allow(clippy::cast_lossless)]
        Ok(match head {
            0x80 ..= 0x8F => {
                let mut array_len = 0_u32;
                if head & 0x01 > 0 {
                    let mut k = 1_u32;
                    for () in [(); 2] {
                        let b = self.read_byte()?;
                        array_len += ((b >> 1) as u32) * k;
                        if b & 0x01 > 0 {
                            k <<= 7;
                        } else { break }
                    }
                }
                let assoc_loglen = Some(((head & 0x0F) >> 1) as u16);
                let assoc_last_free = {
                    let last_free_code = self.read_byte()?;
                    if last_free_code & 0x01 > 0 {
                        return Err(Error::from(
                            "Unrecognized last free index format" ))
                    }
                    (last_free_code >> 1) as u32
                };
                TableHeader {
                    array_len,
                    assoc_loglen,
                    assoc_last_free,
                }
            },
            0x90 ..= 0x9F =>
                TableHeader::array((head & 0x0F) as u32),
            0xDC => TableHeader::array(
                u16::from_le_bytes(self.read_array::<2>()?) as u32,
            ),
            _ => return Err(error_unexpected()),
        })
    }

}

impl<'data> LoaderTr for &mut Loader<'data> {
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
            0xC5 => // dead key
                Err(Error::from("unexpected dead key marker")),
            0x00 ..= 0x7F | 0xE0 ..= 0xFF |
            0xCC | 0xCD | 0xCE |
            0xD0 | 0xD1 | 0xD2 => builder.build_integer(
                self.load_integer(head)? ),
            0xCB => builder.build_float(
                self.load_float(head)? ),
            0xA0 ..= 0xBF | 0xD9 | 0xDA => {
                builder.build_string(self.load_string(head)?)
            },
            0x80 ..= 0x8F | 0x90 ..= 0x9F | 0xDC => {
                let TableHeader { array_len, assoc_loglen, assoc_last_free } =
                    self.load_table_header(head)?;
                if assoc_loglen.is_some_and(|x| x > crate::MAX_ASSOC_LOGLEN) {
                    return Err(Error::from(
                        "Encoded table size is too large" ));
                }
                self.max_array_len = match
                    self.max_array_len.checked_sub(array_len)
                {
                    None => return Err(Error::from(
                        "Encoded table size is too large to be correct" )),
                    Some(rest) => rest,
                };
                builder.build_table(SerialReader::new(
                    self,
                    array_len,
                    assoc_loglen, assoc_last_free,
                ))
            },
            _ => Err(error_unexpected()),
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
            _ => Err(error_unexpected()),
        }
    }

}

struct SerialReader<'l, 'data: 'l, K, V>
where K: KeyLoad, V: Load
{
    loader: &'l mut Loader<'data>,
    array_len: u32,
    assoc_loglen: Option<u16>,
    assoc_last_free: u32,
    assoc_len: u32,
    mask: u8, mask_len: u8,
    output: PhantomData<TableItem<K, V>>,
}

impl<'l, 'data: 'l, K, V> SerialReader<'l, 'data, K, V>
where K: KeyLoad, V: Load
{
    fn new(
        loader: &'l mut Loader<'data>,
        array_len: u32,
        assoc_loglen: Option<u16>, assoc_last_free: u32,
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
        let link_code = self.loader.read_byte()?;
        if link_code & 0x01 > 0 {
            return Err(Error::from("unexpected link code"));
        }
        let mut link: i32 = (link_code >> 2).into();
        if link_code & 0x02 > 0 {
            link = -link;
        }
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

impl<'l, 'data: 'l, K, V> TableSize for SerialReader<'l, 'data, K, V>
where K: KeyLoad, V: Load
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

impl<'l, 'data: 'l, K, V> Iterator for SerialReader<'l, 'data, K, V>
where K: KeyLoad, V: Load
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

impl<'l, 'data: 'l, K, V> TableLoader for SerialReader<'l, 'data, K, V>
where K: KeyLoad, V: Load
{
    type Key = K;
    type Value = V;
    type Error = Error;
}

