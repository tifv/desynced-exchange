use std::marker::PhantomData;
use std::collections::BTreeMap as SortedMap;

mod assoc;

use crate::{
    common::{u32_to_usize, iexp2, ilog2_ceil},
    string::Str,
    table_iter::{TableItem, TableSize},
};

use self::assoc::{
    Table as AssocTable,
    TableBuilder as AssocTableDumpBuilder,
};

pub(super) use self::assoc::Item as AssocItem;

pub use super::Key as Key;

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
    pub fn len(&self) -> usize {
        //! May be greater than the actual number of non-nil entries
        usize::checked_add(
            u32_to_usize(self.array_len()),
            u32_to_usize(iexp2(self.assoc_loglen())),
        ).unwrap()
    }

}

impl<V> TableSize for Table<V> {
    fn array_len(&self) -> u32 {
        self.array.len().try_into()
            .expect("the size should fit")
    }

    fn assoc_loglen(&self) -> Option<u16> {
        self.assoc.loglen()
    }

    fn assoc_last_free(&self) -> u32 {
        self.assoc.last_free()
    }
}

pub struct TableArrayBuilder<V> {
    array: Vec<Option<V>>,
}

impl<V> TableArrayBuilder<V> {

    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        Self { array: Vec::new() }
    }

    #[allow(dead_code)]
    pub fn push(&mut self, value: Option<V>) {
        self.array.push(value);
    }

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

/// Table builder facilitating conversion from Rust structures
pub struct TableBuilder<V> {
    array: Vec<Option<V>>,
    assoc: AssocTableDumpBuilder<V>,
}

impl<V> TableBuilder<V> {

    #[must_use]
    pub fn new(
        array_len: u32,
        assoc_loglen: Option<u16>,
    ) -> Self {
        Self {
            array: Vec::with_capacity(u32_to_usize(array_len)),
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

/// Table builder for the purposes of Deserialize trait
pub struct TableMapBuilder<V> {
    map: SortedMap<Key, Option<V>>,
}

impl<V> TableMapBuilder<V> {

    #[must_use]
    pub fn new() -> Self {
        Self {
            map: SortedMap::new(),
        }
    }

    #[must_use]
    pub fn finish(self) -> Table<V> {
        let Self { mut map } = self;
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
        let mut table = TableBuilder::new(
            array.len().try_into().unwrap(),
            ilog2_ceil(map.len()),
        );
        table.array_extend(array);
        for (key, value) in map {
            table.assoc_insert(key, value);
        }
        table.finish()
    }

    pub fn insert<K: Into<Key>>(&mut self, key: K, value: Option<V>) {
        let old_value = self.map.insert(key.into(), value);
        assert!(old_value.is_none());
    }

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

/// Table iterator of (key, value) pairs,
/// suitable for conversion into a map.
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
    type Item = (Key, &'s V);
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

/// Table iterator of (key, value) pairs,
/// suitable for representing as a map.
pub struct TableIter<'s, V> {
    inner: TableChainIter<Key, &'s V,
        std::slice::Iter<'s, Option<V>>,
        assoc::TableIter<'s, V>,
        TableIterAdapter<'s, V> >
}

struct TableIterAdapter<'s, V>(PhantomData<&'s V>);

impl<'s, V> TableItemAdapter for TableIterAdapter<'s, V> {
    type InputArrayItem = &'s Option<V>;
    type InputAssocItem = &'s Option<AssocItem<V>>;
    type OutputKey = Key;
    type OutputValue = &'s V;

    #[inline]
    fn adapt_array(index: i32, value: Self::InputArrayItem)
    -> Option<(Self::OutputKey, Self::OutputValue)> {
        value.as_ref().map(|v| (Key::Index(index), v))
    }

    #[inline]
    fn adapt_assoc(item: Self::InputAssocItem)
    -> Option<(Self::OutputKey, Self::OutputValue)> {
        let Some(AssocItem::Live { value: Some(value), key, .. }) = item
            else { return None };
        Some((key.clone(), value))
    }

}

impl<'s, V> Iterator for TableIter<'s, V> {
    type Item = (Key, &'s V);
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

/// Table iterator of (key, value) pairs,
/// suitable for serializing as a map.
/// Outputs keys in a predictable order.
pub struct TableSortedIter<'s, V> {
    inner: TableChainIter<Key, &'s V,
        std::slice::Iter<'s, Option<V>>,
        assoc::TableSortedIter<'s, V>,
        TableIterAdapter<'s, V> >
}

impl<'s, V> Iterator for TableSortedIter<'s, V> {
    type Item = (Key, &'s V);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<V> Table<V> {
    pub fn array_iter(&'_ self) -> std::slice::Iter<'_, Option<V>> {
        self.array.iter()
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
        F: FnMut(Str, V) -> Result<(), E>,
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
        F: FnMut(Str, V) -> Result<(), E>,
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

pub(super) mod load {

use crate::{
    common::u32_to_usize,
    load::Error as LoadError,
};

use super::{
    Key, AssocItem, Table,
    assoc::load::TableLoadBuilder as AssocTableLoadBuilder
};

/// Table builder for the purposes of Load trait
pub(in super::super) struct TableLoadBuilder<V> {
    array: Vec<Option<V>>,
    assoc: AssocTableLoadBuilder<V>,
}

impl<V> TableLoadBuilder<V> {

    #[must_use]
    pub(in super::super) fn new(array_len: u32, assoc_loglen: Option<u16>) -> Self {
        let mut array = Vec::with_capacity(u32_to_usize(array_len));
        array.resize_with(u32_to_usize(array_len), || None);
        Self {
            array,
            assoc: AssocTableLoadBuilder::new(assoc_loglen),
        }
    }

    pub(in super::super) fn finish<E: LoadError>(self) -> Result<Table<V>, E> {
        let Self { array, assoc } = self;
        let assoc = assoc.finish::<E>()?;
        Ok(Table { array, assoc })
    }

    pub(in super::super) fn array_insert<E: LoadError>( &mut self,
        index: u32, value: V,
    ) -> Result<(), E> {
        //! `index` is 0-based
        #![allow(clippy::unnecessary_wraps)]
        let index = u32_to_usize(index);
        let old_value = self.array[index].replace(value);
        assert!(old_value.is_none());
        Ok(())
    }

    pub(in super::super) fn assoc_insert<E: LoadError>( &mut self,
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

    pub(in super::super) fn set_assoc_last_free(&mut self, last_free: u32) {
        self.assoc.set_last_free(last_free)
    }

}

}

pub(super) mod dump {

use crate::{
    dump::{Dump, TableDumpIter as TableDumpIterTr},
    table_iter::{TableItem, TableSize},
};

use super::{
    Key, Table,
    assoc::dump::TableDumpIter as AssocTableDumpIter,
};

impl<V> Table<V> {
    #[must_use]
    pub(in super::super) fn dump_iter(&self) -> TableDumpIter<'_, V>
    {
        TableDumpIter {
            array: Some(self.array.iter()),
            assoc: self.assoc.dump_iter(),
        }
    }
}

/// Table iterator for the purposes of Dump trait
pub(in super::super) struct TableDumpIter<'s, V> {
    array: Option<std::slice::Iter<'s, Option<V>>>,
    assoc: AssocTableDumpIter<'s, V>,
}

impl<'s, V> Clone for TableDumpIter<'s, V> {
    fn clone(&self) -> Self {
        Self { array: self.array.clone(), assoc: self.assoc.clone() }
    }
}

impl<'s, V> TableDumpIter<'s, V> {
    pub(in super::super) fn take_array(&mut self)
    -> Option<std::slice::Iter<'s, Option<V>>>
    {
        self.array.take()
    }
}

impl<'s, V> Iterator for TableDumpIter<'s, V> {
    type Item = Option<TableItem<Key, &'s V>>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.array.as_mut()
            .and_then(Iterator::next)
        {
            return Some(item.as_ref().map(TableItem::Array));
        }
        Some(self.assoc.next()?.map(TableItem::Assoc))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = ExactSizeIterator::len(self);
        (len, Some(len))
    }
    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        if let Some(array) = self.array.as_mut() {
            let array_len = array.len();
            match array.nth(n) {
                Some(item) =>
                    return Some(item.as_ref().map(TableItem::Array)),
                None => {
                    self.array = None;
                    n -= array_len;
                },
            }
        }
        Some(self.assoc.nth(n)?.map(TableItem::Assoc))
    }
}

impl<'s, V> ExactSizeIterator for TableDumpIter<'s, V> {
    fn len(&self) -> usize {
        let array_len = self.array.as_ref().map_or(0, ExactSizeIterator::len);
        let assoc_len =  self.assoc.len();
        usize::checked_add(array_len, assoc_len).unwrap()
    }
}

impl<'s, V> TableSize for TableDumpIter<'s, V> {
    fn array_len(&self) -> u32 {
        let Some(array) = &self.array else { return 0 };
        array.len().try_into().unwrap()
    }
    fn assoc_loglen(&self) -> Option<u16> {
        self.assoc.loglen()
    }
    fn assoc_last_free(&self) -> u32 {
        self.assoc.last_free()
    }
}

impl<'s, V> TableDumpIterTr<'s> for TableDumpIter<'s, V>
where V: Dump
{
    type Key = Key;
    type Value = V;
}

}

