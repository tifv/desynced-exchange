use serde::de::{self, Error as _};

use crate::{
    error::LoadError as Error,
    value::{Key, Value, Table},
    Exchange,
};

impl de::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Self::from(msg.to_string())
    }
}

const BLUEPRINT: &str = "Blueprint";
const BEHAVIOR : &str = "Behavior" ;

pub struct Deserializer(Exchange<Option<Value>>);

impl Deserializer {
    pub fn new(data: &str) -> Result<Self, Error> {
        Ok(Self(crate::loader::load_blueprint::<_, _, Error>(data)?))
    }
    fn err_invalid() -> Error {
        Error::from(
            "The outer value for this format should be \
             an enum with newtype variants “Blueprint” and “Behavior”.")
    }
}

impl<'de> de::Deserializer<'de> for Deserializer {
    type Error = Error;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        self.deserialize_enum("Exchange", &[], visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        visitor.visit_enum(self)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match self.0 {
            Exchange::Blueprint(None) => visitor.visit_str(BLUEPRINT),
            Exchange::Behavior (None) => visitor.visit_str(BEHAVIOR ),
            _ => Err(Self::err_invalid())
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_unit_struct<V>( self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match (self.0, name) {
            (Exchange::Blueprint(None), BLUEPRINT) |
            (Exchange::Behavior (None), BEHAVIOR )
                => visitor.visit_unit(),
            _ => Err(Self::err_invalid())
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        visitor.visit_map(DeserializerMap(Some(self.0)))
    }

    fn deserialize_newtype_struct<V>( self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match (self.0, name) {
            (Exchange::Blueprint(content), BLUEPRINT) |
            (Exchange::Behavior (content), BEHAVIOR )
                => visitor.visit_newtype_struct(
                    ValueDeserializer::new(content) ),
            _ => Err(Self::err_invalid())
        }
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match (self.0, name) {
            (Exchange::Blueprint(Some(Value::Table(content))), BLUEPRINT) |
            (Exchange::Behavior (Some(Value::Table(content))), BEHAVIOR )
                => visitor.visit_map(TableMapDeserializer::new(content)),
            _ => Err(Self::err_invalid())
        }
    }

    fn deserialize_bool<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_i8<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_i16<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_i32<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_i64<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_u8<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_u16<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_u32<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_u64<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_f32<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_f64<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_char<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_string<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_bytes<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_byte_buf<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_option<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_unit<V>(self, _: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_seq<V>(self, _: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_tuple<V>(self, _: usize, _: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_tuple_struct<V>(self, _: &'static str, _: usize, _: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { Err(Self::err_invalid()) }

    fn deserialize_ignored_any<V>(self, visitor: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    { self.deserialize_any(visitor) }

}

impl<'de> de::EnumAccess<'de> for Deserializer {
    type Error = Error;

    type Variant = ValueDeserializer;

    fn variant_seed<V>(self, seed: V)
    -> Result<(V::Value, Self::Variant), Self::Error>
    where V: de::DeserializeSeed<'de>
    {
        use de::value::StrDeserializer as StrDe;
        let (name, value) = match self.0 {
            Exchange::Blueprint(value) => (BLUEPRINT, value),
            Exchange::Behavior (value) => (BEHAVIOR , value),
        };
        Ok((
            seed.deserialize(StrDe::<Error>::new(name))?,
            ValueDeserializer::new(value),
        ))
    }

}

struct DeserializerMap(Option<Exchange<Option<Value>>>);

impl<'de> de::MapAccess<'de> for DeserializerMap {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K)
    -> Result<Option<K::Value>, Self::Error>
    where K: de::DeserializeSeed<'de>
    {
        use de::value::StrDeserializer as StrDe;
        Ok(Some(seed.deserialize(
            StrDe::<Error>::new(match self.0 {
                Some(Exchange::Blueprint(_)) => BLUEPRINT,
                Some(Exchange::Behavior (_)) => BEHAVIOR ,
                None => return Ok(None),
            })
        )?))
    }

    fn next_value_seed<V>(&mut self, seed: V)
    -> Result<V::Value, Self::Error>
    where V: de::DeserializeSeed<'de>
    {
        let Some(value) = self.0.take() else {
            panic!("missing `next_key` call");
        };
        seed.deserialize(ValueDeserializer::new(value.unwrap()))
    }

}

pub struct ValueDeserializer(Option<Value>);

impl ValueDeserializer {
    fn new(value: Option<Value>) -> Self {
        Self(value)
    }
}

impl<'de> de::Deserializer<'de> for ValueDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match self.0 {
            None                        => visitor.visit_none(),
            Some(Value::Boolean(value)) => visitor.visit_bool(value),
            Some(Value::Integer(value)) => visitor.visit_i32(value),
            Some(Value::Float  (value)) => visitor.visit_f64(value),
            Some(Value::String (value)) =>
                visitor.visit_str(value.as_ref()),
            Some(Value::Table  (table)) =>
                visitor.visit_map(TableMapDeserializer::new(table)),
        }
    }

    serde::forward_to_deserialize_any!(
        bool
        i8 i16 i32 i64
        u8 u16 u32 u64
        f32 f64
        char str string bytes byte_buf
        identifier ignored_any
    );

    fn deserialize_option<V>(self, visitor: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match self.0 {
            None    => visitor.visit_none(),
            Some(_) => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match self.0 {
            None    => visitor.visit_unit(),
            Some(_) => self.deserialize_any(visitor),
        }
    }

    fn deserialize_unit_struct<V>(self, _: &'static str, visitor: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _: &'static str, visitor: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match self.0 {
            Some(Value::Table(table)) =>
                visitor.visit_seq(TableSeqDeserializer::new(table)),
            _ => self.deserialize_any(visitor)
        }
    }

    fn deserialize_tuple<V>(self, _: usize, visitor: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(self, _: &'static str, _: usize, visitor: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match self.0 {
            Some(Value::Table(table)) =>
                visitor.visit_map(TableMapDeserializer::new(table)),
            _ => self.deserialize_any(visitor)
        }
    }

    fn deserialize_struct<V>( self, _: &'static str, _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>( self, _: &'static str, _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match self.0 {
            Some(Value::Table(table)) =>
                visitor.visit_enum(TableEnumDeserializer::new(table)?),
            _ => self.deserialize_any(visitor)
        }
    }

}

impl<'de> de::VariantAccess<'de> for ValueDeserializer {
    type Error = Error;

    fn unit_variant(self)
    -> Result<(), Self::Error>
    {
        match self.0 {
            None => Ok(()),
            Some(_) => Err(Self::Error::custom(
                "the value is not nil" ))
        }
    }

    fn newtype_variant_seed<T>(self, seed: T)
    -> Result<T::Value, Self::Error>
    where T: de::DeserializeSeed<'de>
    {
        seed.deserialize(self)
    }

    fn tuple_variant<V>(self, _: usize, visitor: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match self.0 {
            Some(Value::Table(table))
                => visitor.visit_seq(TableSeqDeserializer::new(table)),
            _ => Err(Self::Error::custom(
                "the value is not a table" ))
        }
    }

    fn struct_variant<V>(self, _: &'static [&'static str], visitor: V)
    -> Result<V::Value, Self::Error>
    where V: de::Visitor<'de>
    {
        match self.0 {
            Some(Value::Table(table))
                => visitor.visit_map(TableMapDeserializer::new(table)),
            _ => Err(Self::Error::custom(
                "the value is not a table" ))
        }
    }
}

type TableIntoIter = <Table as IntoIterator>::IntoIter;

struct TableMapDeserializer {
    iter: TableIntoIter,
    next_value: Option<Value>,
}

impl TableMapDeserializer {
    fn new(table: Table) -> Self {
        Self { iter: table.into_iter(), next_value: None }
    }
}

impl<'de> de::MapAccess<'de> for TableMapDeserializer {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K)
    -> Result<Option<K::Value>, Self::Error>
    where K: de::DeserializeSeed<'de>
    {
        let Some((key, value)) = self.iter.next() else {
            return Ok(None);
        };
        let old_value = self.next_value.replace(value);
        assert!( old_value.is_none(),
            "consequent `next_key` calls" );
        Ok(Some(seed.deserialize(
            ValueDeserializer::new(Some(key.into()))
        )?))
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where V: de::DeserializeSeed<'de>
    {
        let Some(value) = self.next_value.take() else {
            panic!("missing `next_key` call");
        };
        seed.deserialize(ValueDeserializer::new(Some(value)))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }

}

type TableArrayIntoIter = crate::value::ArrayIntoIter<Value>;

struct TableSeqDeserializer {
    iter: TableArrayIntoIter
}

impl TableSeqDeserializer {
    fn new(table: Table) -> Self {
        Self { iter: table.into_array_iter() }
    }
}

impl<'de> de::SeqAccess<'de> for TableSeqDeserializer {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T)
    -> Result<Option<T::Value>, Self::Error>
    where T: de::DeserializeSeed<'de>
    {
        let Some(value) = self.iter.next() else {
            return Ok(None);
        };
        Ok(Some(seed.deserialize(
            ValueDeserializer::new(value)
        )?))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }

}

struct TableEnumDeserializer(Key, Value);

impl TableEnumDeserializer {
    fn new(table: Table) -> Result<Self, Error> {
        if table.is_empty() {
            return Err(Error::from(
                "The table doesn't have a value to deserialize into an enum" ));
        }
        if table.len() > 1 {
            return Err(Error::from(
                "The table has too many values to deserialize into an enum" ));
        }
        let Some((key, value)) = table.into_iter().next() else {
            unreachable!()
        };
        Ok(Self(key, value))
    }
}

impl<'de> de::EnumAccess<'de> for TableEnumDeserializer {
    type Error = Error;

    type Variant = ValueDeserializer;

    fn variant_seed<V>(self, seed: V)
    -> Result<(V::Value, Self::Variant), Self::Error>
    where V: de::DeserializeSeed<'de>
    {
        let Self(key, value) = self;
        Ok((
            seed.deserialize(ValueDeserializer::new(Some(key.into())))?,
            ValueDeserializer::new(Some(value)),
        ))
    }
}

