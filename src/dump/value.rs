use std::io::Write;

use crate::table::{TableItem, AssocItem, iexp2};
use super::{
    Error,
    DumpKey, Dump, DumpTableIterator,
    Dumper as DD, KeyDumper as KDD,
};

pub(super) struct Dumper<W: Write> {
    writer: W,
}

impl<W: Write> Dumper<W> {

    pub(super) fn new(writer: W) -> Self { Self{writer} }

    pub(super) fn finish(self) -> W { self.writer }

    fn reborrow(&mut self) -> &mut Self { self }

    fn write_byte(&mut self, value: u8) -> Result<(), Error> {
        // Ok(self.writer.write_all(&[value])?)
        Ok(self.writer.write_all(&[value])?)
    }
    fn write_slice( &mut self,
        value: &[u8],
    ) -> Result<(), Error> {
        Ok(self.writer.write_all(value)?)
    }
    fn write_array<const N: usize>( &mut self,
        value: [u8; N],
    ) -> Result<(), Error> {
        Ok(self.writer.write_all(&value)?)
    }
    fn dump_table_header<T: DumpTableIterator>( &mut self,
        table: &T,
    ) -> Result<(), Error> {
        match (table.array_len(), table.assoc_loglen()) {
            (len @ 0 ..= 0xF, None) => {
                self.write_byte(0x90 | (len as u8))?;
            },
            (len @ 0x_0010 ..= 0x_FFFF, None) => {
                self.write_byte(0xDC)?;
                self.write_array::<2>((len as u16).to_le_bytes())?;
            },
            (0, Some(logsize @ 0 ..= 0x7)) => {
                self.write_byte(0x80 | ((logsize << 1) as u8))?;
                self.write_byte((table.assoc_last_free() << 1) as u8)?;
            },
            (len @ 0x01 ..= 0x7F, Some(logsize @ 0 ..= 0x5)) => {
                self.write_byte(0x81 | ((logsize << 1) as u8))?;
                self.write_byte((len << 1) as u8)?;
                self.write_byte((table.assoc_last_free() << 1) as u8)?;
            },
            (len @ 0x_0080 ..= 0x_3FFF, Some(logsize @ 0 ..= 0x5)) => {
                self.write_byte(0x81 | ((logsize << 1) as u8))?;
                self.write_byte(((len & ((1 <<  7) - (1 << 0))) << 1) as u8 + 1)?;
                self.write_byte(((len & ((1 << 14) - (1 << 7))) >> 6) as u8)?;
                self.write_byte((table.assoc_last_free() << 1) as u8)?;
            },
            (0x1_0000 ..= u32::MAX, None) |
            (0x4000 ..= u32::MAX, Some(_)) |
            (_, Some(0x6 ..= u16::MAX)) =>
                return Err(Error::from("unsupported table size")),
        }
        Ok(())
    }
    fn dump_dead_key(&mut self) -> Result<(), Error> {
        self.write_byte(0xC5)
    }
    // fn dump_value<V: Dump>(&mut self, value: &V) -> Result<(), Error> {
    //     value.dump(self)
    // }
    // fn dump_key<K: DumpKey>(&mut self, key: &K) -> Result<(), Error> {
    //     key.dump_key(self)
    // }

}

impl<W: Write> KDD for &mut Dumper<W> {
    type Ok = ();

    fn dump_integer(self, value: i32) -> Result<Self::Ok, Error> {
        <Self as DD>::dump_integer(self, value)
    }

    fn dump_string(self, value: &str) -> Result<Self::Ok, Error> {
        <Self as DD>::dump_string(self, value)
    }

}

impl<W: Write> DD for &mut Dumper<W> {
    type Ok = ();

    fn dump_nil(self) -> Result<Self::Ok, Error> {
        self.write_byte(0xC0)
    }

    fn dump_boolean(self, value: bool) -> Result<Self::Ok, Error> {
        self.write_byte(0xC2 | u8::from(value))
    }

    fn dump_integer(self, value: i32) -> Result<Self::Ok, Error> {
        match value {
            -0x20 ..= 0x7F => {
                self.write_array::<1>((value as i8).to_le_bytes())?;
            },
            0x80 ..= 0xFF => {
                self.write_byte(0xCC)?;
                self.write_array::<1>((value as u8).to_le_bytes())?;
            },
            0x_0100 ..= 0x_FFFF => {
                self.write_byte(0xCD)?;
                self.write_array::<2>((value as u16).to_le_bytes())?;
            },
            0x_0001_0000 ..= 0x_7FFF_FFFF => {
                self.write_byte(0xCE)?;
                self.write_array::<4>(value.to_le_bytes())?;
            },
            -0x7F ..= -0x21 => {
                self.write_byte(0xD0)?;
                self.write_array::<1>((value as i8).to_le_bytes())?;
            },
            -0x_7FFF ..= -0x_0080 => {
                self.write_byte(0xD1)?;
                self.write_array::<2>((value as i16).to_le_bytes())?;
            },
            -0x_8000_0000 ..= -0x_0000_8000 => {
                self.write_byte(0xD2)?;
                self.write_array::<4>(value.to_le_bytes())?;
            },
        }
        Ok(())
    }

    fn dump_float(self, value: f64) -> Result<Self::Ok, Error> {
        self.write_byte(0xCB)?;
        self.write_array::<8>(value.to_le_bytes())?;
        Ok(())
    }

    fn dump_string(self, value: &str) -> Result<Self::Ok, Error> {
        match value.len() {
            0 ..= 0x1F => {
                self.write_byte(0xA0 | (value.len() as u8))?;
                self.write_slice(value.as_bytes())?;
            },
            0x20 ..= 0xFF => {
                self.write_byte(0xD9)?;
                self.write_byte(value.len() as u8)?;
                self.write_slice(value.as_bytes())?;
            },
            0x_0100 ..= 0x_FFFF => {
                self.write_byte(0xDA)?;
                self.write_array::<2>((value.len() as u16).to_le_bytes())?;
                self.write_slice(value.as_bytes())?;
            },
            _ => return Err(Error::from("too long string")),
        }
        Ok(())
    }

    fn dump_table<K, V, T>( self,
        mut table: T,
    ) -> Result<Self::Ok, Error>
    where
        K: DumpKey, V: Dump,
        T: DumpTableIterator<Key=K, Value=V>,
    {
        let mut array_len = table.array_len();
        let mut assoc_len = iexp2(table.assoc_loglen());
        self.dump_table_header(&table)?;
        let mut serial = SerialWriter::new(self);
        for item in table {
            match (&item, array_len > 0, assoc_len > 0) {
                (Some(TableItem::Array(_)) | None, true, _) =>
                    array_len -= 1,
                (Some(TableItem::Array(_)), false, _) =>
                    panic!("unexpected array item"),
                (Some(TableItem::Assoc(_)) | None, false, true) =>
                    assoc_len -= 1,
                (Some(TableItem::Assoc(_)), true, _) |
                (Some(TableItem::Assoc(_)), _, false) =>
                    panic!("unexpected assoc item"),
                (None, false, false) => panic!("unexpected item"),
            }
            serial.push(item)?;
        }
        assert!( array_len == 0 && assoc_len == 0,
            "less than expected number of items" );
        serial.end()?;
        Ok(())
    }

}

const SERIAL_LEN: usize = {
    let len = 8;
    assert!(len <= u8::BITS as usize);
    len
};

struct SerialWriter<'v, K, V, W>
where
    K: DumpKey, V: Dump,
    W: Write,
{
    dumper: &'v mut Dumper<W>,
    values: [Option<TableItem<K, V>>; SERIAL_LEN],
    len: u8,
    mask: u8,
}

impl<'v, K, V, W> SerialWriter<'v, K, V, W>
where
    K: DumpKey, V: Dump,
    W: Write,
{
    fn new(dumper: &'v mut Dumper<W>) -> Self {
        Self{
            dumper,
            values: [(); SERIAL_LEN].map(|()| None),
            len: 0,
            mask: 0,
        }
    }
    fn push( &mut self,
        item: Option<TableItem<K, V>>,
    ) -> Result<(), Error> {
        assert!(self.len < SERIAL_LEN as u8);
        match item {
            None => self.mask |= 1 << self.len,
            Some(item) =>
                if self.values[self.len as usize].replace(item).is_some() {
                    unreachable!();
                },
        }
        self.len += 1;
        if self.len == SERIAL_LEN as u8 {
            self.pop()?
        }
        Ok(())
    }
    fn pop(&mut self) -> Result<(), Error> {
        assert!(self.len > 0);
        self.dumper.write_byte(self.mask)?;
        for item in &mut self.values[..self.len as usize] {
            let Some(item) = item.take() else { continue };
            let (key, value, link) = match item {
                TableItem::Array(value) => {
                    value.dump(self.dumper.reborrow())?;
                    continue;
                },
                TableItem::Assoc(AssocItem::Dead{link}) =>
                    (None, None, link),
                TableItem::Assoc(AssocItem::Live{key, value, link}) =>
                    (Some(key), Some(value), link),
            };
            if let Some(value) = value {
                value.dump(self.dumper.reborrow())?;
            } else {
                self.dumper.dump_nil()?;
            }
            match key {
                // None => self.ser.serialize_dead()?,
                None => self.dumper.dump_dead_key()?,
                Some(key) => key.dump_key(self.dumper.reborrow())?,
            }
            self.dumper.write_byte(match Self::encode_link(link) {
                Some(code) => code,
                None => return Err(Error::from("unsupported table size"))
            })?;
        }
        self.len = 0;
        self.mask = 0;
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
    fn end(mut self) -> Result<(), Error> {
        if self.len > 0 {
            self.pop()?
        }
        Ok(())
    }
}

