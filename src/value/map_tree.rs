#![allow(dead_code)]

use serde::{
    Deserialize, de::Visitor, Deserializer,
    Serialize, Serializer,
};

use crate::{
    common::TransparentRef,
    string::Str,
    serde::FlatOption,
    table_iter::TableSize as _,
};

use super::{
    Key, Value as InnerValue, Table,
    TableMapBuilder,
};

pub fn serialize<T, S>(value: &Option<InnerValue>, ser: S)
-> Result<S::Ok, S::Error>
where S: Serializer
{
    FlatOption(value.as_ref().map(Value::from_ref)).serialize(ser)
}

pub fn deserialize<'de, D>(de: D)
-> Result<Option<InnerValue>, D::Error>
where D: Deserializer<'de>
{
    Ok(FlatOption::deserialize(de)?.into_inner().map(Value::into_inner))
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "InnerValue")]
#[serde(untagged)]
enum ValueDef {
    Boolean(bool),
    Integer(i32),
    Float(f64),
    String(Str),
    #[serde(
        serialize_with="table_serialize",
        deserialize_with="table_deserialize" )]
    Table(Table),
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct Value(
    #[serde(with = "ValueDef")]
    pub InnerValue,
);

impl std::fmt::Debug for Value {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<InnerValue> for Value {
    fn as_ref(&self) -> &InnerValue {
        &self.0
    }
}

// SAFETY: `Self` is `repr(transparent)` over `Target`
unsafe impl TransparentRef for Value {
    type Target = InnerValue;
}

impl Value {
    #[must_use]
    pub fn into_inner(self) -> InnerValue {
        <Self as TransparentRef>::unwrap(self)
    }
}

impl<'v> FlatOption<&'v Value> {
    #[inline]
    #[must_use]
    fn from_option_ref(option_ref: Option<&'v InnerValue>) -> Self {
        FlatOption(option_ref.map(Value::from_ref))
    }
}

fn table_serialize<S>(value: &Table, ser: S)
-> Result<S::Ok, S::Error>
where S: Serializer
{
    if value.assoc_loglen().is_none() {
        ser.collect_seq( value.array_iter()
            .map(Option::as_ref)
            .map(FlatOption::from_option_ref) )
    } else {
        ser.collect_map( value.sorted_iter()
            .map(|(k, v)| (k, Value::from_ref(v))) )
    }
}

fn table_deserialize<'de, D>(de: D)
-> Result<Table, D::Error>
where D: Deserializer<'de>
{
    de.deserialize_any(TableVisitor)
}

struct TableVisitor;

impl<'de> Visitor<'de> for TableVisitor {
    type Value = Table;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "a map or an array")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de>
    {
        let mut builder = TableMapBuilder::new();
        let mut index = 1;
        while let Some(FlatOption(value)) = seq.next_element()? {
            let value = value.map(Value::into_inner);
            builder.insert(Key::Index(index), value);
            index += 1;
        }
        Ok(builder.finish())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de>
    {
        let mut builder = TableMapBuilder::new();
        while let Some((key, FlatOption(value))) = map.next_entry()? {
            let value = value.map(Value::into_inner);
            builder.insert::<Key>(key, value);
        }
        Ok(builder.finish())
    }

}

