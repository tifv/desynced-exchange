use crate::{
    Exchange,
    table::{TableItem, AssocItem, iexp2},
};
use super::{
    error::Error, writer::Writer,
    DumpKey, Dump, DumpTableIterator,
    Dumper as DD, KeyDumper as KDD,
};

const EXCEEDED_LOGLEN: u16 = crate::MAX_ASSOC_LOGLEN + 1;

pub(crate) fn encode_blueprint<P, B>(exchange: Exchange<Option<P>, Option<B>>)
-> Result<Exchange<Vec<u8>, Vec<u8>>, Error>
where P: Dump, B: Dump
{
    fn dump<V: Dump>(value: Option<V>) -> Result<Vec<u8>, Error> {
        let mut dumper = Dumper::new();
        V::dump_option(value.as_ref(), &mut dumper)?;
        Ok(dumper.finish())
    }
    exchange.map(dump, dump).transpose()
}

pub(super) struct Dumper {
    output: Writer,
}

impl Dumper {

    pub(super) fn new() -> Self {
        Self{output: Writer::new()}
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.output.into_vec()
    }

    #[inline]
    fn write_byte(&mut self, value: u8) {
        self.output.write_byte(value);
    }

    #[inline]
    fn write_array<const N: usize>(&mut self, value: [u8; N]) {
        self.output.write_array(value);
    }

    #[inline]
    fn write_slice(&mut self, value: &[u8]) {
        self.output.write_slice(value)
    }

    #[inline]
    fn dump_table_header<'v, T: DumpTableIterator<'v>>( &mut self,
        table: &T,
    ) -> Result<(), Error> {
        self.output.reserve(4);
        match (table.array_len(), table.assoc_loglen()) {
            (len @ 0 ..= 0xF, None) => {
                self.write_byte(0x90 | (len as u8));
            },
            (len @ 0x_0010 ..= 0x_FFFF, None) => {
                self.write_byte(0xDC);
                self.write_array::<2>((len as u16).to_le_bytes());
            },
            (0, Some(logsize @ 0 ..= 0x7)) => {
                self.write_byte(0x80 | ((logsize << 1) as u8));
                self.write_byte((table.assoc_last_free() << 1) as u8);
            },
            (len @ 0x01 ..= 0x7F, Some(logsize @ 0 ..= 0x5)) => {
                self.write_byte(0x81 | ((logsize << 1) as u8));
                self.write_byte((len << 1) as u8);
                self.write_byte((table.assoc_last_free() << 1) as u8);
            },
            (len @ 0x_0080 ..= 0x_3FFF, Some(logsize @ 0 ..= 0x5)) => {
                self.write_byte(0x81 | ((logsize << 1) as u8));
                self.write_byte(((len & ((1 <<  7) - (1 << 0))) << 1) as u8 + 1);
                self.write_byte(((len & ((1 << 14) - (1 << 7))) >> 6) as u8);
                self.write_byte((table.assoc_last_free() << 1) as u8);
            },
            (0x1_0000 ..= u32::MAX, None) |
            (0x4000 ..= u32::MAX, Some(_)) |
            (_, Some(self::EXCEEDED_LOGLEN ..= u16::MAX)) =>
                return Err(Error::from("unsupported table size")),
        }
        Ok(())
    }

    #[inline]
    fn dump_dead_key(&mut self) {
        self.write_byte(0xC5)
    }

}

impl KDD for &mut Dumper {
    type Ok = ();
    type Error = Error;

    fn dump_integer(self, value: i32) -> Result<Self::Ok, Error> {
        <Self as DD>::dump_integer(self, value)
    }

    fn dump_string(self, value: &str) -> Result<Self::Ok, Error> {
        <Self as DD>::dump_string(self, value)
    }

}

impl DD for &mut Dumper {
    type Ok = ();
    type Error = Error;

    fn dump_nil(self) -> Result<Self::Ok, Error> {
        self.write_byte(0xC0);
        Ok(())
    }

    fn dump_boolean(self, value: bool) -> Result<Self::Ok, Error> {
        self.write_byte(0xC2 | u8::from(value));
        Ok(())
    }

    fn dump_integer(self, value: i32) -> Result<Self::Ok, Error> {
        self.output.reserve(5);
        match value {
            -0x20 ..= 0x7F => {
                self.write_array::<1>((value as i8).to_le_bytes());
            },
            0x80 ..= 0xFF => {
                self.write_byte(0xCC);
                self.write_array::<1>((value as u8).to_le_bytes());
            },
            0x_0100 ..= 0x_FFFF => {
                self.write_byte(0xCD);
                self.write_array::<2>((value as u16).to_le_bytes());
            },
            0x_0001_0000 ..= 0x_7FFF_FFFF => {
                self.write_byte(0xCE);
                self.write_array::<4>(value.to_le_bytes());
            },
            -0x7F ..= -0x21 => {
                self.write_byte(0xD0);
                self.write_array::<1>((value as i8).to_le_bytes());
            },
            -0x_7FFF ..= -0x_0080 => {
                self.write_byte(0xD1);
                self.write_array::<2>((value as i16).to_le_bytes());
            },
            -0x_8000_0000 ..= -0x_0000_8000 => {
                self.write_byte(0xD2);
                self.write_array::<4>(value.to_le_bytes());
            },
        }
        Ok(())
    }

    fn dump_float(self, value: f64) -> Result<Self::Ok, Error> {
        self.output.reserve(9);
        self.write_byte(0xCB);
        self.write_array::<8>(value.to_le_bytes());
        Ok(())
    }

    fn dump_string(self, value: &str) -> Result<Self::Ok, Error> {
        self.output.reserve(3);
        match value.len() {
            0 ..= 0x1F => {
                self.write_byte(0xA0 | (value.len() as u8));
                self.write_slice(value.as_bytes());
            },
            0x20 ..= 0xFF => {
                self.write_byte(0xD9);
                self.write_byte(value.len() as u8);
                self.write_slice(value.as_bytes());
            },
            0x_0100 ..= 0x_FFFF => {
                self.write_byte(0xDA);
                self.write_array::<2>((value.len() as u16).to_le_bytes());
                self.write_slice(value.as_bytes());
            },
            _ => return Err(Error::from("too long string")),
        }
        Ok(())
    }

    fn dump_table<'v, K, V, T>( self,
        table: T,
    ) -> Result<Self::Ok, Error>
    where
        K: DumpKey, V: Dump,
        T: DumpTableIterator<'v, Key=K, Value=V>,
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

struct SerialWriter<'v, K, V>
where
    K: DumpKey, V: Dump,
{
    dumper: &'v mut Dumper,
    values: [Option<TableItem<K, &'v V>>; SERIAL_LEN],
    len: u8,
    mask: u8,
}

impl<'v, K, V> SerialWriter<'v, K, V>
where
    K: DumpKey, V: Dump,
{
    fn new(dumper: &'v mut Dumper) -> Self {
        Self{
            dumper,
            values: [(); SERIAL_LEN].map(|()| None),
            len: 0,
            mask: 0,
        }
    }
    fn push( &mut self,
        item: Option<TableItem<K, &'v V>>,
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
        self.dumper.write_byte(self.mask);
        for item in &mut self.values[..self.len as usize] {
            let Some(item) = item.take() else { continue };
            let (key, value, link) = match item {
                TableItem::Array(value) => {
                    value.dump(&mut *self.dumper)?;
                    continue;
                },
                TableItem::Assoc(AssocItem::Dead{link}) =>
                    (None, None, link),
                TableItem::Assoc(AssocItem::Live{key, value, link}) =>
                    (Some(key), value, link),
            };
            if let Some(value) = value {
                value.dump(&mut *self.dumper)?;
            } else {
                self.dumper.dump_nil()?;
            }
            match key {
                // None => self.ser.serialize_dead()?,
                None => self.dumper.dump_dead_key(),
                Some(key) => key.dump_key(&mut *self.dumper)?,
            }
            self.dumper.write_byte(match Self::encode_link(link) {
                Some(code) => code,
                None => return Err(Error::from("unsupported table size"))
            });
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

