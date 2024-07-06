//! Communication
//! between `load::Load` and `load::Loader` and
//! between `dump::Dump` and `dump::Dumper`.

use serde::{Deserialize, Serialize};

use crate::common::LogSize;

#[derive(Debug, Clone)]
#[allow(clippy::exhaustive_enums)]
pub enum TableItem<K, V> {
    Array(V),
    Assoc(AssocItem<K, V>),
}

#[derive(Clone, Deserialize, Serialize)]
#[allow(clippy::exhaustive_enums)]
pub enum AssocItem<K, V> {
    Dead { link: i32 },
    Live { key: K, value: Option<V>, link: i32 },
}

impl<K, V> AssocItem<K, V>
where K: Clone
{
    #[inline]
    pub fn as_value_ref(&self) -> AssocItem<K, &V> {
        match *self {
            Self::Dead { link } => AssocItem::Dead { link },
            Self::Live { ref key, ref value, link } =>
                AssocItem::Live {
                    key: K::clone(key),
                    value: value.as_ref(),
                    link
                },
        }
    }
}

impl<K, V> std::fmt::Debug for AssocItem<K, V>
where K: std::fmt::Debug, V: std::fmt::Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let link = match self {
            Self::Dead { link } => {
                f.write_str("×")?;
                *link
            },
            Self::Live { value, key, link } => {
                key.fmt(f)?;
                f.write_str(": ")?;
                value.fmt(f)?;
                *link
            },
        };
        if link != 0 {
            f.write_str(" *")?;
            link.fmt(f)?;
        }
        Ok(())
    }
}

pub trait TableSize {
    #[must_use]
    fn array_len(&self) -> u32;
    #[must_use]
    fn assoc_loglen(&self) -> Option<LogSize>;
    #[must_use]
    fn assoc_last_free(&self) -> u32;
}

