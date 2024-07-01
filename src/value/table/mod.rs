use std::marker::PhantomData;
use std::collections::BTreeMap as SortedMap;

mod assoc;

use serde::{Deserialize, Serialize};

use crate::{
    table::{
        KeyRef,
        TableItem,
        iexp2, u32_to_usize, ilog2_ceil,
    },
    dump::Dump,
    load,
};

use self::assoc::{
    Table as AssocTable, Item as AssocItem,
    TableLoadBuilder as AssocTableLoadBuilder,
    TableDumpBuilder as AssocTableDumpBuilder,
};

pub use crate::table::KeyOwned as Key;

#[derive(Clone)]
pub struct Table<V> {
    array: Vec<Option<V>>,
    assoc: AssocTable<V>,
}

impl<V> std::fmt::Debug for Table<V>
where V: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt;
        struct NilEntry;
        impl fmt::Debug for NilEntry {
            fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
                Ok(())
            }
        }
        let mut map = f.debug_set();
        for item in self.dump_iter() {
            let Some(item) = item else {
                map.entry(&NilEntry);
                continue;
            };
            match item {
                TableItem::Array(value) => map.entry(value),
                TableItem::Assoc(item) => map.entry(&item),
            };
        }
        map.finish()
    }
}

impl<V> Table<V> {

    #[must_use]
    pub fn dump_iter(&self) -> TableDumpIter<'_, V>
    {
        TableDumpIter {
            array: Some(self.array.iter()),
            assoc: self.assoc.dump_iter(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        //! May be greater than the actual number of non-nil entries
        usize::checked_add(
            self.array_len(),
            u32_to_usize(iexp2(self.assoc_loglen())),
        ).unwrap()
    }

    #[must_use]
    pub fn array_len(&self) -> usize {
        self.array.len()
    }

    #[must_use]
    pub fn assoc_loglen(&self) -> Option<u16> {
        self.assoc.loglen()
    }

}

pub struct TableArrayBuilder<V> {
    array: Vec<Option<V>>,
}

impl<V> TableArrayBuilder<V> {

    pub fn finish(self) -> Table<V> {
        let Self { array } = self;
        Table { array, assoc: AssocTable::new(None) }
    }

}

impl<V> FromIterator<Option<V>> for TableArrayBuilder<V> {
    fn from_iter<T: IntoIterator<Item = Option<V>>>(iter: T) -> Self {
        Self { array: Vec::from_iter(iter) }
    }
}

pub(crate) struct TableLoadBuilder<V> {
    array: Vec<Option<V>>,
    assoc: AssocTableLoadBuilder<V>,
}

impl<V> TableLoadBuilder<V> {

    #[must_use]
    pub(crate) fn new(array_len: u32, assoc_loglen: Option<u16>) -> Self {
        let mut array = Vec::with_capacity(u32_to_usize(array_len));
        array.resize_with(u32_to_usize(array_len), || None);
        Self {
            array,
            assoc: AssocTableLoadBuilder::new(assoc_loglen),
        }
    }

    pub(crate) fn finish<E: load::Error>(self) -> Result<Table<V>, E> {
        let Self { array, assoc } = self;
        let assoc = assoc.finish::<E>()?;
        Ok(Table { array, assoc })
    }

    pub(crate) fn array_insert<E: load::Error>( &mut self,
        index: u32, value: V,
    ) -> Result<(), E> {
        //! `index` is 0-based
        #![allow(clippy::unnecessary_wraps)]
        let index = u32_to_usize(index);
        let old_value = self.array[index].replace(value);
        assert!(old_value.is_none());
        Ok(())
    }

    pub(crate) fn assoc_insert<E: load::Error>( &mut self,
        index: u32, item: AssocItem<V>,
    ) -> Result<(), E> {
        //! `index` is 0-based
        match &item {
            &AssocItem::Live { key: Key::Index(key), .. }
                if key > 0 && (key as u32) <= (self.array.len() as u32)
                => return Err(E::from(
                    "this assoc item should belong to array part" )),
            _ => (),
        }
        self.assoc.insert(index, item);
        Ok(())
    }

    pub(crate) fn set_last_free(&mut self, last_free: u32) {
        self.assoc.set_last_free(last_free)
    }

}

pub struct TableDumpBuilder<V> {
    array: Vec<Option<V>>,
    assoc: AssocTableDumpBuilder<V>,
}

impl<V> TableDumpBuilder<V> {

    #[must_use]
    pub fn new(
        array_len: Option<u32>,
        assoc_loglen: Option<u16>,
    ) -> Self {
        Self {
            array: match array_len {
                Some(len) => Vec::with_capacity(u32_to_usize(len)),
                None => Vec::new(),
            },
            assoc: AssocTableDumpBuilder::new(assoc_loglen),
        }
    }

    #[must_use]
    pub fn finish(self) -> Table<V> {
        let Self { array, assoc } = self;
        Table { array, assoc: assoc.finish() }
    }

    pub fn array_extend<I>(&mut self, iter: I)
    where I: IntoIterator<Item=Option<V>>
    {
        self.array.extend(iter)
    }

    pub fn assoc_insert<K: Into<Key>>(&mut self, key: K, value: Option<V>) {
        self.assoc.insert(key.into(), value)
    }

    pub fn assoc_insert_dead<K: Into<Key>>(&mut self, key: K) {
        self.assoc.insert_dead(key.into())
    }

}

pub struct TableMapBuilder<V> {
    map: SortedMap<Key, Option<V>>,
    dead_keys: Vec<Key>,
}

impl<V> TableMapBuilder<V> {

    #[must_use]
    fn new() -> Self {
        Self {
            map: SortedMap::new(),
            dead_keys: Vec::new(),
        }
    }

    #[must_use]
    pub fn finish(self) -> Table<V> {
        let Self { mut map, dead_keys } = self;
        let mut array = Vec::new();
        let max_len = i32::try_from(map.len())
            .unwrap_or(i32::MAX).saturating_mul(2);
        let mut array_len: usize = 0;
        let mut nonpositive_items = Vec::new();
        loop {
            let Some(entry) = map.first_entry() else { break; };
            let index = match *entry.key() {
                Key::Index(index) if index <= 0 => {
                    nonpositive_items.push(entry.remove_entry());
                    continue;
                },
                Key::Index(index) if index <= max_len => index,
                _ => break,
            };
            let index = u32_to_usize((index - 1) as u32);
            if array.len() <= index {
                array.resize_with(index + 1, || None);
            }
            let (_, value) = entry.remove_entry();
            if value.is_some() {
                array_len += 1;
            }
            let None = std::mem::replace(&mut array[index], value) else {
                unreachable!()
            };
        }
        map.extend(nonpositive_items);
        loop {
            while matches!(array.last(), Some(None)) {
                array.pop();
            }
            if array_len.saturating_mul(2) >= array.len() {
                break;
            }
            let index = i32::try_from(array.len()).unwrap();
            let Some(value) = array.pop() else {
                unreachable!()
            };
            array_len -= 1;
            map.insert(Key::Index(index), value);
        }
        let mut table = TableDumpBuilder::new(
            Some(array.len().try_into().unwrap()),
            ilog2_ceil(
                usize::checked_add(map.len(), dead_keys.len())
                    .unwrap()
            ),
        );
        table.array_extend(array);
        for (key, value) in map {
            table.assoc_insert(key, value);
        }
        for key in dead_keys {
            table.assoc_insert_dead(key);
        }
        table.finish()
    }

    pub fn insert<K: Into<Key>>(&mut self, key: K, value: Option<V>) {
        let old_value = self.map.insert(key.into(), value);
        assert!(old_value.is_none());
    }

    // pub fn insert_assoc_dead<K: Into<Key>>(&mut self, key: K) {
    //     self.dead_keys.push(key.into());
    // }

}

impl<'de, V> serde::de::Visitor<'de> for TableMapBuilder<V>
where V: Deserialize<'de>
{
    type Value = Table<V>;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "a map or an array")
    }

    fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de>
    {
        use crate::serde::FlatOption;
        let mut index = 1;
        while let Some(value) = seq.next_element::<FlatOption<_>>()? {
            self.insert(Key::Index(index), value.0);
            index += 1;
        }
        Ok(self.finish())
    }

    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de>
    {
        use crate::serde::FlatOption;
        while let Some((key, value)) =
            map.next_entry::<Key, FlatOption<_>>()?
        {
            self.insert(key, value.0);
        }
        Ok(self.finish())
    }

}

pub struct TableDumpIter<'s, V> {
    array: Option<std::slice::Iter<'s, Option<V>>>,
    assoc: assoc::TableDumpIter<'s, V>,
}

impl<'s, V> Iterator for TableDumpIter<'s, V> {
    type Item = Option<TableItem<KeyRef<'s>, &'s V>>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.array.as_mut()
            .and_then(Iterator::next)
        {
            return Some(item.as_ref().map(TableItem::Array));
        }
        self.assoc.next()
    }
}

impl<'s, V> crate::table::TableSize for TableDumpIter<'s, V> {

    #[must_use]
    fn array_len(&self) -> u32 {
        let Some(array) = &self.array else { return 0 };
        array.len().try_into().unwrap()
    }

    #[must_use]
    fn assoc_loglen(&self) -> Option<u16> {
        self.assoc.loglen()
    }

    #[must_use]
    fn assoc_last_free(&self) -> u32 {
        self.assoc.last_free()
    }
}

impl<'s, V: Dump> crate::dump::DumpTableIterator<'s> for TableDumpIter<'s, V> {
    type Key = KeyRef<'s>;
    type Value = V;
}

trait TableItemAdapter {
    type InputArrayItem;
    type InputAssocItem;
    type OutputKey;
    type OutputValue;
    fn adapt_array(index: i32, value: Self::InputArrayItem)
    -> Option<(Self::OutputKey, Self::OutputValue)>;
    fn adapt_assoc(item: Self::InputAssocItem)
    -> Option<(Self::OutputKey, Self::OutputValue)>;
}

struct TableChainIter<K, V, I, J, A>
where
    I: Iterator,
    J: Iterator,
    A: TableItemAdapter<
        InputArrayItem = I::Item,
        InputAssocItem = J::Item,
        OutputKey = K,
        OutputValue = V,
    >,
{
    array_iter: Option<std::iter::Enumerate<I>>,
    assoc_iter: Option<J>,
    adapter: PhantomData<A>,
}

impl<K, V, I, J, A> Iterator for TableChainIter<K, V, I, J, A>
where
    I: Iterator,
    J: Iterator,
    A: TableItemAdapter<
        InputArrayItem = I::Item,
        InputAssocItem = J::Item,
        OutputKey = K,
        OutputValue = V,
    >,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((index, value)) =
            self.array_iter.as_mut().and_then(Iterator::next)
        {
            let Some(index) = index
                .checked_add(1)
                .map(i32::try_from).and_then(Result::ok)
                else { unreachable!() };
            let Some((key, value)) = A::adapt_array(index, value)
                else { continue };
            return Some((key, value));
        }
        while let Some(item) =
            self.assoc_iter.as_mut().and_then(Iterator::next)
        {
            let Some((key, value)) = A::adapt_assoc(item)
                else { continue };
            return Some((key, value));
        }
        None
    }
}

impl<V> IntoIterator for Table<V> {
    type Item = (Key, V);
    type IntoIter = TableIntoIter<V>;
    fn into_iter(self) -> Self::IntoIter {
        TableIntoIter { inner: TableChainIter {
            array_iter: Some(self.array.into_iter().enumerate()),
            assoc_iter: Some(self.assoc.into_iter()),
            adapter: PhantomData,
        } }
    }
}

pub struct TableIntoIter<V> {
    inner: TableChainIter<Key, V,
        std::vec::IntoIter<Option<V>>,
        assoc::TableIntoIter<V>,
        TableIntoIterAdapter<V> >
}

struct TableIntoIterAdapter<V>(PhantomData<V>);

impl<V> TableItemAdapter for TableIntoIterAdapter<V> {
    type InputArrayItem = Option<V>;
    type InputAssocItem = Option<AssocItem<V>>;
    type OutputKey = Key;
    type OutputValue = V;

    #[inline]
    fn adapt_array(index: i32, value: Self::InputArrayItem)
    -> Option<(Self::OutputKey, Self::OutputValue)> {
        value.map(|v| (Key::Index(index), v))
    }

    #[inline]
    fn adapt_assoc(item: Self::InputAssocItem)
    -> Option<(Self::OutputKey, Self::OutputValue)> {
        let Some(AssocItem::Live { value: Some(value), key, .. }) = item
            else { return None };
        Some((key, value))
    }

}

impl<V> Iterator for TableIntoIter<V> {
    type Item = (Key, V);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'s, V> IntoIterator for &'s Table<V> {
    type Item = (KeyRef<'s>, &'s V);
    type IntoIter = TableIter<'s, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<V> Table<V> {
    pub fn iter(&'_ self) -> TableIter<'_, V> {
        TableIter { inner: TableChainIter {
            array_iter: Some(self.array.iter().enumerate()),
            assoc_iter: Some(self.assoc.iter()),
            adapter: PhantomData,
        } }
    }
}

pub struct TableIter<'s, V> {
    inner: TableChainIter<KeyRef<'s>, &'s V,
        std::slice::Iter<'s, Option<V>>,
        assoc::TableIter<'s, V>,
        TableIterAdapter<'s, V> >
}

struct TableIterAdapter<'s, V>(PhantomData<&'s V>);

impl<'s, V> TableItemAdapter for TableIterAdapter<'s, V> {
    type InputArrayItem = &'s Option<V>;
    type InputAssocItem = &'s Option<AssocItem<V>>;
    type OutputKey = KeyRef<'s>;
    type OutputValue = &'s V;

    #[inline]
    fn adapt_array(index: i32, value: Self::InputArrayItem)
    -> Option<(Self::OutputKey, Self::OutputValue)> {
        value.as_ref().map(|v| (KeyRef::Index(index), v))
    }

    #[inline]
    fn adapt_assoc(item: Self::InputAssocItem)
    -> Option<(Self::OutputKey, Self::OutputValue)> {
        let Some(AssocItem::Live { value: Some(value), key, .. }) = item
            else { return None };
        Some((key.as_ref(), value))
    }

}

impl<'s, V> Iterator for TableIter<'s, V> {
    type Item = (KeyRef<'s>, &'s V);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<V> Table<V> {
    pub fn sorted_iter(&'_ self) -> TableSortedIter<'_, V> {
        TableSortedIter { inner: TableChainIter {
            array_iter: Some(self.array.iter().enumerate()),
            assoc_iter: Some(self.assoc.sorted_iter()),
            adapter: PhantomData,
        } }
    }
}

pub struct TableSortedIter<'s, V> {
    inner: TableChainIter<KeyRef<'s>, &'s V,
        std::slice::Iter<'s, Option<V>>,
        assoc::TableSortedIter<'s, V>,
        TableIterAdapter<'s, V> >
}

impl<'s, V> Iterator for TableSortedIter<'s, V> {
    type Item = (KeyRef<'s>, &'s V);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub enum TableIntoError {
    NonContinuous(i32),
    UnexpectedKey(Key),
}

pub trait MaybeSequence<V> : Sized {
    fn new(table_len: u32) -> Self;
    fn insert(&mut self, index: i32, value: V) -> Result<(), TableIntoError>;
}

pub struct MaybeVec<V> {
    max_index: u32,
    vec: Vec<Option<V>>,
}

impl<V> MaybeSequence<V> for MaybeVec<V> {

    fn new(table_len: u32) -> Self {
        Self { max_index: table_len, vec: Vec::new() }
    }

    fn insert(&mut self, index: i32, value: V) -> Result<(), TableIntoError> {
        if index < 1 {
            return Err(TableIntoError::NonContinuous(index));
        };
        let index = index as u32;
        if index > self.max_index {
            return Err(TableIntoError::NonContinuous(index as i32));
        };
        let index = u32_to_usize(index);
        if self.vec.len() < index {
            self.vec.resize_with(index, || None);
        }
        self.vec[index-1] = Some(value);
        Ok(())
    }

}

impl<const N: usize, V> MaybeSequence<V> for [Option<V>; N] {
    fn new(_table_len: u32) -> Self {
        #[allow(clippy::use_self)]
        [(); N].map(|()| None)
    }

    fn insert(&mut self, index: i32, value: V) -> Result<(), TableIntoError> {
        if index < 1 {
            return Err(TableIntoError::NonContinuous(index));
        };
        let index = u32_to_usize(index as u32);
        if index > N {
            return Err(TableIntoError::UnexpectedKey(Key::Index(index as i32)));
        };
        self[index-1] = Some(value);
        Ok(())
    }
}

pub trait Sequence<V>: Sized {
    type Builder: MaybeSequence<V>;
    fn finish(precursor: Self::Builder) -> Result<Self, TableIntoError>;
}

impl<V> Sequence<V> for Vec<V> {
    type Builder = MaybeVec<V>;

    fn finish(maybe_vec: MaybeVec<V>) -> Result<Self, TableIntoError> {
        let maybe_vec = maybe_vec.vec;
        let mut vec = Self::with_capacity(maybe_vec.len());
        let mut maybe_vec = maybe_vec.into_iter();
        while let Some(value) = maybe_vec.next() {
            if let Some(value) = value {
                vec.push(value);
                continue;
            }
            let missing = vec.len() + 1;
            let Some(next_index) = maybe_vec.enumerate()
                .find_map(|(i, x)| x.map(|_| missing + i))
                .map(i32::try_from).and_then(Result::ok)
                else { unreachable!() };
            return Err(TableIntoError::NonContinuous(next_index));
        }
        Ok(vec)
    }
}

impl<const N: usize, V> Sequence<V> for [Option<V>; N] {
    type Builder = Self;
    fn finish(this: Self) -> Result<Self, TableIntoError> {
        Ok(this)
    }
}

pub struct LimitedVec<const N: usize, V> {
    vec: Vec<Option<V>>,
}

impl<const N: usize, V> LimitedVec<N, V> {
    #[must_use]
    pub fn get(self) -> Vec<Option<V>> { self.vec }
}

impl<const N: usize, V> MaybeSequence<V> for LimitedVec<N, V> {
    fn new(_table_len: u32) -> Self {
        Self { vec: Vec::new() }
    }

    fn insert(&mut self, index: i32, value: V) -> Result<(), TableIntoError> {
        if index < 1 {
            return Err(TableIntoError::NonContinuous(index));
        };
        let index = u32_to_usize(index as u32);
        if index > N {
            return Err(TableIntoError::UnexpectedKey(Key::Index(index as i32)));
        };
        if self.vec.len() < index {
            self.vec.resize_with(index, || None);
        }
        self.vec[index-1] = Some(value);
        Ok(())
    }
}

impl<const L: usize, V> Sequence<V> for LimitedVec<L, V> {
    type Builder = Self;

    fn finish(maybe_vec: Self) -> Result<Self, TableIntoError> {
        Ok(maybe_vec)
    }
}

impl<V> Table<V> {
    /// Sort indices in integers and strings.
    /// String indices are forwarded to `consume_name` function;
    /// any error is immediately forwarded back to the caller.
    /// Integer indices are collected into a sequence, which may
    /// additionally check them (e.g. return errors for negatives);
    /// the resulting sequence is returned to the caller.
    pub fn try_into_seq_and_named<S, F, E, EF>( self,
        mut consume_name: F,
        mut err_map: EF,
    ) -> Result<S, E>
    where
        S: Sequence<V>,
        F: FnMut(String, V) -> Result<(), E>,
        EF: FnMut(TableIntoError) -> E,
    {
        let Ok(table_len) = u32::try_from(self.len()) else {
            unreachable!()
        };
        let mut maybe_cont = S::Builder::new(table_len);
        for (key, value) in self {
            match key {
                Key::Index(index) => {
                    maybe_cont.insert(index, value).map_err(&mut err_map)?;
                    continue
                },
                Key::Name(name) => consume_name(name, value)?,
            }
        }
        let cont = S::finish(maybe_cont).map_err(&mut err_map)?;
        Ok(cont)
    }
    pub fn try_into_named<F, E, EF>( self,
        consume_name: F,
        err_map: EF,
    ) -> Result<(), E>
    where
        F: FnMut(String, V) -> Result<(), E>,
        EF: FnMut(TableIntoError) -> E,
    {
        let _: [_;0] = self.try_into_seq_and_named(consume_name, err_map)?;
        Ok(())
    }
}

impl<V> TryFrom<Table<V>> for Vec<V> {
    type Error = TableIntoError;
    #[allow(clippy::use_self)]
    fn try_from(this: Table<V>) -> Result<Vec<V>, Self::Error> {
        this.try_into_seq_and_named(
            |name, _value| Err(TableIntoError::UnexpectedKey(Key::Name(name))),
            std::convert::identity,
        )
    }
}

impl<V> Serialize for Table<V>
where V: Serialize
{
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer
    {
        use crate::serde::FlatOption;
        if self.assoc_loglen().is_none() {
            ser.collect_seq( self.array.iter()
                .map(Option::as_ref).map(FlatOption) )
        } else {
            ser.collect_map(self.sorted_iter())
        }
    }
}

impl<'de, V> Deserialize<'de> for Table<V>
where V: Deserialize<'de>
{
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de>
    {
        de.deserialize_any(TableMapBuilder::new())
    }
}

