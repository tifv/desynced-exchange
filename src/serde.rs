use std::marker::PhantomData;

use serde::{
    Deserialize, Deserializer, de,
    Serialize, Serializer
};

use crate::{
    common::TransparentRef,
    string::{Str, SharedStr},
};

pub(crate) enum Identifier<'de>{
    Shared(SharedStr),
    Borrowed(&'de str),
}

impl<'de> std::fmt::Debug for Identifier<'de> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <str as std::fmt::Debug>::fmt(self, f)
    }
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


pub(crate) struct IgnoredValue;

impl<'de> Deserialize<'de> for IgnoredValue {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        de.deserialize_ignored_any(IgnoringVisitor)
    }
}

pub(crate) struct IgnoringVisitor;

impl<'de> de::Visitor<'de> for IgnoringVisitor {
    type Value = IgnoredValue;
    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "nothing")
    }

    fn visit_i32<E>(self, _v: i32) -> Result<Self::Value, E>
    where E: de::Error
    { Ok(IgnoredValue) }
    delegate_to_i32!();

    fn visit_f64<E>(self, _v: f64) -> Result<Self::Value, E>
    where E: de::Error
    { Ok(IgnoredValue) }
    delegate_to_f64!();

    fn visit_bool<E>(self, _v: bool) -> Result<Self::Value, E>
    where E: de::Error
    { Ok(IgnoredValue) }

    fn visit_str<E>(self, _v: &str) -> Result<Self::Value, E>
    where E: de::Error
    { Ok(IgnoredValue) }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where E: de::Error
    { Ok(IgnoredValue) }

    fn visit_some<D>(self, _de: D) -> Result<Self::Value, D::Error>
    where D: Deserializer<'de>
    { Ok(IgnoredValue) }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where E: de::Error
    { Ok(IgnoredValue) }

    fn visit_newtype_struct<D>(self, _de: D) -> Result<Self::Value, D::Error>
    where D: Deserializer<'de>
    { Ok(IgnoredValue) }

    fn visit_seq<A>(self, _seq: A) -> Result<Self::Value, A::Error>
    where A: de::SeqAccess<'de>
    { Ok(IgnoredValue) }

    fn visit_map<A>(self, _map: A) -> Result<Self::Value, A::Error>
    where A: de::MapAccess<'de>
    { Ok(IgnoredValue) }

    fn visit_enum<A>(self, _data: A) -> Result<Self::Value, A::Error>
    where A: de::EnumAccess<'de>
    { Ok(IgnoredValue) }

}


pub(crate) struct PairVisitor<A, B>(PhantomData<(A, B)>);

impl<A, B> PairVisitor<A, B> {
    pub fn new() -> Self { Self(PhantomData) }
}

impl<'de, A, B> de::Visitor<'de> for PairVisitor<A, B>
where A: Deserialize<'de>, B: Deserialize<'de>
{
    type Value = (A, B);
    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "a pair of values")
    }
    fn visit_seq<S>(self, mut seq: S) -> Result<(A, B), S::Error>
    where S: de::SeqAccess<'de>
    {
        let custom_err = <S::Error as de::Error>::custom;
        let Some(a) = seq.next_element()? else {
            return Err(custom_err("missing first element of the pair")); };
        let Some(b) = seq.next_element()? else {
            return Err(custom_err("missing second element of the pair")); };
        if seq.next_element::<IgnoredValue>()?.is_some() {
            return Err(custom_err("unexpected third element")); };
        Ok((a, b))
    }
}


macro_rules! delegate_to_i32 {
    () => {

    fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
    where E: de::Error
    { self.visit_i32(i32::from(v)) }

    fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
    where E: de::Error
    { self.visit_i32(i32::from(v)) }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where E: de::Error
    { self.visit_i32(i32::try_from(v).map_err(E::custom)?) }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where E: de::Error
    { self.visit_i32(i32::from(v)) }

    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where E: de::Error
    { self.visit_i32(i32::from(v)) }

    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where E: de::Error
    { self.visit_i32(i32::try_from(v).map_err(E::custom)?) }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where E: de::Error
    { self.visit_i32(i32::try_from(v).map_err(E::custom)?) }

    };
}

pub(crate) use delegate_to_i32 as delegate_to_i32;

macro_rules! delegate_to_f64 {
    () => {

    fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
    where E: de::Error
    { self.visit_f64(f64::try_from(v).map_err(E::custom)?) }

    };
}

pub(crate) use delegate_to_f64 as delegate_to_f64;


pub trait DeserializeOption<'de> : Deserialize<'de> {
    fn deserialize_option<D>(de: D)
    -> Result<Option<Self>, D::Error>
    where D: Deserializer<'de>
    {
        Option::<Self>::deserialize(de)
    }
}

macro_rules! forward_de_to_de_option {
    ($type:ty) => {

    impl<'de> ::serde::Deserialize<'de> for $type
    where $type: $crate::serde::DeserializeOption<'de>
    {
        fn deserialize<D>(de: D)
        -> ::std::result::Result<Self, D::Error>
        where D: ::serde::Deserializer<'de>
        {
            use ::std::result::Result::{Ok, Err};
            use ::std::option::Option::{None, Some};
            use ::serde::de::Error as _;
            let value_option = <Self as $crate::serde::DeserializeOption>
                ::deserialize_option(de)?;
            Ok(match value_option {
                Some(value) => value,
                None => return Err(D::Error::custom(
                    "expected some value, not None" ))
            })
        }
    }

    };
}

pub(crate) use forward_de_to_de_option as forward_de_to_de_option;

pub trait SerializeOption : Serialize {
    fn serialize_option<S>(this: Option<&Self>, ser: S)
    -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        this.serialize(ser)
    }
}

macro_rules! impl_flat_se_option {
    ($type:ty) => {

    impl $crate::serde::SerializeOption for $type {
        fn serialize_option<S>(this: ::std::option::Option<&Self>, ser: S)
        -> ::std::result::Result<S::Ok, S::Error>
        where S: ::serde::Serializer
        {
            use ::std::option::Option::{None, Some};
            match this {
                None => ser.serialize_none(),
                Some(value) =>
                    <Self as ::serde::Serialize>::serialize(value, ser),
            }
        }
    }

    };
}

pub(crate) use impl_flat_se_option as impl_flat_se_option;

#[repr(transparent)]
pub struct OptionSerdeWrap<V>(
    pub Option<V>
);

impl<V> AsRef<Option<V>> for OptionSerdeWrap<V> {
    fn as_ref(&self) -> &Option<V> { &self.0 }
}

// SAFETY: `Self` is `repr(transparent)` over `Target`
unsafe impl<V> TransparentRef for OptionSerdeWrap<V> {
    type Target = Option<V>;
}

impl<'de, V> Deserialize<'de> for OptionSerdeWrap<V>
where V: DeserializeOption<'de>
{
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>
    {
        Ok(Self(V::deserialize_option(de)?))
    }
}

impl<V> Serialize for OptionSerdeWrap<V>
where V: SerializeOption
{
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        V::serialize_option(self.0.as_ref(), ser)
    }
}

#[repr(transparent)]
pub struct OptionRefSerdeWrap<'v, V>(
    pub Option<&'v V>
);

impl<'v, V> AsRef<Option<&'v V>> for OptionRefSerdeWrap<'v, V> {
    fn as_ref(&self) -> &Option<&'v V> { &self.0 }
}

// SAFETY: `Self` is `repr(transparent)` over `Target`
unsafe impl<'v, V> TransparentRef for OptionRefSerdeWrap<'v, V> {
    type Target = Option<&'v V>;
}

impl<'v, V> Serialize for OptionRefSerdeWrap<'v, V>
where V: SerializeOption
{
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        V::serialize_option(self.0, ser)
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

