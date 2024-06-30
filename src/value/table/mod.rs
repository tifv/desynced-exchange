use std::collections::HashMap;

mod assoc;

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
    pub fn map_builder() -> TableMapBuilder<V> {
        TableMapBuilder::new()
    }

    #[must_use]
    pub fn dump_builder(array_len: Option<u32>, assoc_loglen: Option<u16>)
    -> TableDumpBuilder<V> {
        TableDumpBuilder::new(array_len, assoc_loglen)
    }

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

pub(super) struct TableLoadBuilder<V> {
    array: Vec<Option<V>>,
    assoc: AssocTableLoadBuilder<V>,
}

impl<V> TableLoadBuilder<V> {

    #[must_use]
    pub(super) fn new(array_len: u32, assoc_loglen: Option<u16>) -> Self {
        let mut array = Vec::with_capacity(u32_to_usize(array_len));
        array.resize_with(u32_to_usize(array_len), || None);
        Self {
            array,
            assoc: AssocTableLoadBuilder::new(assoc_loglen),
        }
    }

    pub(super) fn finish<E: load::Error>(self) -> Result<Table<V>, E> {
        let Self { array, assoc } = self;
        let assoc = assoc.finish::<E>()?;
        Ok(Table { array, assoc })
    }

    pub(super) fn array_insert<E: load::Error>( &mut self,
        index: u32, value: V,
    ) -> Result<(), E> {
        //! `index` is 0-based
        #![allow(clippy::unnecessary_wraps)]
        let index = u32_to_usize(index);
        let old_value = self.array[index].replace(value);
        assert!(old_value.is_none());
        Ok(())
    }

    pub(super) fn assoc_insert<E: load::Error>( &mut self,
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

    pub(super) fn set_last_free(&mut self, last_free: u32) {
        self.assoc.set_last_free(last_free)
    }

}

pub struct TableDumpBuilder<V> {
    array: Vec<Option<V>>,
    assoc: AssocTableDumpBuilder<V>,
}

impl<V> TableDumpBuilder<V> {

    #[must_use]
    fn new(
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

    pub fn array_push(&mut self, value: Option<V>) {
        self.array.push(value);
    }

    pub fn assoc_insert(&mut self, key: Key, value: Option<V>) {
        self.assoc.insert(key, value)
    }

    pub fn assoc_insert_name(&mut self, key: &'static str, value: Option<V>) {
        self.assoc_insert(Key::from(key), value)
    }

    pub fn assoc_insert_dead(&mut self, key: Key) {
        self.assoc.insert_dead(key)
    }

    pub fn assoc_insert_dead_name(&mut self, key: &'static str) {
        self.assoc_insert_dead(Key::from(key))
    }

}

impl<V> Extend<Option<V>> for TableDumpBuilder<V> {
    fn extend<T: IntoIterator<Item=Option<V>>>(&mut self, iter: T) {
        self.array.extend(iter)
    }
}

impl<V> FromIterator<Option<V>> for Table<V> {
    fn from_iter<T: IntoIterator<Item=Option<V>>>(iter: T) -> Self {
        Self {
            array: Vec::from_iter(iter),
            assoc: AssocTable::new(None),
        }
    }
}

pub struct TableMapBuilder<V> {
    values: HashMap<Key, Option<V>>,
    dead_keys: Vec<Key>,
}

impl<V> TableMapBuilder<V> {

    #[must_use]
    fn new() -> Self {
        Self {
            values: HashMap::new(),
            dead_keys: Vec::new(),
        }
    }

    #[must_use]
    pub fn finish(self) -> Table<V> {
        let Self { mut values, dead_keys } = self;
        let mut array = Vec::new();
        let max_len = i32::try_from(values.len())
            .unwrap_or(i32::MAX).saturating_mul(2);
        let mut array_len: usize = 0;
        for index in 1 ..= max_len {
            let Some(value) = values.remove(&Key::Index(index)) else {
                continue
            };
            let Some(value) = value else {
                values.insert(Key::Index(index), value);
                continue
            };
            let index = u32_to_usize((index - 1) as u32);
            if array.len() <= index {
                array.resize_with(index + 1, || None);
            }
            array[index] = Some(value);
            array_len += 1;
        }
        while array_len.saturating_mul(2) < array.len() {
            let index = i32::try_from(array.len()).unwrap();
            let value = array.pop().unwrap();
            array_len -= 1;
            values.insert(Key::Index(index), value);
            while array.last().is_some_and(Option::is_none) {
                array.pop();
            }
        }
        let mut table = Table::dump_builder(
            Some(array.len().try_into().unwrap()),
            ilog2_ceil(
                usize::checked_add(values.len(), dead_keys.len())
                    .unwrap()
            ),
        );
        table.extend(array);
        for (key, value) in values {
            table.assoc_insert(key, value);
        }
        for key in dead_keys {
            table.assoc_insert_dead(key);
        }
        table.finish()
    }

    pub fn insert(&mut self, key: Key, value: Option<V>) {
        let old_value = self.values.insert(key, value);
        assert!(old_value.is_none());
    }

    pub fn insert_name(&mut self, key: &'static str, value: Option<V>) {
        self.insert(Key::from(key), value)
    }

    pub fn insert_assoc_dead(&mut self, key: Key) {
        self.dead_keys.push(key);
    }

    pub fn insert_assoc_dead_name(&mut self, key: &'static str) {
        self.insert_assoc_dead(Key::from(key))
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

impl<V> IntoIterator for Table<V> {
    type Item = (Key, V);
    type IntoIter = TableIntoIter<V>;
    fn into_iter(self) -> Self::IntoIter {
        TableIntoIter {
            array_iter: Some(self.array.into_iter().enumerate()),
            assoc_iter: Some(self.assoc.into_iter()),
        }
    }
}

pub struct TableIntoIter<V> {
    array_iter: Option<std::iter::Enumerate<
        std::vec::IntoIter<Option<V>> >>,
    assoc_iter: Option<assoc::TableIntoIter<V>>,
}

impl<V> Iterator for TableIntoIter<V> {
    type Item = (Key, V);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((index, value)) =
            self.array_iter.as_mut().and_then(Iterator::next)
        {
            let Some(value) = value else { continue };
            let Some(index) = index
                .checked_add(1)
                .map(i32::try_from).and_then(Result::ok)
                else { unreachable!() };
            return Some((Key::Index(index), value));
        }
        while let Some(item) =
            self.assoc_iter.as_mut().and_then(Iterator::next)
        {
            let Some(AssocItem::Live { value: Some(value), key, .. }) = item
                else { continue };
            return Some((key, value));
        }
        None
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

