#![allow(dead_code)]

use serde::{
    Deserialize, de::Visitor, Deserializer,
    Serialize, Serializer,
};

use crate::{
    common::TransparentRef,
    string::Str,
    table_iter::TableSize as _,
};

use super::{
    Key, Value as InnerValue, Table,
    TableMapBuilder,
};

type InnerValueOption = Option<InnerValue>;

#[derive(Clone)]
#[repr(transparent)]
pub struct ValueOption(
    pub InnerValueOption,
);

impl AsRef<InnerValueOption> for ValueOption {
    fn as_ref(&self) -> &InnerValueOption { &self.0 }
}

// SAFETY: `Self` is `repr(transparent)` over `Target`
unsafe impl TransparentRef for ValueOption {
    type Target = InnerValueOption;
}

impl ValueOption {

    #[inline]
    fn serialize_inner<S>(this: &InnerValueOption, ser: S)
    -> Result<S::Ok, S::Error>
    where S: Serializer
    { Self::from_ref(this).serialize(ser) }

    #[inline]
    fn deserialize_inner<'de, D>(de: D)
    -> Result<InnerValueOption, D::Error>
    where D: Deserializer<'de>
    { Ok(Self::deserialize(de)?.into_inner()) }

}

impl<'de> Deserialize<'de> for ValueOption {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>
    {
        Ok(Self(de.deserialize_any(ValueOptionVisitor)?))
    }
}

struct ValueOptionVisitor;

impl<'de> Visitor<'de> for ValueOptionVisitor {
    type Value = Option<InnerValue>;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Boolean(v))) }

    fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Integer(v.into()))) }

    fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Integer(v.into()))) }

    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Integer(v))) }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Integer(v.try_into().map_err(E::custom)?))) }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Integer(v.into()))) }

    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Integer(v.into()))) }

    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Integer(v.try_into().map_err(E::custom)?))) }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Integer(v.try_into().map_err(E::custom)?))) }

    fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Float(v.into()))) }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Float(v))) }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::String(Str::from(v)))) }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(None) }

    fn visit_some<D>(self, de: D) -> Result<Self::Value, D::Error>
    where D: Deserializer<'de>
    { Ok(ValueOption::deserialize(de)?.into_inner()) }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where E: serde::de::Error
    { Ok(Some(InnerValue::Table(TableVisitor.visit_unit()?))) }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de>
    { Ok(Some(InnerValue::Table(TableVisitor.visit_seq(seq)?))) }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de>
    { Ok(Some(InnerValue::Table(TableVisitor.visit_map(map)?))) }

}

impl Serialize for ValueOption {
    #[inline]
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        match self.0 {
            None => ser.serialize_none(),
            Some(ref value) => Value::from_ref(value).serialize(ser),
        }
    }
}


#[derive(Clone)]
#[repr(transparent)]
pub struct Value(
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
    #[allow(clippy::same_name_method)]
    #[must_use]
    pub fn into_inner(self) -> InnerValue {
        <Self as TransparentRef>::into_inner(self)
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>
    {
        use serde::de::Error as _;
        let Some(value) = de.deserialize_any(ValueOptionVisitor)? else {
            return Err(D::Error::custom("The value cannot be None"));
        };
        Ok(Self(value))
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        match &self.0 {
            InnerValue::Boolean (value) => value.serialize(ser),
            InnerValue::Integer (value) => value.serialize(ser),
            InnerValue::Float   (value) => value.serialize(ser),
            InnerValue::String  (value) => value.serialize(ser),
            InnerValue::Table   (table) => table_serialize(table, ser),
        }
    }
}


fn table_serialize<S>(value: &Table, ser: S)
-> Result<S::Ok, S::Error>
where S: Serializer
{
    if value.assoc_loglen().is_none() && value.array_len() > 0 {
        ser.collect_seq( value.array_iter()
            .map(ValueOption::from_ref) )
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

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where E: serde::de::Error
    {
        Ok(TableMapBuilder::new().finish())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de>
    {
        let mut builder = TableMapBuilder::new();
        let mut index = 1;
        while let Some(ValueOption(value)) = seq.next_element()? {
            builder.insert(Key::Index(index), value);
            index += 1;
        }
        Ok(builder.finish())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de>
    {
        let mut builder = TableMapBuilder::new();
        while let Some((key, ValueOption(value))) = map.next_entry()? {
            builder.insert::<Key>(key, value);
        }
        Ok(builder.finish())
    }

}

#[cfg(test)]
mod test {

use crate::string::Str;

use super::{
    ValueOption as VO, InnerValue as Value,
};

#[test]
fn test_simple_value_option_ron() {
    for (v, s) in [
        (VO(None),                                  "None"),
        (VO(Some(Value::Boolean(true))),            "true"),
        (VO(Some(Value::Boolean(false))),           "false"),
        (VO(Some(Value::Integer(42))),              "42"),
        (VO(Some(Value::Integer(-42))),             "-42"),
        (VO(Some(Value::Float(42.0))),              "42.0"),
        (VO(Some(Value::String(Str::known("42")))), "\"42\""),
    ] {
        let as_str = String::as_str;
        let ron_result = ron::to_string(&v);
        assert_eq!(ron_result.as_ref().map(as_str), Ok(s));
        let ron_result2 = ron::to_string(
            &ron::from_str::<VO>(s).unwrap() );
        assert_eq!(ron_result2.as_ref().map(as_str), Ok(s));
    }
}

#[test]
fn test_value_ron() {
    for (s0, s2) in [
        ("None"                        , "None"                        ),
        ("false"                       , "false"                       ),
        ("true"                        , "true"                        ),
        ("42"                          , "42"                          ),
        ("42.0"                        , "42.0"                        ),
        ("\"42\""                      , "\"42\""                      ),
        ("()"                          , "{}"                          ),
        ("[]"                          , "{}"                          ),
        ("{}"                          , "{}"                          ),
        ("( )"                         , "{}"                          ),
        ("{ }"                         , "{}"                          ),
        ("[ ]"                         , "{}"                          ),
        ("[42]"                        , "[42]"                        ),
        ("[{}]"                        , "[{}]"                        ),
        ("[\"42\",{}]"                 , "[\"42\",{}]"                 ),
        ("[\"\\\"42\\\"\"]"            , r#"["\"42\""]"#               ),
        ("{17:42}"                     , "{17:42}"                     ),
        ("{0:7,17:42}"                 , "{0:7,17:42}"                 ),
    ] {
        let s1 = test_single_value(s0);
        assert_eq!(s1.as_ref().map(String::as_str), Ok(s2));
    }
}

fn test_single_value(data0: &str) -> ron::Result<String> {
    let value: VO = ron::from_str(data0)?;
    let data1 = ron::to_string(&value)?;
    Ok(data1)
}

}
