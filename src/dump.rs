//! A specialized imitation of `serde::ser`.
//! Due to the nature of serialization format, it is more serializer-driven.

use crate::table_iter::{TableItem, TableSize};

pub trait Error : std::error::Error + for<'s> From<&'s str> {}

pub trait KeyDump {
    fn dump_key<KDD: KeyDumper>(&self, dumper: KDD)
    -> Result<KDD::Ok, KDD::Error>;
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

pub trait TableDumpIter<'v> : TableSize + Iterator<
    Item = Option<TableItem<Self::Key, &'v Self::Value>> >
{
    type Key: KeyDump;
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
    fn dump_table<'v, T>(self, table: T) -> Result<Self::Ok, Self::Error>
    where
        T: TableDumpIter<'v>,
        T::Key: KeyDump,
        T::Value: Dump,
    ;
}

