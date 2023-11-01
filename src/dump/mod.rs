//! A specialized imitation of `serde::ser`.
//! Due to the nature of serialization format, it is more serializer-driven.

use crate::{
    Exchange,
    table::{TableItem, TableSize},
};

mod writer;
pub(crate) mod value;
pub(crate) mod compress;

pub trait Error : std::error::Error + for<'s> From<&'s str> {}

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[error("Dump error: {reason}")]
    pub struct Error {
        reason: String,
    }

    impl<'s> From<&'s str> for Error {
        fn from(reason: &'s str) -> Self {
            Self::from(String::from(reason))
        }
    }

    impl From<String> for Error {
        fn from(reason: String) -> Self {
            Self{reason}
        }
    }

    impl super::Error for Error {}

}

pub trait Dump {
    fn dump<DD: Dumper>(&self, dumper: DD) -> Result<DD::Ok, DD::Error>;
    fn dump_option<DD: Dumper>(this: Option<&Self>, dumper: DD)
    -> Result<DD::Ok, DD::Error> {
        match this {
            None => dumper.dump_nil(),
            Some(value) => value.dump(dumper),
        }
    }
}

pub trait DumpKey {
    fn dump_key<DD: KeyDumper>(&self, dumper: DD) -> Result<DD::Ok, DD::Error>;
}

pub trait DumpTableIterator<'v> : TableSize + Iterator<
    Item=Option<TableItem<Self::Key, &'v Self::Value>> >
{
    type Key: DumpKey;
    type Value: Dump + 'v;
}

pub trait KeyDumper : Sized {
    type Ok;
    type Error;
    fn dump_integer(self, value: i32) -> Result<Self::Ok, Self::Error>;
    fn dump_string(self, value: &str) -> Result<Self::Ok, Self::Error>;
}

pub trait Dumper : Sized {
    type Ok;
    type Error;
    fn dump_nil(self) -> Result<Self::Ok, Self::Error>;
    fn dump_boolean(self, value: bool) -> Result<Self::Ok, Self::Error>;
    fn dump_integer(self, value: i32) -> Result<Self::Ok, Self::Error>;
    fn dump_float(self, value: f64) -> Result<Self::Ok, Self::Error>;
    fn dump_string(self, value: &str) -> Result<Self::Ok, Self::Error>;
    fn dump_table<'v, K, V, T>( self,
        table: T,
    ) -> Result<Self::Ok, Self::Error>
    where
        K: DumpKey, V: Dump,
        T: DumpTableIterator<'v, Key=K, Value=V>,
    ;
}

pub fn dump_blueprint<P, B>(exchange: Exchange<Option<P>, Option<B>>)
-> Result<String, error::Error>
where P: Dump, B: Dump
{
    let encoded_body = value::encode_blueprint(exchange)?;
    Ok(compress::compress(encoded_body.as_deref()))
}

