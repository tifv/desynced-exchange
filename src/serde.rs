use std::{
    marker::PhantomData,
    borrow::Cow,
};

use serde::{
    Serialize, Serializer,
    Deserialize, Deserializer,
};

pub(crate) struct DeIdentifier<'de>(Cow<'de, str>);

impl<'de> std::ops::Deref for DeIdentifier<'de> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct DeIdentifierVisitor;

impl<'de> Deserialize<'de> for DeIdentifier<'de> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>
    {
        de.deserialize_identifier(DeIdentifierVisitor)
    }
}

impl<'de> serde::de::Visitor<'de> for DeIdentifierVisitor {
    type Value = DeIdentifier<'de>;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "an identifier")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(DeIdentifier(Cow::Owned(v.into())))
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(DeIdentifier(Cow::Borrowed(v)))
    }

}

pub(crate) mod option_some {
    use serde::{
        Serialize, Serializer,
        Deserialize, Deserializer,
    };

    pub(crate) fn serialize<T, S>(value: &Option<T>, ser: S)
    -> Result<S::Ok, S::Error>
    where T: Serialize, S: Serializer {
        match value.as_ref() {
            Some(value) => value.serialize(ser),
            None => unreachable!(),
        }
    }

    pub(crate) fn deserialize<'de, T, D>(de: D)
    -> Result<Option<T>, D::Error>
    where T: Deserialize<'de>, D: Deserializer<'de> {
        T::deserialize(de).map(Some)
    }

}

pub(crate) mod option_flat {
    use serde::{Serialize, Serializer};
    pub(crate) fn serialize<T, S>(value: &Option<T>, ser: S)
    -> Result<S::Ok, S::Error>
    where T: Serialize, S: Serializer {
        match value.as_ref() {
            Some(value) => value.serialize(ser),
            None => ser.serialize_none(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(
    transparent,
    bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>")
)]
pub(crate) struct FlatOption<T>(
    #[serde(serialize_with="option_flat::serialize")]
    pub Option<T>,
);

pub trait FieldNames {
    fn get_names() -> &'static [&'static str];
    fn find_static_name(name: &str) -> Option<(usize, &'static str)> {
        Self::get_names().iter().enumerate()
            .find(|(_, &n)| n == name)
            .map(|(index, &n)| (index, n))
    }
}

macro_rules! define_field_names {
    ($type_vis: vis $type_name:ident, [$($field:literal),*$(,)?]) => {
        $type_vis struct $type_name;
        impl $type_name {
            const FIELD_NAMES: &'static [&'static str] = &[$($field),*];
        }
        impl FieldNames for $type_name {
            fn get_names() -> &'static [&'static str] { Self::FIELD_NAMES }
        }
    };
}

pub struct ExtraFields<F: FieldNames, T> {
    f: PhantomData<F>,
    fields: Vec<(&'static str, bool, T)>,
}

impl<F: FieldNames, T> std::fmt::Debug for ExtraFields<F, T>
where T: std::fmt::Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("ExtraFields");
        for (name, _, value) in &self.fields {
            f.field(name, value);
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

struct ExtraFieldsSlice<'s, F: FieldNames, T> {
    f: PhantomData<F>,
    fields: &'s [(&'static str, bool, T)],
}

impl<F: FieldNames, T> ExtraFields<F, T> {
    #[inline]
    fn as_ref(&'_ self) -> ExtraFieldsSlice<'_, F, T> {
        ExtraFieldsSlice { f: self.f, fields: &self.fields }
    }
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.as_ref().len()
    }
}

impl<'s, F: FieldNames, T> Clone for ExtraFieldsSlice<'s, F, T> {
    fn clone(&self) -> Self {
        Self { f: PhantomData, fields: self.fields }
    }
}

impl<'s, F: FieldNames, T> ExtraFieldsSlice<'s, F, T> {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.fields.len()
    }
    fn peek_field_name(&self) -> Option<&'static str> {
        Some(self.fields.first()?.0)
    }
    fn serialize_into<S>(mut self, ser: &mut S::SerializeStruct)
    -> Result<(), S::Error>
    where T: Serialize, S: Serializer
    {
        use serde::ser::SerializeStruct as _;
        for name in F::get_names() {
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
        for (name, value) in self {
            ser.serialize_field(name, value)?;
        }
        Ok(())
    }
}

impl<'s, F: FieldNames, T> Iterator for ExtraFieldsSlice<'s, F, T> {
    type Item = (&'static str, &'s T);

    fn next(&mut self) -> Option<Self::Item> {
        let ((name, _, value), rest) = self.fields.split_first()?;
        self.fields = rest;
        Some((name, value))
    }
}

impl<'s, F: FieldNames, T> Serialize for ExtraFieldsSlice<'s, F, T>
where T: Serialize
{
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        ser.collect_map(self.clone())
    }
}

impl<F: FieldNames, T> ExtraFields<F, T> {
    fn search(&self, name: &str, extra: bool) -> Result<usize, usize> {
        use std::cmp::Ordering::{self, Less, Equal, Greater};
        for (index, (n, e, _)) in self.fields.iter().enumerate() {
            let cmp: Ordering = match (e, extra) {
                (false, true) => Less,
                (true, false) => Greater,
                (false, false) => {
                    let Some((i, j)) = Option::zip(
                        F::find_static_name(n).unzip().0,
                        F::find_static_name(name).unzip().0,
                    ) else {
                        unreachable!()
                    };
                    usize::cmp(&i, &j)
                },
                (true, true) => str::cmp(n, name),
            };
            match cmp {
                Less    => continue,
                Equal   => return Ok(index),
                Greater => return Err(index),
            }
        }
        Err(self.fields.len())
    }
    pub(crate) fn insert(&mut self, name: &str, value: T) -> Option<T> {
        use std::sync::Mutex;
        static EXTRA_FIELD_NAMES: Mutex<Vec<&'static str>> =
            Mutex::new(Vec::new());
        if let Some((_, name)) = F::find_static_name(name) {
            match self.search(name, false) {
                Ok(index) => {
                    Some(std::mem::replace(&mut self.fields[index].2, value))
                },
                Err(index) => {
                    self.fields.insert(index, (name, false, value));
                    None
                },
            }
        } else {
            let Ok(mut lock) = EXTRA_FIELD_NAMES.lock() else {
                panic!("the mutex should not be poisoned")
            };
            let mut lock_iter = lock.iter();
            let name = loop {
                let Some(&static_name) = lock_iter.next() else {
                    let name: &'static str = Box::leak(Box::<str>::from(name));
                    lock.push(name);
                    break name;
                };
                if static_name == name {
                    break static_name;
                }
            };
            std::mem::drop(lock);
            match self.search(name, true) {
                Ok(index) => {
                    Some(std::mem::replace(&mut self.fields[index].2, value))
                },
                Err(index) => {
                    self.fields.insert(index, (name, true, value));
                    None
                },
            }
        }
    }
    pub(crate) fn serialize_into_struct<S>(&self, ser: &mut S::SerializeStruct)
    -> Result<(), S::Error>
    where T: Serialize, S: Serializer,
    {
        self.as_ref().serialize_into::<S>(ser)
    }
}

pub struct ExtraFieldsIntoIter<T>{
    fields: std::vec::IntoIter<(&'static str, bool, T)>
}

impl<F: FieldNames, T> IntoIterator for ExtraFields<F, T> {
    type Item = (&'static str, T);

    type IntoIter = ExtraFieldsIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        ExtraFieldsIntoIter { fields: self.fields.into_iter() }
    }
}

impl<T> Iterator for ExtraFieldsIntoIter<T> {
    type Item = (&'static str, T);

    fn next(&mut self) -> Option<Self::Item> {
        self.fields.next().map(|(name, _, value)| (name, value))
    }
}

