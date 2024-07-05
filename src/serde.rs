use std::marker::PhantomData;

use serde::{
    Deserialize, de::{DeserializeSeed, Visitor},
    Deserializer, de::EnumAccess,
    de::value::StrDeserializer,
    forward_to_deserialize_any,
    Serialize, Serializer
};

use crate::string::{Str, SharedStr};

pub enum Identifier<'de>{
    Shared(SharedStr),
    Borrowed(&'de str),
}

impl<'de> std::ops::Deref for Identifier<'de> {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Shared(ref s) => s,
            Self::Borrowed(s) => s,
        }
    }
}

impl<'de> std::borrow::Borrow<str> for Identifier<'de> {
    fn borrow(&self) -> &str {
        self
    }
}

impl<'de> AsRef<str> for Identifier<'de> {
    fn as_ref(&self) -> &str {
        self
    }
}

impl<'de> From<Identifier<'de>> for Str {
    fn from(value: Identifier<'de>) -> Self {
        match value {
            Identifier::Shared(string) =>
                Self::Shared(string),
            Identifier::Borrowed(string) =>
                Self::from(string),
        }
    }
}

impl<'de> Deserialize<'de> for Identifier<'de> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>
    {
        de.deserialize_identifier(IdentifierVisitor)
    }
}

struct IdentifierVisitor;

impl<'de> serde::de::Visitor<'de> for IdentifierVisitor {
    type Value = Identifier<'de>;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "an identifier")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Identifier::Shared(SharedStr::from(v)))
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Identifier::Borrowed(v))
    }

}


pub(crate) mod option_none {
    use serde::{
        de::Visitor, Deserializer,
        Serializer,
    };

    pub(crate) fn serialize<S>(ser: S)
    -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        ser.serialize_none()
    }

    pub(crate) fn deserialize<'de, D>(de: D)
    -> Result<(), D::Error>
    where D: Deserializer<'de>
    {
        de.deserialize_option(NoneVisitor)
    }

    struct NoneVisitor;

    impl<'de> Visitor<'de> for NoneVisitor {
        type Value = ();
        fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(fmt, "a None")
        }
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where E: serde::de::Error
        {
            Ok(())
        }
    }

}

pub(crate) mod option_some {
    use serde::{
        Serialize, Serializer,
        Deserialize, Deserializer,
    };

    pub(crate) fn serialize<T, S>(value: &Option<T>, ser: S)
    -> Result<S::Ok, S::Error>
    where T: Serialize, S: Serializer
    {
        match value.as_ref() {
            Some(value) => value.serialize(ser),
            None => unreachable!(),
        }
    }

    pub(crate) fn deserialize<'de, T, D>(de: D)
    -> Result<Option<T>, D::Error>
    where T: Deserialize<'de>, D: Deserializer<'de>
    {
        T::deserialize(de).map(Some)
    }

}

#[derive(Deserialize, Serialize)]
#[serde(untagged, remote = "Option")]
enum FlatOptionDef<T> {
    #[serde(with="option_none")]
    None,
    Some(T),
}

#[derive(Deserialize, Serialize)]
#[serde(transparent)]
#[repr(transparent)]
pub(crate) struct FlatOption<T>(
    #[serde( with = "FlatOptionDef",
        bound(deserialize="T: Deserialize<'de>", serialize="T: Serialize") )]
    pub Option<T>,
);

impl<T> FlatOption<T> {
    pub(crate) fn into_inner(self) -> Option<T> { self.0 }
}

#[repr(transparent)]
pub(crate) struct FlatEnumOption<T>(
    pub Option<T>,
);

impl<T> FlatEnumOption<T> {
    pub(crate) fn into_inner(self) -> Option<T> { self.0 }
}

impl<'de, T> Deserialize<'de> for FlatEnumOption<T>
where T: Deserialize<'de>
{
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de>
    {
        Ok(Self(
            de.deserialize_enum( "Option", &["None"],
                FlatEnumOptionVisitor::new() )?
        ))
    }
}

struct FlatEnumOptionVisitor<T>(PhantomData<T>);

impl<T> FlatEnumOptionVisitor<T> {
    fn new() -> Self { Self(PhantomData) }
}

impl<T> Default for FlatEnumOptionVisitor<T> {
    fn default() -> Self { Self::new() }
}

impl<'de, T> Visitor<'de> for FlatEnumOptionVisitor<T>
where T: Deserialize<'de>
{
    type Value = Option<T>;
    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "something")
    }
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where E: serde::de::Error
    {
        Ok(None)
    }
    fn visit_some<D>(self, de: D) -> Result<Self::Value, D::Error>
    where D: serde::Deserializer<'de>
    {
        Ok(Some(T::deserialize(de)?))
    }
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where A: serde::de::EnumAccess<'de>
    {
        let (name, value) = data.variant::<Identifier>()?;
        if name.as_ref() == "None" {
            return Ok(None);
        }
        Ok(Some(T::deserialize(FlatEnumOptionSome {
            variant: name,
            value,
        })?))
    }
}

struct FlatEnumOptionSome<'de, VA>
where VA: serde::de::VariantAccess<'de>
{
    variant: Identifier<'de>,
    value: VA,
}

impl<'de, VA> Deserializer<'de> for FlatEnumOptionSome<'de, VA>
where VA: serde::de::VariantAccess<'de>
{
    type Error = VA::Error;

    fn deserialize_enum<V>( self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
    {
        visitor.visit_enum(self)
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de> {
        visitor.visit_enum(self)
    }

    forward_to_deserialize_any! {
        bool
        i8 i16 i32 i64
        u8 u16 u32 u64
        f32 f64
        char str string
        bytes byte_buf
        option
        unit unit_struct newtype_struct
        seq tuple tuple_struct
        map struct
        identifier ignored_any
    }
}

impl<'de, VA> EnumAccess<'de> for FlatEnumOptionSome<'de, VA>
where VA: serde::de::VariantAccess<'de>
{
    type Error = VA::Error;
    type Variant = VA;

    fn variant_seed<V>(self, seed: V)
    -> Result<(V::Value, Self::Variant), Self::Error>
    where V: DeserializeSeed<'de>
    {
        Ok((
            V::deserialize(seed, StrDeserializer::new(self.variant.as_ref()))?,
            self.value,
        ))
    }
}

impl<T> Serialize for FlatEnumOption<T>
where T: Serialize
{
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        match self.0 {
            None => ser.serialize_unit_variant("Option", 0, "None"),
            Some(ref value) => value.serialize(ser),
        }
    }
}


#[derive(Clone)]
pub enum ExtraFieldName<S>
where S: std::borrow::Borrow<str>
{
    Known(usize, &'static str),
    Unknown(S),
}

impl<S> AsRef<str> for ExtraFieldName<S>
where S: std::borrow::Borrow<str> + AsRef<str>
{
    fn as_ref(&self) -> &str {
        match self {
            Self::Known(_, string) => string,
            Self::Unknown(string) => string.as_ref(),
        }
    }
}

#[allow(clippy::use_self)]
impl From<ExtraFieldName<Str>> for Str {
    fn from(value: ExtraFieldName<Str>) -> Self {
        match value {
            ExtraFieldName::Known(_, string) => Str::known(string),
            ExtraFieldName::Unknown(string) => string,
        }
    }
}

impl<S, Z> PartialEq<ExtraFieldName<Z>> for ExtraFieldName<S>
where
    S: std::borrow::Borrow<str>,
    Z: std::borrow::Borrow<str>,
{
    fn eq(&self, other: &ExtraFieldName<Z>) -> bool {
        use ExtraFieldName::{Known, Unknown};
        match (self, other) {
            (Known(..),   Unknown(..)) |
            (Unknown(..), Known(..)) => false,
            (Known(i, _), Known(j, _)) =>
                usize::eq(i, j),
            (Unknown(u),  Unknown(v)) =>
                str::eq(u.borrow(), v.borrow()),
        }
    }
}

impl<S> Eq for ExtraFieldName<S>
where S: std::borrow::Borrow<str> {}

impl<S, Z> PartialOrd<ExtraFieldName<Z>> for ExtraFieldName<S>
where
    S: std::borrow::Borrow<str>,
    Z: std::borrow::Borrow<str>,
{
    fn partial_cmp(&self, other: &ExtraFieldName<Z>)
    -> Option<std::cmp::Ordering> {
        use ExtraFieldName::{Known, Unknown};
        use std::cmp::Ordering as Order;
        Some(match (self, other) {
            (Known(..), Unknown(..)) => Order::Less,
            (Unknown(..), Known(..)) => Order::Greater,
            (Known(i, _), Known(j, _)) => usize::cmp(i, j),
            (Unknown(u), Unknown(v)) =>
                str::cmp(u.borrow(), v.borrow()),
        })
    }
}

impl<S> Ord for ExtraFieldName<S>
where S: std::borrow::Borrow<str>
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        Self::partial_cmp(self, other).unwrap()
    }
}

pub trait FieldNames {
    fn get_names() -> &'static [&'static str];
    fn find_static_name<S>(name: S) -> ExtraFieldName<S>
    where S: std::borrow::Borrow<str>
    {
        Self::get_names().iter().enumerate()
            .find(|(_, &n)| n == name.borrow())
            .map_or_else(
                || ExtraFieldName::Unknown(name),
                |(index, &n)| ExtraFieldName::Known(index, n),
            )
    }
}

macro_rules! define_field_names {
    ($type_vis: vis $type_name:ident, [$($field:literal),*$(,)?]) => {
        $type_vis struct $type_name;
        impl $type_name {
            const FIELD_NAMES: &'static [&'static str] = &[$($field),*];
        }
        impl $crate::serde::FieldNames for $type_name {
            fn get_names() -> &'static [&'static str] { Self::FIELD_NAMES }
        }
    };
}

pub(crate) use define_field_names as define_field_names;

pub struct ExtraFields<F: FieldNames, T> {
    f: PhantomData<F>,
    fields: Vec<(ExtraFieldName<Str>, T)>,
}

impl<F: FieldNames, T> std::fmt::Debug for ExtraFields<F, T>
where T: std::fmt::Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("ExtraFields");
        for (name, value) in &self.fields {
            f.field(name.as_ref(), value);
        }
        f.finish()
    }
}

impl<F: FieldNames, T> Clone for ExtraFields<F, T>
where T: Clone,
{
    fn clone(&self) -> Self {
        Self { f: PhantomData, fields: self.fields.clone() }
    }
}

impl<F: FieldNames, T> Default for ExtraFields<F, T> {
    fn default() -> Self {
        Self { f: PhantomData, fields: Vec::new() }
    }
}

impl<F: FieldNames, T> ExtraFields<F, T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.fields.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
    pub fn get(&self, name: &str) -> Option<&T> {
        for (n, v) in &self.fields {
            if n.as_ref() == name {
                return Some(v);
            }
        }
        None
    }
    fn search<S>(&self, name: &ExtraFieldName<S>) -> Result<usize, usize>
    where S: std::borrow::Borrow<str>
    {
        use std::cmp::Ordering as Order;
        for (index, (n, _)) in self.fields.iter().enumerate() {
            match ExtraFieldName::partial_cmp(n, name) {
                Some(Order::Less   ) => continue,
                Some(Order::Equal  ) => return Ok(index),
                Some(Order::Greater) => return Err(index),
                None => unreachable!(),
            }
        }
        Err(self.fields.len())
    }
    pub fn insert(&mut self, name: Str, value: T) -> Option<T> {
        let name = F::find_static_name(name);
        match self.search(&name) {
            Ok(index) => {
                Some(std::mem::replace(&mut self.fields[index].1, value))
            },
            Err(index) => {
                self.fields.insert(index, (name, value));
                None
            },
        }
    }
    pub fn take<S>(&mut self, name: S) -> Option<T>
    where S: std::borrow::Borrow<str>
    {
        let name = F::find_static_name(name);
        match self.search(&name) {
            Ok(index) => Some(self.fields.remove(index).1),
            Err(_) => None,
        }
    }
}

impl<F: FieldNames, T> ExtraFields<F, T> {
    pub(crate) fn consume_next_value<'de, A>(&mut self, name: Str, map: &mut A)
    -> Result<(), A::Error>
    where T: Deserialize<'de>, A: serde::de::MapAccess<'de>
    {
        match name.as_ref() {
            "extra" => {
                map.next_value_seed(ExtraFieldsDeserializeSeed { inner: self })?;
            }
            _ => {
                self.insert(name, map.next_value()?);
            },
        }
        Ok(())
    }
}

pub(crate) struct ExtraFieldsDeserializeSeed<'s, F: FieldNames, T> {
    inner: &'s mut ExtraFields<F, T>,
}

impl<'de, 's, F: FieldNames, T> DeserializeSeed<'de>
    for ExtraFieldsDeserializeSeed<'s, F, T>
where T: Deserialize<'de>
{
    type Value = ();

    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where D: serde::Deserializer<'de>
    {
        de.deserialize_map(self)
    }
}

impl<'de, 's, F: FieldNames, T> Visitor<'de>
    for ExtraFieldsDeserializeSeed<'s, F, T>
where T: Deserialize<'de>
{
    type Value = ();

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "a map of extra fields")
    }

    fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
    where A: serde::de::MapAccess<'de>,
    {
        while let Some((key, value)) = map.next_entry()? {
            self.inner.insert(key, value);
        }
        Ok(())
    }

}

impl<F: FieldNames, T> ExtraFields<F, T> {
    #[inline]
    fn as_ref(&'_ self) -> ExtraFieldsSlice<'_, F, T> {
        ExtraFieldsSlice { f: self.f, fields: &self.fields }
    }
}

struct ExtraFieldsSlice<'s, F: FieldNames, T> {
    f: PhantomData<F>,
    fields: &'s [(ExtraFieldName<Str>, T)],
}

impl<'s, F: FieldNames, T> Clone for ExtraFieldsSlice<'s, F, T> {
    fn clone(&self) -> Self {
        Self { f: PhantomData, fields: self.fields }
    }
}

impl<'s, F: FieldNames, T> Iterator for ExtraFieldsSlice<'s, F, T> {
    type Item = (&'s ExtraFieldName<Str>, &'s T);

    fn next(&mut self) -> Option<Self::Item> {
        let ((name, value), rest) = self.fields.split_first()?;
        self.fields = rest;
        Some((name, value))
    }
}

impl<'s, F: FieldNames, T> ExtraFieldsSlice<'s, F, T> {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.fields.len()
    }
    fn peek_field_name(&self) -> Option<&str> {
        Some(self.fields.first()?.0.as_ref())
    }
    fn serialize_into_struct<S>(mut self, ser: &mut S::SerializeStruct)
    -> Result<(), S::Error>
    where T: Serialize, S: Serializer
    {
        use serde::ser::SerializeStruct as _;
        for name in F::get_names().iter().copied() {
            let field_name = self.peek_field_name();
            if field_name == Some(name) {
                let Some((_, value)) = self.next() else {
                    unreachable!()
                };
                ser.serialize_field(name, value)?;
            } else {
                ser.skip_field(name)?;
            }
        }
        if self.len() > 0 {
            ser.serialize_field( "extra",
                &ExtraFieldsSerialize { inner: self } )?;
        }
        Ok(())
    }
}

struct ExtraFieldsSerialize<'s, F: FieldNames, T> {
    inner: ExtraFieldsSlice<'s, F, T>,
}

impl<'s, F: FieldNames, T> Serialize for ExtraFieldsSerialize<'s, F, T>
where T: Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        serializer.collect_map(
            self.inner.clone().map(|(k, v)| (k.as_ref(), v)) )
    }
}

impl<F: FieldNames, T> ExtraFields<F, T> {
    pub(crate) fn serialize_into_struct<S>(&self, ser: &mut S::SerializeStruct)
    -> Result<(), S::Error>
    where T: Serialize, S: Serializer,
    {
        self.as_ref().serialize_into_struct::<S>(ser)
    }
}

pub struct ExtraFieldsIntoIter<T>{
    fields: std::vec::IntoIter<(ExtraFieldName<Str>, T)>
}

impl<F: FieldNames, T> IntoIterator for ExtraFields<F, T> {
    type Item = (Str, T);

    type IntoIter = ExtraFieldsIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        ExtraFieldsIntoIter { fields: self.fields.into_iter() }
    }
}

impl<T> Iterator for ExtraFieldsIntoIter<T> {
    type Item = (Str, T);

    fn next(&mut self) -> Option<Self::Item> {
        self.fields.next().map(|(name, value)| (name.into(), value))
    }
}


pub(crate) struct VecSeed<T> (
    pub Vec<T>
);

impl<'de, T> DeserializeSeed<'de> for VecSeed<T>
where T: Deserialize<'de>
{
    type Value = Vec<T>;

    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where D: serde::Deserializer<'de>
    {
        de.deserialize_seq(self)
    }
}

impl<'de, T> Visitor<'de> for VecSeed<T>
where T: Deserialize<'de>
{
    type Value = Vec<T>;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "a sequence")
    }

    fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de>
    {
        while let Some(value) = seq.next_element()? {
            self.0.push(value);
        }
        Ok(self.0)
    }

}
