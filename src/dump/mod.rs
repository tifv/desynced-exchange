//! A specialized imitation of `serde::ser`.
//! Due to the nature of serialization format, it is more serializer-driven.

use thiserror::Error;

use crate::{
    Exchange, ExchangeKind,
    table::{TableItem, TableSize},
};

mod value;
mod compress;

#[derive(Debug, Error)]
#[error("Dump error: {reason}")]
pub struct Error {
    reason: String,
}

impl From<&str> for Error {
    fn from(reason: &str) -> Self {
        Self{reason: String::from(reason)}
    }
}

impl From<String> for Error {
    fn from(reason: String) -> Self {
        Self{reason}
    }
}

macro_rules! error_from_error {
    ($type:ty) => {
        impl From<$type> for Error {
            fn from(value: $type) -> Self {
                Self::from(value.to_string())
            }
        }
    };
}

error_from_error!(std::io::Error);
error_from_error!(std::fmt::Error);
error_from_error!(std::num::TryFromIntError);

pub fn dump_blueprint<P, B>(exchange: Exchange<P, B>) -> Result<String, Error>
where P: Dump, B: Dump
{
    let mut compressor = compress::Compressor::new(
        String::new(),
        ExchangeKind::from(&exchange),
    )?;
    let mut dumper = value::Dumper::new(compressor.content_writer());
    match exchange {
        Exchange::Blueprint(data) => data.dump(&mut dumper)?,
        Exchange::Behavior(data) => data.dump(&mut dumper)?,
    }
    dumper.finish();
    compressor.finish()
}

pub trait Dump {
    fn dump<DD: Dumper>(&self, dumper: DD) -> Result<DD::Ok, Error>;
}

impl<'v, D: Dump> Dump for &'v D {
    fn dump<DD: Dumper>(&self, dumper: DD) -> Result<DD::Ok, Error> {
        <D as Dump>::dump(self, dumper)
    }
}

pub trait DumpKey {
    fn dump_key<DD: KeyDumper>(&self, dumper: DD) -> Result<DD::Ok, Error>;
}

pub trait DumpTableIterator : TableSize + Iterator<
    Item=Option<TableItem<Self::Key, Self::Value>> >
{
    type Key: DumpKey;
    type Value: Dump;
}

pub trait KeyDumper : Sized {
    type Ok;
    fn dump_integer(self, value: i32) -> Result<Self::Ok, Error>;
    fn dump_string(self, value: &str) -> Result<Self::Ok, Error>;
}

pub trait Dumper : Sized {
    type Ok;
    fn dump_nil(self) -> Result<Self::Ok, Error>;
    fn dump_boolean(self, value: bool) -> Result<Self::Ok, Error>;
    fn dump_integer(self, value: i32) -> Result<Self::Ok, Error>;
    fn dump_float(self, value: f64) -> Result<Self::Ok, Error>;
    fn dump_string(self, value: &str) -> Result<Self::Ok, Error>;
    fn dump_table<K, V, T>( self,
        table: T,
    ) -> Result<Self::Ok, Error>
    where
        K: DumpKey, V: Dump,
        T: DumpTableIterator<Key=K, Value=V>,
    ;
}

