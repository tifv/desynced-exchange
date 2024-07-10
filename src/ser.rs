use serde::ser::{self, Error as _};

use crate::{
    error::DumpError as Error,
    Str,
    value::{
        Key, Value,
        Table, ArrayBuilder, TableBuilder,
    },
    Exchange,
};

impl ser::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Self::from(msg.to_string())
    }
}

pub struct Serializer(());

impl Serializer {
    #[must_use]
    pub fn new() -> Self {
        Self(())
    }
    fn err_invalid() -> Error {
        Error::from(
            "The outer value for this format should be \
             an enum with newtype variants “Blueprint” and “Behavior”.")
    }
}

impl Default for Serializer {
    fn default() -> Self {
        Self::new()
    }
}

impl ser::Serializer for Serializer {
    type Ok = String;
    type Error = Error;

    type SerializeSeq = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + ser::Serialize
    {
        crate::dumper::dump_blueprint(match variant {
            "Blueprint" => Exchange::Blueprint(()),
            "Behavior" => Exchange::Behavior(()),
            _ => return Err(Self::err_invalid()),
        }.with_value(
            value.serialize(ValueSerializer::new())?
        ))
    }

    fn serialize_bool(self, _: bool) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_i8(self, _: i8) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_i16(self, _: i16) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_i32(self, _: i32) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_i64(self, _: i64) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_u8(self, _: u8) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_u16(self, _: u16) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_u32(self, _: u32) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_u64(self, _: u64) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_f32(self, _: f32) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_f64(self, _: f64) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_char(self, _: char) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_str(self, _: &str) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_some<T>(self, _: &T) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + ser::Serialize
    { Err(Self::err_invalid()) }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_unit_variant( self,
        _: &'static str,
        _: u32, _: &'static str,
    ) -> Result<Self::Ok, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_newtype_struct<T>( self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + ser::Serialize
    {
        self.serialize_newtype_variant("", 0, name, value)
    }

    fn serialize_seq(self, _: Option<usize>)
    -> Result<Self::SerializeSeq, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_tuple(self, _: usize)
    -> Result<Self::SerializeTuple, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_tuple_variant( self,
        _: &'static str,
        _: u32, _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_map(self, _: Option<usize>)
    -> Result<Self::SerializeMap, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_struct( self, _: &'static str, _: usize)
    -> Result<Self::SerializeStruct, Self::Error>
    { Err(Self::err_invalid()) }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error>
    { Err(Self::err_invalid()) }

}

struct ValueSerializer(());

impl ValueSerializer {
    fn new() -> Self {
        Self(())
    }
}

impl ser::Serializer for ValueSerializer {
    type Ok = Option<Value>;
    type Error = Error;

    type SerializeSeq = TableArraySerializer<TrivialFinisher>;
    type SerializeTuple = TableArraySerializer<TrivialFinisher>;
    type SerializeTupleStruct = TableArraySerializer<TrivialFinisher>;
    type SerializeTupleVariant = TableArraySerializer<VariantFinisher>;
    type SerializeMap = TableSerializer<TrivialFinisher>;
    type SerializeStruct = TableSerializer<TrivialFinisher>;
    type SerializeStructVariant = TableSerializer<VariantFinisher>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Some(Value::Boolean(v)))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Some(Value::Integer(v)))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error>
    { self.serialize_i32(v.into()) }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error>
    { self.serialize_i32(v.into()) }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error>
    { self.serialize_i32(v.try_into().map_err(Error::custom)?) }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error>
    { self.serialize_i32(v.into()) }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error>
    { self.serialize_i32(v.into()) }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error>
    { self.serialize_i32(v.try_into().map_err(Error::custom)?) }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error>
    { self.serialize_i32(v.try_into().map_err(Error::custom)?) }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Some(Value::Float(v)))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error>
    { self.serialize_f64(v.into()) }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&String::from(v))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Some(Value::String(Str::from(v))))
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("bytes are not supported"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(None)
    }

    fn serialize_some<T>(self, value: &T)
    -> Result<Self::Ok, Self::Error>
    where T: ?Sized + ser::Serialize
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    fn serialize_unit_struct(self, _name: &'static str)
    -> Result<Self::Ok, Self::Error>
    {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error>
    {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>( self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + ser::Serialize
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>( self,
        _name: &'static str,
        _variant_index: u32, variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + ser::Serialize
    {
        Ok(Some(Value::Table([
            (variant, value.serialize(Self::new())?)
        ].into_iter().filter_map(|(k, v)| Some((k, v?))).collect())))
    }

    fn serialize_seq(self, _len: Option<usize>)
    -> Result<Self::SerializeSeq, Self::Error>
    {
        Ok(TableArraySerializer::new())
    }

    fn serialize_tuple(self, _len: usize)
    -> Result<Self::SerializeTuple, Self::Error>
    {
        Ok(TableArraySerializer::new())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error>
    {
        Ok(TableArraySerializer::new())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(TableArraySerializer::new_with_finisher(
            VariantFinisher(variant)
        ))
    }

    fn serialize_map(self, _len: Option<usize>)
    -> Result<Self::SerializeMap, Self::Error>
    {
        Ok(TableSerializer::new())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(TableSerializer::new())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(TableSerializer::new_with_finisher(VariantFinisher(variant)))
    }

}

trait ValueFinisher {
    fn finish(self, value: Value) -> Value;
}

struct TrivialFinisher;

impl ValueFinisher for TrivialFinisher {
    fn finish(self, value: Value) -> Value {
        value
    }
}

struct VariantFinisher(&'static str);

impl ValueFinisher for VariantFinisher {
    fn finish(self, value: Value) -> Value {
        Value::Table(Table::from_iter(
            [
                (self.0, value)
            ]
        ))
    }
}

struct TableArraySerializer<F: ValueFinisher> {
    f: F,
    array: ArrayBuilder<Value>,
}

impl TableArraySerializer<TrivialFinisher> {
    fn new() -> Self {
        Self::new_with_finisher(TrivialFinisher)
    }
}

impl<F: ValueFinisher> TableArraySerializer<F> {
    fn new_with_finisher(f: F) -> Self {
        Self { f, array: ArrayBuilder::new() }
    }
    fn push<V: ser::Serialize>(&mut self, value: V) -> Result<(), Error> {
        self.array.push_option(
            value.serialize(ValueSerializer::new())?
        );
        Ok(())
    }
    #[allow(clippy::same_name_method)]
    fn end(self) -> Value {
        self.f.finish(Value::Table(self.array.build()))
    }
}

struct TableSerializer<F: ValueFinisher> {
    f: F,
    table: TableBuilder<Value>,
    next_key: Option<Key>,
}

impl TableSerializer<TrivialFinisher> {
    fn new() -> Self {
        Self::new_with_finisher(TrivialFinisher)
    }
}

impl<F: ValueFinisher> TableSerializer<F> {
    fn new_with_finisher(f: F) -> Self {
        Self { f, table: TableBuilder::new(), next_key: None }
    }
    fn serialize_value_with_key<V>( &mut self,
        key: Key, value: &V,
    ) -> Result<(), Error>
    where V: ?Sized + ser::Serialize
    {
        let Some(value) = value.serialize(ValueSerializer::new())? else {
            // we can't store nil values, so we just drop it
            return Ok(());
        };
        self.table.insert(key, value);
        Ok(())
    }
    #[allow(clippy::same_name_method)]
    fn end(self) -> Value {
        self.f.finish(Value::Table(self.table.build()))
    }
}

impl<F: ValueFinisher> ser::SerializeSeq for TableArraySerializer<F> {
    type Ok = Option<Value>;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T)
    -> Result<(), Self::Error>
    where T: ?Sized + ser::Serialize
    {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Some(self.end()))
    }
}

impl<F: ValueFinisher> ser::SerializeTuple for TableArraySerializer<F> {
    type Ok = Option<Value>;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + ser::Serialize
    {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Some(self.end()))
    }
}

impl<F: ValueFinisher> ser::SerializeTupleStruct for TableArraySerializer<F> {
    type Ok = Option<Value>;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + ser::Serialize
    {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Some(self.end()))
    }
}

impl<F: ValueFinisher> ser::SerializeTupleVariant for TableArraySerializer<F> {
    type Ok = Option<Value>;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + ser::Serialize
    {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Some(self.end()))
    }
}

impl<F: ValueFinisher> ser::SerializeMap for TableSerializer<F> {
    type Ok = Option<Value>;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where T: ?Sized + ser::Serialize
    {
        let old_key = self.next_key.replace(
            key.serialize(ValueSerializer::new())?.try_into()? );
        assert!( old_key.is_none(),
            "consequent `serialize_key` calls" );
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + ser::Serialize
    {
        let Some(key) = self.next_key.take() else {
            panic!("missing `serialize_key` call");
        };
        self.serialize_value_with_key(key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Some(self.end()))
    }

}

impl<F: ValueFinisher> ser::SerializeStruct for TableSerializer<F> {
    type Ok = Option<Value>;
    type Error = Error;

    fn serialize_field<T>( &mut self,
        key: &'static str, value: &T,
    ) -> Result<(), Self::Error>
    where T: ?Sized + ser::Serialize
    {
        let key = Key::Name(Str::known(key));
        self.serialize_value_with_key(key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Some(self.end()))
    }
}

impl<F: ValueFinisher> ser::SerializeStructVariant for TableSerializer<F> {
    type Ok = Option<Value>;
    type Error = Error;

    fn serialize_field<T>( &mut self,
        key: &'static str, value: &T,
    ) -> Result<(), Self::Error>
    where T: ?Sized + ser::Serialize
    {
        <Self as ser::SerializeStruct>::serialize_field(self, key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Some(self.end()))
    }
}

#[cfg(test)]
mod test {

use serde::Serialize;

use crate::common::{
    TransparentRef,
    serde::OptionSerdeWrap,
};

use super::{Value, ValueSerializer};

#[test]
fn test_value_ser() {
    let value1: Option<Value> =
        ron::from_str::<OptionSerdeWrap<_>>(crate::test::RON_VALUE_1)
        .unwrap().into_inner();
    let value2 = value1.serialize(ValueSerializer::new()).unwrap();
    assert_eq!(value1, value2);
}

}

