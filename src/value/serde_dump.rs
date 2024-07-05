#![allow(dead_code)]

use serde::{
    Deserialize, de::Visitor, Deserializer,
    Serialize, Serializer, ser::SerializeStruct,
};

use crate::{
    common::{ilog2_exact, TransparentRef},
    string::Str,
    serde::{Identifier, FlatOption, FlatEnumOption, VecSeed},
    table_iter::{TableItem, TableSize},
};

use super::{
    Key, Value as InnerValue,
    Table, table::AssocItem as InnerAssocItemGen,
    table::load::TableLoadBuilder,
    table::dump::TableDumpIter,
};


pub fn serialize<S>(value: &Option<InnerValue>, ser: S)
-> Result<S::Ok, S::Error>
where S: Serializer
{
    ValueOption::from_ref(value).serialize(ser)
}

pub fn serialize_ref<S>(value: &Option<&InnerValue>, ser: S)
-> Result<S::Ok, S::Error>
where S: Serializer
{
    FlatOption(value.map(Value::from_ref)).serialize(ser)
}

pub fn deserialize<'de, D>(de: D)
-> Result<Option<InnerValue>, D::Error>
where D: Deserializer<'de>
{
    // Somehow `FlatOption` doesn't do the job here.
    // Probably because one of the elements is a
    Ok(ValueOption::deserialize(de)?.into_inner())
}


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
    fn into_inner(self) -> InnerValueOption { self.0 }
}

impl Serialize for ValueOption {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        match self.0 {
            None => crate::serde::option_none::serialize(ser),
            Some(ref value) => ValueDef::serialize(value, ser),
        }
    }
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
    where
        D: Deserializer<'de>,
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


#[derive(Serialize)]
#[serde(remote = "InnerValue")]
#[serde(untagged)]
enum ValueDef {
    Boolean(bool),
    Integer(i32),
    Float(f64),
    String(Str),
    #[serde(serialize_with="table_serialize")]
    Table(Table),
}

#[derive(Clone, Serialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct Value(
    #[serde(serialize_with = "ValueDef::serialize")]
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


type InnerValueOptionRef<'v> = Option<&'v InnerValue>;

#[derive(Clone)]
#[repr(transparent)]
pub struct ValueOptionRef<'v>(
    pub InnerValueOptionRef<'v>,
);

impl<'v> Serialize for ValueOptionRef<'v> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        match self.0 {
            None => crate::serde::option_none::serialize(ser),
            Some(value) => ValueDef::serialize(value, ser),
        }
    }
}


type InnerAssocItemRef<'v> = InnerAssocItemGen::<&'v InnerValue>;

#[derive(Serialize)]
#[serde(remote = "InnerAssocItemRef")]
pub enum AssocItemRefDef<'v> {
    Dead { link: i32 },
    Live {
        key: Key,
        #[serde(serialize_with="serialize_ref")]
        value: Option<&'v InnerValue>,
        link: i32,
    },
}

#[derive(Serialize)]
#[serde(transparent)]
#[repr(transparent)]
struct AssocItemRef<'s>(
    #[serde(with = "AssocItemRefDef")]
    pub InnerAssocItemRef<'s>
);

type InnerAssocItem = InnerAssocItemGen::<InnerValue>;

#[derive(Deserialize)]
#[serde(remote = "InnerAssocItem")]
pub enum AssocItemDef {
    Dead { link: i32 },
    Live {
        key: Key,
        #[serde(deserialize_with="deserialize")]
        value: Option<InnerValue>,
        link: i32,
    },
}

#[derive(Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
struct AssocItem(
    #[serde(with = "AssocItemDef")]
    pub InnerAssocItem
);

impl AsRef<InnerAssocItem> for AssocItem {
    fn as_ref(&self) -> &InnerAssocItem {
        &self.0
    }
}

// SAFETY: `Self` is `repr(transparent)` over `Target`
unsafe impl TransparentRef for AssocItem {
    type Target = InnerAssocItem;
}

impl AssocItem {
    #[must_use]
    pub fn into_inner(self) -> InnerAssocItem {
        <Self as TransparentRef>::unwrap(self)
    }
}

fn table_serialize<S>(table: &Table, ser: S)
-> Result<S::Ok, S::Error>
where S: Serializer
{
    let mut ser = ser.serialize_struct("Table", 2)?;
    let mut dump_iter = table.dump_iter();
    if let Some(array) = dump_iter.take_array()
        .filter(|array| array.len() > 0)
    {
        ser.serialize_field("array", &DumpIterArrayPart(array))?;
    } else {
        ser.skip_field("array")?;
    }
    if dump_iter.assoc_loglen().is_some() {
        let last_free = dump_iter.assoc_last_free();
        ser.serialize_field("assoc", &DumpIterAssocItems(dump_iter))?;
        ser.serialize_field("assoc_last_free", &last_free)?;
    } else {
        ser.skip_field("assoc")?;
        ser.skip_field("assoc_last_free")?;
    }
    ser.end()
}

#[repr(transparent)]
struct DumpIterArrayPart<'s>(std::slice::Iter<'s, Option<InnerValue>>);

impl<'s> Serialize for DumpIterArrayPart<'s> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        ser.collect_seq( self.0.clone()
            .map(Option::as_ref)
            .map(ValueOptionRef) )
    }
}

struct DumpIterAssocItems<'s>(TableDumpIter<'s, InnerValue>);

impl<'s> Serialize for DumpIterAssocItems<'s> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        ser.collect_seq(self.0.clone().map(|item| {
            Some(match item? {
                TableItem::Assoc(item) => AssocItemRef(item),
                TableItem::Array(_) =>
                    unreachable!("we should have already consumed the array part"),
            })
        }).map(FlatEnumOption))
    }
}

struct TableVisitor;

impl<'de> Visitor<'de> for TableVisitor {
    type Value = Table;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "a struct with array, assoc and assoc_last_free fields")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where E: serde::de::Error
    {
        let load_err_as_custom = E::custom::<crate::error::LoadError>;
        TableLoadBuilder::new(0, None).finish().map_err(load_err_as_custom)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de>
    {
        use serde::de::Error as _;
        let load_err_as_custom = A::Error::custom::<crate::error::LoadError>;
        let Option::<ValueOption>::None = seq.next_element()? else {
            return Err(A::Error::custom(
                "a sequence should not be here (unless empty)" ));
        };
        let builder = TableLoadBuilder::new(0, None);
        builder.finish().map_err(load_err_as_custom)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de>
    {
        use serde::de::Error as _;
        let load_err_as_custom = A::Error::custom::<crate::error::LoadError>;
        let mut array = Vec::<ValueOption>::new();
        let mut assoc = Vec::<FlatEnumOption<AssocItem>>::new();
        let mut assoc_last_free: Option<u32> = None;
        while let Some(key) = map.next_key::<Identifier>()? {
            match &*key {
                "array" => array = map.next_value_seed(VecSeed(array))?,
                "assoc" => assoc = map.next_value_seed(VecSeed(assoc))?,
                "assoc_last_free" => assoc_last_free = Some(map.next_value()?),
                other => return Err(A::Error::custom(
                    format!("field key should not be “{other}”") )),
            }
        }
        let array_len = array.len().try_into()
            .map_err(A::Error::custom)?;
        let assoc_loglen = if assoc.is_empty() { None } else {Some(
            ilog2_exact(assoc.len()).ok_or_else(|| A::Error::custom(
                "the number of assoc items should be a power of two" ))?
        )};
        let mut builder = TableLoadBuilder::new(array_len, assoc_loglen);
        for (index, value) in array.into_iter().enumerate() {
            let index = index.try_into().map_err(A::Error::custom)?;
            let Some(value) = ValueOption::into_inner(value) else {
                continue;
            };
            builder.array_insert(index, value).map_err(load_err_as_custom)?;
        }
        for (index, item) in assoc.into_iter().enumerate() {
            let index = index.try_into().map_err(A::Error::custom)?;
            let Some(item) = FlatEnumOption::into_inner(item) else {
                continue;
            };
            let item = AssocItem::into_inner(item);
            builder.assoc_insert(index, item).map_err(load_err_as_custom)?;
        }
        if let Some(assoc_last_free) = assoc_last_free {
            builder.set_assoc_last_free(assoc_last_free);
        }
        builder.finish().map_err(load_err_as_custom)
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
        ("()"                          , "()"                          ),
        ("(array:[])"                  , "()"                          ),
        ("(assoc:[])"                  , "()"                          ),
        ("(array:[],assoc:[])"         , "()"                          ),
        ("(array:[42])"                , "(array:[42])"                ),
        ("(array:[()])"                , "(array:[()])"                ),
        ("(array:[\"42\",( )])"        , "(array:[\"42\",()])"                ),
        ("(assoc:[None])"              ,
         "(assoc:[None],assoc_last_free:1)" ),
        ("(assoc:[Dead(link:0)])"    ,
         "(assoc:[Dead(link:0)],assoc_last_free:1)" ),
        ("(assoc:[Live(key:17,value:42,link:0)])",
         "(assoc:[Live(key:17,value:42,link:0)],assoc_last_free:1)"),
        ("(assoc:[Dead(link:0)],assoc_last_free:0)",
         "(assoc:[Dead(link:0)],assoc_last_free:0)"),
        ("(assoc:[Live(key:17,value:42,link:0)],assoc_last_free:0)",
         "(assoc:[Live(key:17,value:42,link:0)],assoc_last_free:0)"),
        ("(array:[42],assoc:[None])",
         "(array:[42],assoc:[None],assoc_last_free:1)"),
        ("(array:[42],assoc:[Dead(link:0)])",
         "(array:[42],assoc:[Dead(link:0)],assoc_last_free:1)"),
        ("(array:[42],assoc:[Live(key:17,value:42,link:0)])",
         "(array:[42],assoc:[Live(key:17,value:42,link:0)],assoc_last_free:1)"),
        ("(array:[42],assoc:[Live(key:17,value:42,link:0)],assoc_last_free:0)",
         "(array:[42],assoc:[Live(key:17,value:42,link:0)],assoc_last_free:0)"),
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
