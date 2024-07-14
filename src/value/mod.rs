use crate::Str;

mod table;
pub use table::{ArrayBuilder, TableBuilder};
pub(crate) use table::ArrayIntoIter;

#[derive( Clone,
    PartialEq, Eq, PartialOrd, Ord, Hash )]
#[allow(clippy::exhaustive_enums)]
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

        // behavior
        "name", "desc",
        "parameters", "pnames",
        "subs",

        // blueprint
        // "name" (duplicate)
        "frame", "powered_down", "disconnected", "logistics",
        "components", "regs", "links",
        "locks",

        // logistics
        "carrier", "requester", "supplier",
        "channel_1", "channel_2", "channel_3", "channel_4",
        "high_priority", "crane_only",
        "transport_route",
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

fn err_key_from_value() -> crate::error::DumpError {
    crate::error::DumpError::from(
        "only integers ans strings can serve as keys")
}

#[derive(Clone, PartialEq)]
#[allow(clippy::exhaustive_enums)]
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

#[allow(clippy::use_self)]
impl TryFrom<Value> for Key {
    type Error = crate::error::DumpError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value {
            Value::Integer(number) => Key::Index(number),
            Value::String(string) => Key::Name(string),
            Value::Boolean(_) | Value::Float(_) | Value::Table(_)
                => return Err(err_key_from_value()),
        })
    }
}

impl TryFrom<Option<Value>> for Key {
    type Error = crate::error::DumpError;
    fn try_from(value: Option<Value>) -> Result<Self, Self::Error> {
        Self::try_from(value.ok_or_else(err_key_from_value)?)
    }
}

#[allow(clippy::use_self)]
impl From<Key> for Value {
    fn from(value: Key) -> Self {
        match value {
            Key::Index(index) => Value::Integer(index),
            Key::Name(name) => Value::String(name),
        }
    }
}


mod load {

use crate::{
    Str,
    load::{
        Error,
        KeyLoad, KeyBuilder as KeyBuilderTr,
        Load, Builder as BuilderTr,
        Loader, TableLoader,
    },
};

use super::{Key, Value, Table};

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

struct Builder;

impl BuilderTr for Builder {
    type Key = Key;
    type Value = Value;
    type Output = Value;

    #[inline]
    fn build_boolean<E: Error>(self, value: bool)
    -> Result<Option<Value>, E>
    {
        Ok(Some(Value::Boolean(value)))
    }

    #[inline]
    fn build_integer<E: Error>(self, value: i32)
    -> Result<Option<Value>, E>
    {
        Ok(Some(Value::Integer(value)))
    }

    #[inline]
    fn build_float<E: Error>(self, value: f64)
    -> Result<Option<Value>, E>
    {
        Ok(Some(Value::Float(value)))
    }

    #[inline]
    fn build_string<E: Error>(self, value: &str)
    -> Result<Option<Value>, E>
    {
        Ok(Some(Value::String(Str::from(value))))
    }

    fn build_table<T>(self, items: T) -> Result<Option<Value>, T::Error>
    where
        T : TableLoader<Key=Self::Key, Value=Self::Value>,
        T::Error : Error
    {
        Ok(Some(Value::Table(Table::load(items)?)))
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


mod de {

use serde::{Deserialize, de};

use crate::{
    Str,
    common::serde::{ self as common_serde,
        DeserializeOption,
    },
};

use super::{Key, Value};

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de>
    {
        de.deserialize_any(KeyVisitor)
    }
}

struct KeyVisitor;

impl<'de> de::Visitor<'de> for KeyVisitor {
    type Value = Key;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "an integer or a string")
    }

    common_serde::visit_forward_to_i32!();

    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(Key::Index(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(Key::from_maybe_known(v))
    }

}

impl<'de> DeserializeOption<'de> for Value {
    fn deserialize_option<D>(de: D) -> Result<Option<Self>, D::Error>
    where D: serde::Deserializer<'de>
    {
        de.deserialize_any(ValueVisitor)
    }
}

common_serde::forward_de_to_de_option!(Value);

struct ValueVisitor;

impl<'de> de::Visitor<'de> for ValueVisitor {
    type Value = Option<Value>;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "anything, really")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(Some(Value::Boolean(v)))
    }

    common_serde::visit_forward_to_i32!();

    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(Some(Value::Integer(v)))
    }

    common_serde::visit_forward_to_f64!();

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(Some(Value::Float(v)))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(Some(Value::String(Str::from(v))))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where D: de::Deserializer<'de>
    {
        deserializer.deserialize_any(self)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(Some(Value::Table(
            super::table::de::TableVisitor::new().visit_unit()? )))
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where A: de::SeqAccess<'de>
    {
        Ok(Some(Value::Table(
            super::table::de::TableVisitor::new().visit_seq(seq)? )))
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where A: de::MapAccess<'de>
    {
        Ok(Some(Value::Table(
            super::table::de::TableVisitor::new().visit_map(map)? )))
    }

}

}


mod ser {

use ::serde::{Serialize, ser};

use crate::common::serde as common_serde;

use super::{Key, Value};

impl Serialize for Key {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: ser::Serializer
    {
        match self {
            Self::Index(index) => index.serialize(ser),
            Self::Name (name)  => name .serialize(ser),
        }
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: ser::Serializer
    {
        match self {
            Self::Boolean(value) => value.serialize(ser),
            Self::Integer(value) => value.serialize(ser),
            Self::Float  (value) => value.serialize(ser),
            Self::String (value) => value.serialize(ser),
            Self::Table  (table) => table.serialize(ser),
        }
    }
}

common_serde::impl_flat_se_option!(Value);

}


#[cfg(test)]
mod test {

use crate::common::{
    TransparentRef,
    serde::{OptionSerdeWrap, OptionRefSerdeWrap},
};

use super::Value;

#[test]
fn test_value_serde() {
    let value: Option<Value> =
        ron::from_str::<OptionSerdeWrap<_>>(crate::test::RON_VALUE_1)
        .unwrap().into_inner();
    let ron_again = ron::to_string(
        OptionRefSerdeWrap::from_ref(&value.as_ref()) ).unwrap();
    assert_eq!(ron_again.as_str(), crate::test::RON_VALUE_1_COMPACT);
}

}

