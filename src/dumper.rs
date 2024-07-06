use crate::{
    common::{LogSize, iexp2},
    error::DumpError as Error,
    table_iter::{TableItem, AssocItem},
    dump::{
        KeyDump, Dump, TableDumpIter,
        Dumper as DumperTr, KeyDumper,
    },
    Exchange,
};

pub(crate) mod compress;

mod writer;
use writer::Writer;

const EXCEEDED_LOGLEN: LogSize = crate::MAX_ASSOC_LOGLEN + 1;

pub fn dump_blueprint<P, B>(exchange: Exchange<Option<P>, Option<B>>)
-> Result<String, Error>
where P: Dump, B: Dump
{
    let encoded_body = encode_blueprint(exchange)?;
    Ok(compress::compress(encoded_body.as_deref()))
}

pub(crate) fn encode_blueprint<P, B>(exchange: Exchange<Option<P>, Option<B>>)
-> Result<Exchange<Vec<u8>, Vec<u8>>, Error>
where P: Dump, B: Dump
{
    #[inline]
    fn dump<V: Dump>(value: Option<V>) -> Result<Vec<u8>, Error> {
        let mut dumper = Dumper::new();
        V::dump_option(value.as_ref(), &mut dumper)?;
        Ok(dumper.finish())
    }
    exchange.map(dump, dump).transpose()
}

#[inline]
const fn mask(loglen: u8) -> u32 {
    (1_u32 << loglen) - 1
}

pub(super) struct Dumper {
    output: Writer,
}

impl Dumper {

    pub(super) fn new() -> Self {
        Self { output: Writer::new() }
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
    fn write_ext_uint(&mut self, mut value: u32) {
        assert!(value.checked_ilog2() < Some(14));
        loop {
            let shift = 7;
            let mut byte = (value & mask(shift)) as u8;
            value >>= shift;
            let continued = value > 0;
            byte = (byte << 1) | u8::from(continued);
            self.write_byte(byte);
            if !continued {
                break;
            }
        }
    }

    #[inline]
    fn write_ext_sint(&mut self, value: i32) {
        let (mut negative, mut value) = if value >= 0 {
            (Some(false), value as u32)
        } else {
            (Some(true), value.wrapping_neg() as u32)
        };
        assert!(value.checked_ilog2() < Some(14));
        loop {
            let mut shift = 7;
            let negative = negative.take();
            if negative.is_some() {
                shift -= 1;
            }
            let mut byte = (value & mask(shift)) as u8;
            value >>= shift;
            let continued = value > 0;
            if let Some(negative) = negative {
                byte = (byte << 1) | u8::from(negative);
            }
            byte = (byte << 1) | u8::from(continued);
            self.write_byte(byte);
            if !continued {
                break;
            }
        }
    }

    #[inline]
    fn dump_table_header<'v, T>(&mut self, table: &T) -> Result<(), Error>
    where T: TableDumpIter<'v>, T::Key: KeyDump
    {
        self.output.reserve(4);
        match (table.array_len(), table.assoc_loglen()) {
            (len @ 0 ..= 0xF, None) => {
                self.write_byte(0x90 | (len as u8));
            },
            (len @ 0x_0010 ..= 0x_FFFF, None) => {
                self.write_byte(0xDC);
                self.write_array::<2>((len as u16).to_le_bytes());
            },
            (len @ 0 ..= 0x_3FFF, Some(logsize @ 0 ..= 0x7)) => {
                let has_array = len > 0;
                self.write_byte(0x80 | u8::from(has_array) | (logsize << 1));
                if has_array {
                    self.write_ext_uint(len);
                }
                self.write_ext_uint(table.assoc_last_free());
            },
            (len @ 1 ..= 0x_3FFF, Some(logsize @ 0 ..= 0x0E)) => {
                self.write_byte(0xDE);
                let has_array = len > 0;
                self.write_byte(u8::from(has_array) | (logsize << 1));
                self.write_byte(0x00);
                if has_array {
                    self.write_ext_uint(len);
                }
                self.write_ext_uint(table.assoc_last_free());
            },
            (0x0, Some(8_u8 ..= 16_u8)) | // temporary
            (0x1_0000 ..= u32::MAX, None) |
            (0x4000 ..= u32::MAX, Some(_)) |
            (_, Some(self::EXCEEDED_LOGLEN ..= LogSize::MAX)) =>
                return Err(Error::from("unsupported table size")),
        }
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    #[inline]
    fn dump_dead_key(&mut self) -> Result<(), Error> {
        self.write_byte(0xC5);
        Ok(())
    }

}

impl KeyDumper for &mut Dumper {
    type Ok = <Self as DumperTr>::Ok;
    type Error = <Self as DumperTr>::Error;

    #[inline]
    fn dump_integer(self, value: i32) -> Result<Self::Ok, Self::Error> {
        <Self as DumperTr>::dump_integer(self, value)
    }

    #[inline]
    fn dump_string(self, value: &str) -> Result<Self::Ok, Self::Error> {
        <Self as DumperTr>::dump_string(self, value)
    }
}

impl DumperTr for &mut Dumper {
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

    fn dump_table<'v, T>(self, table: T) -> Result<Self::Ok, Error>
    where
        T: TableDumpIter<'v>,
        T::Key: KeyDump,
        T::Value: Dump,
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
where K: KeyDump, V: Dump
{
    dumper: &'v mut Dumper,
    values: [Option<TableItem<K, &'v V>>; SERIAL_LEN],
    len: u8,
    mask: u8,
}

impl<'v, K, V> SerialWriter<'v, K, V>
where K: KeyDump, V: Dump
{
    fn new(dumper: &'v mut Dumper) -> Self {
        Self {
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
                TableItem::Assoc(AssocItem::Dead { link }) =>
                    (None, None, link),
                TableItem::Assoc(AssocItem::Live { key, value, link }) =>
                    (Some(key), value, link),
            };
            if let Some(value) = value {
                value.dump(&mut *self.dumper)?;
            } else {
                self.dumper.dump_nil()?;
            }
            match key {
                None => self.dumper.dump_dead_key()?,
                Some(key) => key.dump_key(&mut *self.dumper)?,
            }
            self.dumper.write_ext_sint(link);
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

