//! Communication
//! between `load::Load` and `load::Loader` and
//! between `dump::Dump` and `dump::Dumper`.

use std::borrow::Borrow;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Key<I: Borrow<i32>, S: Borrow<str>> {
    Index(I),
    Name(S),
}

pub type KeyRef<'s> = Key<i32, &'s str>;

impl<I: Borrow<i32>, S: Borrow<str>> Key<I, S> {
    #[inline]
    pub fn as_ref(&self) -> KeyRef<'_> {
        match *self {
            Self::Index(ref index) => KeyRef::Index(*index.borrow()),
            Self::Name(ref name) => KeyRef::Name(name.borrow()),
        }
    }
}

impl<I, S> crate::dump::DumpKey for Key<I, S>
where I: Borrow<i32>, S: Borrow<str>
{
    fn dump_key<DD: crate::dump::KeyDumper>(&self, dumper: DD)
    -> Result<DD::Ok, crate::dump::Error> {
        match self {
            Self::Index(index) => dumper.dump_integer(*index.borrow()),
            Self::Name(name) => dumper.dump_string(name.borrow()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TableItem<K, V> {
    Array(V),
    Assoc(AssocItem<K, V>),
}

impl<K, V> TableItem<K, V> {
    #[inline]
    pub fn as_ref(&self) -> TableItem<&K, &V> {
        match *self {
            Self::Array(ref value) => TableItem::Array(value),
            Self::Assoc(ref item) => TableItem::Assoc(item.as_ref()),
        }
    }
    #[inline]
    pub fn map_key_value<K1, V1, KF, VF>(self, keyf: KF, valuef: VF)
        -> TableItem<K1, V1>
    where KF: FnOnce(K) -> K1, VF: FnOnce(V) -> V1
    {
        match self {
            Self::Array(value) => TableItem::Array(valuef(value)),
            Self::Assoc(item) =>
                TableItem::Assoc(item.map_key_value(keyf, valuef)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AssocItem<K, V> {
    Dead{link: i32},
    Live{value: V, key: K, link: i32},
}

impl<K, V> AssocItem<K, V> {
    #[inline]
    pub fn as_ref(&self) -> AssocItem<&K, &V> {
        match *self {
            Self::Dead{link} => AssocItem::Dead{link},
            Self::Live{ref value, ref key, link} =>
                AssocItem::Live{value, key, link},
        }
    }
    #[inline]
    pub fn map_key_value<K1, V1, KF, VF>(self, keyf: KF, valuef: VF)
        -> AssocItem<K1, V1>
    where KF: FnOnce(K) -> K1, VF: FnOnce(V) -> V1
    {
        match self {
            Self::Dead{link} => AssocItem::Dead{link},
            Self::Live{value, key, link} =>
                AssocItem::Live{value: valuef(value), key: keyf(key), link},
        }
    }
}

pub trait TableSize {
    fn array_len(&self) -> u32;
    fn assoc_loglen(&self) -> Option<u16>;
    fn assoc_last_free(&self) -> u32;
}

pub const fn u32_to_usize(len: u32) -> usize {
    assert!({ const OK: bool = {
        let ok = u32::BITS <= usize::BITS;
        assert!(ok); ok
    }; OK});
    len as usize
}

#[inline]
pub const fn ilog2_ceil(len: usize) -> Option<u16> {
    //! Upper-rounded base 2 logarithm
    let Some(mut ilog2) = len.checked_ilog2() else {
        return None;
    };
    if ilog2 > len.trailing_zeros() {
        ilog2 += 1;
    }
    Some(ilog2 as u16)
}

#[inline]
pub const fn ilog2_exact(len: usize) -> Option<u16> {
    //! Base 2 logarithm. Returns `None` if `len` is not a power of two.
    let Some(ilog2) = len.checked_ilog2() else {
        return None;
    };
    if ilog2 > len.trailing_zeros() {
        return None;
    }
    Some(ilog2 as u16)
}

#[inline]
pub const fn iexp2(loglen: Option<u16>) -> u32 {
    let Some(loglen) = loglen else { return 0 };
    match 1_u32.checked_shl(loglen as u32) {
        Some(exp) if exp - 1 <= (i32::MAX as u32) => exp,
        _ => panic!("size should be addressable by i32"),
    }
}

