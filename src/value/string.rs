use crate::table::u32_to_usize;

pub type Str = String;

pub(crate) fn str_from_len_read<R: std::io::Read>(
    len: u32,
    read: &mut R,
) -> Result<Str, crate::load::Error> {
    let len = u32_to_usize(len);
    let mut buffer = vec![0; len];
    read.read_exact(&mut buffer)?;
    Ok(String::from_utf8(buffer)?)
}

/*

use std::{
    rc::Rc,
    ops::Deref, borrow::Borrow,
    hash::Hash,
};

#[derive(Clone)]
pub(crate) enum Str {
    Name(&'static str),
    Data(Rc<str>),
}

impl std::fmt::Debug for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <str as std::fmt::Debug>::fmt(self, f)
    }
}

impl std::fmt::Display for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <str as std::fmt::Display>::fmt(self, f)
    }
}

impl<'a> From<&'a str> for Str {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self::Data(Rc::from(value))
    }
}

impl From<Rc<str>> for Str {
    #[inline]
    fn from(value: Rc<str>) -> Self {
        Self::Data(value)
    }
}

impl From<String> for Str {
    #[inline]
    fn from(value: String) -> Self {
        Self::from(Rc::from(value))
    }
}

impl Deref for Str {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        match self {
            Self::Name(s) => s,
            Self::Data(s) => s,
        }
    }
}

impl Borrow<str> for Str {
    #[inline]
    fn borrow(&self) -> &str {
        self
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
        str::cmp(self, other)
    }
}

impl Hash for Str {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        str::hash(self, state)
    }
}

*/
