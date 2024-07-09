use std::rc::Rc;

use serde::{Deserialize, Serialize};

pub type SharedStr = Rc<str>;

#[derive(Clone)]
pub enum Str {
    Static(&'static str),
    Shared(SharedStr),
}

impl std::ops::Deref for Str {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Static(s) => s,
            Self::Shared(ref s) => s,
        }
    }
}

impl std::borrow::Borrow<str> for Str {
    fn borrow(&self) -> &str {
        self
    }
}

impl AsRef<str> for Str {
    fn as_ref(&self) -> &str {
        self
    }
}

impl<'s> From<&'s str> for Str {
    fn from(value: &'s str) -> Self {
        Self::new(value)
    }
}

impl Str {
    #[must_use]
    #[inline]
    fn new(string: &str) -> Self {
        Self::Shared(SharedStr::from(string))
    }
    #[must_use]
    #[inline]
    pub fn known(string: &'static str) -> Self {
        Self::Static(string)
    }
    #[must_use]
    #[inline]
    pub fn shared(string: SharedStr) -> Self {
        Self::Shared(string)
    }
}

impl std::fmt::Debug for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <str as std::fmt::Debug>::fmt(self, f)
    }
}

impl PartialEq for Str {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        <str as PartialEq<str>>::eq(self, other)
    }
}

impl Eq for Str {}

impl PartialOrd for Str {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(Self::cmp(self, other))
    }
}

impl Ord for Str {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        <str as Ord>::cmp(self, other)
    }
}

impl std::hash::Hash for Str {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        <str as std::hash::Hash>::hash(self, state)
    }
}

impl<'de> Deserialize<'de> for Str {
    #[inline]
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de>
    {
        Ok(Self::new(<&'de str as Deserialize>::deserialize(de)?))
    }
}

impl Serialize for Str {
    #[inline]
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer
    {
        ser.serialize_str(self)
    }
}

