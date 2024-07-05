use serde::{Deserialize, Serialize};

mod table;

pub mod map_tree;
pub mod serde_dump;

pub use table::{TableIntoError, LimitedVec};

use crate::string::Str;

pub(crate) use self::table::{
    TableArrayBuilder,
    TableMapBuilder,
    TableBuilder,
};


#[derive(
    Clone,
    PartialEq, Eq, PartialOrd, Ord, Hash,
    Deserialize, Serialize,
)]
#[allow(clippy::exhaustive_enums)]
#[serde(untagged)]
pub enum Key {
    Index(i32),
    Name(Str),
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Index(index) => index.fmt(f),
            Self::Name(name) => name.fmt(f),
        }
    }
}

impl Key {
    #[must_use]
    #[inline]
    pub fn as_index(&self) -> Option<i32> {
        match *self {
            Self::Index(index) => Some(index),
            Self::Name(_) => None,
        }
    }
    #[must_use]
    #[inline]
    pub fn as_name(&self) -> Option<&str> {
        match *self {
            Self::Index(_) => None,
            Self::Name(ref name) => Some(name),
        }
    }
    #[must_use]
    pub fn from_maybe_known(name: &str) -> Self {
        Self::Name(match find_known_name(name) {
            Some(name) => Str::known(name),
            None => Str::from(name),
        })
    }
}

#[inline]
fn find_known_name(name: &str) -> Option<&'static str> {
    macro_rules! search_static {
        ($($name:literal,)*) => {
            match name {
                $(
                    $name => Some($name),
                )*
                _ => return None,
            }
        }
    }
    search_static!(
        // operand
        "num", "id",
        "coord", "x", "y",

        // instruction
        "op", "next",
        "cmt", "nx", "ny",
        "c", "txt", "sub",

        // logistics
        "carrier", "requester", "supplier",
        "channel_1", "channel_2", "channel_3", "channel_4",
        "high_priority", "crane_only",
        "transport_route",

        // behavior
        "name", "desc",
        "parameters", "pnames",
        "subs",

        // XXX blueprint
    )
}

impl From<&'static str> for Key {
    fn from(string: &'static str) -> Self {
        Self::Name(Str::known(string))
    }
}

impl From<Str> for Key {
    fn from(string: Str) -> Self {
        Self::Name(string)
    }
}

#[derive(Clone)]
pub enum Value {
    Boolean(bool),
    Integer(i32),
    Float(f64),
    String(Str),
    Table(Table),
}

pub type Table = table::Table<Value>;

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boolean(value) => value.fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Float  (value) => value.fmt(f),
            Self::String (value) => value.fmt(f),
            Self::Table  (table) => table.fmt(f),
        }
    }
}

mod load {

use crate::{
    common::iexp2,
    string::Str,
    table_iter::TableItem,
    load::{
        Error,
        KeyLoad, KeyBuilder as KeyBuilderTr,
        Load, Builder as BuilderTr,
        Loader, TableLoader,
    },
};

use super::{
    Key, Value,
    table::load::TableLoadBuilder,
};

impl KeyLoad for Key {
    fn load_key<L: Loader>(loader: L) -> Result<Option<Self>, L::Error> {
        loader.load_key(KeyBuilder)
    }
}

struct KeyBuilder;

impl KeyBuilderTr for KeyBuilder {
    type Output = Key;

    fn build_integer<E: Error>(self, value: i32) -> Result<Self::Output, E> {
        Ok(Key::Index(value))
    }

    fn build_string<E: Error>(self, value: &str) -> Result<Self::Output, E> {
        Ok(Key::from_maybe_known(value))
    }

}

impl Load for Value {
    fn load<L: Loader>(loader: L) -> Result<Option<Self>, L::Error> {
        loader.load_value(Builder)
    }
}

pub struct Builder;

impl BuilderTr for Builder {
    type Key = Key;
    type Value = Value;
    type Output = Value;

    #[inline]
    fn build_boolean<E: Error>( self,
        value: bool,
    ) -> Result<Option<Value>, E> {
        Ok(Some(Value::Boolean(value)))
    }

    #[inline]
    fn build_integer<E: Error>( self,
        value: i32,
    ) -> Result<Option<Value>, E> {
        Ok(Some(Value::Integer(value)))
    }

    #[inline]
    fn build_float<E: Error>( self,
        value: f64,
    ) -> Result<Option<Value>, E> {
        Ok(Some(Value::Float(value)))
    }

    #[inline]
    fn build_string<E: Error>( self,
        value: &str,
    ) -> Result<Option<Value>, E> {
        Ok(Some(Value::String(Str::from(value))))
    }

    fn build_table<T>(self, items: T) -> Result<Option<Value>, T::Error>
    where
        T: TableLoader<Key=Self::Key, Value=Self::Value>,
        T::Error : Error
    {
        let array_len = items.array_len();
        let assoc_loglen = items.assoc_loglen();
        let assoc_len = iexp2(assoc_loglen);
        let mut table = TableLoadBuilder::<Value>::new(array_len, assoc_loglen);
        table.set_assoc_last_free(items.assoc_last_free());
        let mut array_index = 0;
        let mut assoc_index = 0;
        for item in items {
            let item = item?;
            match (item, array_index < array_len, assoc_index < assoc_len) {
                (Some(TableItem::Array(value)), true, _) => {
                    let index = array_index;
                    array_index += 1;
                    table.array_insert::<T::Error>(index, value)?;
                },
                (None, true, _) => array_index += 1,
                (Some(TableItem::Array(_)), false, _) =>
                    panic!("unexpected array item"),
                (Some(TableItem::Assoc(assoc_item)), false, true) => {
                    let index = assoc_index;
                    assoc_index += 1;
                    table.assoc_insert::<T::Error>(index, assoc_item)?;
                },
                (None, false, true) => assoc_index += 1,
                (Some(TableItem::Assoc(_)), true, _) =>
                    panic!("unexpected assoc item"),
                (_, false, false) =>
                    panic!("unexpected item"),
            }
        }
        Ok(Some(Value::Table(table.finish::<T::Error>()?)))
    }

}

}

mod dump {

use crate::dump::{
    KeyDump, Dump,
    KeyDumper, Dumper,
};

use super::{Key, Value};

impl KeyDump for Key {
    fn dump_key<D: KeyDumper>(&self, dumper: D)
    -> Result<D::Ok, D::Error> {
        match *self {
            Self::Index(index) => dumper.dump_integer(index),
            Self::Name(ref name) => dumper.dump_string(name),
        }
    }

}

impl Dump for Value {
    fn dump<D: Dumper>(&self, dumper: D) -> Result<D::Ok, D::Error> {
        match *self {
            Self::Boolean(value) =>
                dumper.dump_boolean(value),
            Self::Integer(value) =>
                dumper.dump_integer(value),
            Self::Float(value) =>
                dumper.dump_float(value),
            Self::String(ref value) =>
                dumper.dump_string(value),
            Self::Table(ref table) =>
                dumper.dump_table(table.dump_iter()),
        }
    }
}

}

#[cfg(test)]
pub(crate) mod test {
    type Value = super::Value;

    use crate::{test, dumper, loader};

    #[test]
    fn test_1_load() {
        let decompress = loader::decompress::decompress;
        let decode = loader::decode_blueprint::<Value, Value>;
        let encode = dumper::encode_blueprint::<Value, Value>;
        let compress = dumper::compress::compress;

        let exchange = test::EXCHANGE_BEHAVIOR_1_UNIT;
        let encoded = decompress(exchange)
            .unwrap();
        let value = decode(encoded.clone())
            .unwrap();
        let reencoded = encode(value)
            .unwrap();
        assert_eq!(encoded, reencoded);
        let reexchange = compress(reencoded.as_deref());
        let revalue = decode(decompress(&reexchange).unwrap()).unwrap();
        assert_eq!(reencoded, encode(revalue).unwrap());
    }

}

