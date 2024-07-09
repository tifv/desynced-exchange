use std::{iter::FusedIterator, ops::Range};

use thiserror::Error;

use super::Key;

mod assoc;

#[derive(Clone, )]
pub struct Table<V> {
    items: Vec<(Key, V)>,
    // the range of positive integer keys
    indices: Range<usize>,
}

impl<V: std::fmt::Debug> std::fmt::Debug for Table<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_map();
        f.entries(self.items.iter().map(|(k, v)| (k, v)));
        f.finish()
    }
}

impl<V> Table<V> {
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new(), indices: 0..0 }
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn insert(&mut self, key: Key, value: V) -> Option<V> {
        //! Returns the replaced element, if any.
        match self.find_item(&key) {
            Ok(index) => Some(std::mem::replace(
                &mut self.items[index], (key, value) ).1),
            Err(index) => {
                self.items.insert(index, (key, value));
                self.indices_fix();
                None
            },
        }
    }
    pub fn remove(&mut self, key: &Key) -> Option<V> {
        match self.find_item(key) {
            Err(_) => None,
            Ok(index) => {
                let (_, value) = self.items.remove(index);
                let Range { start, end } = &mut self.indices;
                if *start > index { *start -= 1; }
                if *end   > index { *end   -= 1; }
                Some(value)
            },
        }
    }
    #[must_use]
    pub fn array_builder() -> ArrayBuilder<V> {
        ArrayBuilder::new()
    }
}

impl<K: Into<Key>, V> FromIterator<(K, V)> for Table<V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut this = Self::new();
        this.extend(iter);
        this
    }
}

impl<K: Into<Key>, V> Extend<(K, V)> for Table<V> {
    fn extend<I>(&mut self, iter: I)
    where I: IntoIterator<Item = (K, V)>
    {
        for (key, value) in iter {
            self.push_item(key.into(), value);
        }
        self.sort_items();
    }
}

fn dedup_assign<V, F>(vec: &mut Vec<V>, same_bucket: F)
where F: Fn(&V, &V) -> bool
{
    //! Eliminate consecutive duplicates,
    //! leaving only the last of each bucket.
    #![allow(clippy::undocumented_unsafe_blocks)]
    let len = vec.len();
    if len <= 1 { return; }
    unsafe { vec.set_len(0); }
    let start = vec.as_mut_ptr();
    let end = unsafe { start.add(len) };
    let mut before_gap = start;
    let mut after_gap = unsafe { before_gap.add(1) };
    while unsafe { end.offset_from(after_gap) > 0 } {
        {
            if same_bucket(unsafe { &*after_gap }, unsafe { &*before_gap }) {
                let value = unsafe { after_gap.read() };
                after_gap = unsafe { after_gap.add(1) };
                std::mem::drop(unsafe { before_gap.read() });
                unsafe { before_gap.write(value); }
                continue;
            }
        }
        before_gap = unsafe { before_gap.add(1) };
        if before_gap < after_gap {
            let value = unsafe { after_gap.read() };
            after_gap = unsafe { after_gap.add(1) };
            unsafe { before_gap.write(value); }
        } else {
            after_gap = unsafe { after_gap.add(1) };
        }
    }
    let new_len = unsafe { before_gap.offset_from(start) as usize + 1 };
    unsafe { vec.set_len(new_len); }
}

impl<V> Table<V> {
    fn find_item(&self, key: &Key) -> Result<usize, usize> {
        use std::cmp::Ordering::{Less, Equal, Greater};
        let mut index = match *key {
            Key::Index(index) if index > 0 =>
                usize::max(
                    self.indices.start,
                    usize::saturating_sub(
                        self.indices.end,
                        usize::try_from(1 + self.last_index_key() - index).unwrap_or(0)
                    )
                ),
            Key::Index(_) => 0,
            Key::Name(_) => self.indices.end,
        };
        while index < self.items.len() {
            let Some((k, _)) = self.items.get(index) else {
                unreachable!();
            };
            match Key::cmp(k, key) {
                Less => (),
                Equal => return Ok(index),
                Greater => return Err(index),
            }
            index += 1;
        }
        Err(self.items.len())
    }
    fn push_item(&mut self, key: Key, value: V) {
        self.items.push((key, value));
    }
    fn sort_items(&mut self) {
        self.items.sort_by(|(k1, _), (k2, _)| Key::cmp(k1, k2));
        dedup_assign(&mut self.items, |(k1, _), (k2, _)| k1 == k2);
        self.indices_fix();
    }
    fn last_index_key(&self) -> i32 {
        //! Always returns a nonnegative number.
        if self.indices.is_empty() {
            return 0;
        }
        let Some(last) = self.indices.end.checked_sub(1) else {
            return 0;
        };
        let (ref key, _) = self.items[last];
        match *key {
            Key::Index(index) if index > 0 => index,
            _ => unreachable!(),
        }
    }
    fn indices_fix(&mut self) {
        loop { match self.items.get(self.indices.start) {
            Some(&(Key::Index(index), _)) if index <= 0 => {
                self.indices.start += 1;
                self.indices.end += 1;
                continue;
            },
            _ => break,
        } }
        self.indices_fix_end()
    }
    fn indices_fix_end(&mut self) {
        while let Some(&(Key::Index(_), _)) =
            self.items.get(self.indices.end)
        {
            self.indices.end += 1;
            continue;
        }
    }
}

impl<V> Default for Table<V> {
    fn default() -> Self { Self::new() }
}

pub struct ArrayBuilder<V> {
    table: Table<V>,
    last_index: i32,
}

impl<V> ArrayBuilder<V> {
    #[allow(clippy::new_without_default)]
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self { table: Table::new(), last_index: 0 }
    }
    #[must_use]
    #[inline]
    pub fn build(self) -> Table<V> {
        let mut table = self.table;
        table.indices = 0 .. table.items.len();
        table
    }
    #[inline]
    pub fn push(&mut self, value: V) {
        self.push_option(Some(value))
    }
    #[inline]
    pub fn push_option(&mut self, value: Option<V>) {
        self.last_index += 1;
        let Some(value) = value else { return; };
        self.table.push_item(Key::Index(self.last_index), value);
    }
}

impl<V, K> FromIterator<K> for ArrayBuilder<V>
where Self : Extend<K>
{
    #[inline]
    fn from_iter<T: IntoIterator<Item=K>>(iter: T) -> Self {
        let mut this = Self::new();
        this.extend(iter);
        this
    }
}

impl<V> Extend<V> for ArrayBuilder<V> {
    fn extend<T: IntoIterator<Item = V>>(&mut self, iter: T) {
        for value in iter {
            self.push(value);
        }
    }
}

impl<V> Extend<Option<V>> for ArrayBuilder<V> {
    fn extend<T: IntoIterator<Item=Option<V>>>(&mut self, iter: T) {
        for value in iter {
            self.push_option(value);
        }
    }
}

impl<V> IntoIterator for Table<V> {
    type Item = (Key, V);
    type IntoIter = std::vec::IntoIter<(Key, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

pub struct ClonedKeys<'s, V: 's, I>
where I: Iterator<Item=&'s (Key, V)>,
{
    inner: I,
}

impl<'s, V: 's, I> ClonedKeys<'s, V, I>
where I: Iterator<Item=&'s (Key, V)>
{
    fn new(inner: I) -> Self { ClonedKeys { inner } }
}

impl<'s, V: 's, I> Iterator for ClonedKeys<'s, V, I>
where I: Iterator<Item=&'s (Key, V)>
{
    type Item = (Key, &'s V);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| (k.clone(), v))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'s, V: 's, I> ExactSizeIterator for ClonedKeys<'s, V, I>
where I: ExactSizeIterator<Item=&'s (Key, V)>
{
    #[inline]
    fn len(&self) -> usize { self.inner.len() }
}

impl<'s, V: 's, I> DoubleEndedIterator for ClonedKeys<'s, V, I>
where I: DoubleEndedIterator<Item=&'s (Key, V)>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|(k, v)| (k.clone(), v))
    }
}

fn add_size_hint(
    ahint: (usize, Option<usize>),
    bhint: (usize, Option<usize>),
) -> (usize, Option<usize>) {
    #![allow(clippy::similar_names)]
    (
        usize::saturating_add(ahint.0, bhint.0),
        Option::zip(ahint.1, bhint.1)
            .and_then(|(x, y)| usize::checked_add(x, y)),
    )
}

struct ChainIter<I, J>
where
    I: Iterator,
    J: Iterator<Item=I::Item>,
{
    iter: Option<I>,
    jter: Option<J>,
}

impl<I, J> ChainIter<I, J>
where
    I: Iterator,
    J: Iterator<Item=I::Item>,
{
    fn new(iter: I, jter: J) -> Self {
        #![allow(clippy::similar_names)]
        Self { iter: Some(iter), jter: Some(jter) }
    }
}

impl<I, J> Iterator for ChainIter<I, J>
where
    I: Iterator,
    J: Iterator<Item=I::Item>,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut iter) = self.iter {
            if let Some(item) = iter.next() {
                return Some(item);
            }
            self.iter = None;
        }
        if let Some(ref mut jter) = self.jter {
            if let Some(item) = jter.next() {
                return Some(item);
            }
            self.jter = None;
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        add_size_hint(
            self.iter.as_ref().map_or((0, Some(0)), Iterator::size_hint),
            self.jter.as_ref().map_or((0, Some(0)), Iterator::size_hint),
        )
    }
}

impl<I, J> ExactSizeIterator for ChainIter<I, J>
where
    I: ExactSizeIterator,
    J: ExactSizeIterator<Item=I::Item>,
{
    fn len(&self) -> usize {
        usize::checked_add(
            self.iter.as_ref().map_or(0, ExactSizeIterator::len),
            self.jter.as_ref().map_or(0, ExactSizeIterator::len),
        ).unwrap()
    }
}


impl<V> Table<V> {
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item=(Key, &'_ V)> {
        <&Self as IntoIterator>::into_iter(self)
    }
}

impl<'s, V> IntoIterator for &'s Table<V> {
    type Item = (Key, &'s V);
    type IntoIter = ClonedKeys<'s, V, std::slice::Iter<'s, (Key, V)>>;
    fn into_iter(self) -> Self::IntoIter {
        ClonedKeys::new(self.items.iter())
    }
}

struct ArrayIter<'s, V> {
    keys: Range<i32>,
    items: &'s [(Key, V)],
}

impl<V> Table<V> {
    /// Split the map into array and assoc parts
    fn array_assoc_iter(&self) -> (
        impl ExactSizeIterator<Item=Option<&'_ V>>,
        impl ExactSizeIterator<Item=(Key, &'_ V)>,
    ) {
        let mut indices = self.indices.clone();
        loop {
            let Some(end) = indices.end.checked_sub(1) else { break; };
            if end < indices.start { break; }
            let max_index = usize::saturating_sub(indices.end, indices.start)
                .saturating_mul(2)
                .try_into().unwrap_or(i32::MAX);
            let key = self.items[end].0.as_index().unwrap();
            if key <= max_index {
                break;
            }
            let Some(_) = indices.next_back() else { break; };
        }
        let array_items = &self.items[indices.clone()];
        let array_keys = 1 .. array_items.last()
            .map_or(1, |(k, _)| 1 + k.as_index().unwrap());
        (
            ArrayIter { keys: array_keys, items: array_items },
            ClonedKeys::new(ChainIter::new(
                self.items[..indices.start].iter(),
                self.items[indices.end..].iter()
            ))
        )
    }
}

impl<'s, V> Iterator for ArrayIter<'s, V> {
    type Item = Option<&'s V>;
    fn next(&mut self) -> Option<Self::Item> {
        let key = self.keys.next()?;
        let Some((next_key, _)) = self.items.first() else {
            return Some(None);
        };
        let &Key::Index(next_key) = next_key else {
            unreachable!()
        };
        if next_key > key {
            return Some(None);
        }
        assert!(next_key == key);
        let Some(((_, value), rest)) = self.items.split_first() else {
            unreachable!()
        };
        self.items = rest;
        Some(Some(value))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.keys.size_hint()
    }
}

impl<'s, V> ExactSizeIterator for ArrayIter<'s, V> {
    fn len(&self) -> usize {
        self.keys.len()
    }
}

#[derive(Debug, Error)]
#[error("The sequence cannot contain None")]
pub struct NonContinuousError;

// Halts iteration if an error is encountered.
// (Also acts as a fuse.)
struct HaltingIter<V, E, I>
where I: Iterator<Item=Result<V, E>>
{
    iter: I,
    halted: bool,
}

impl<V, E, I> HaltingIter<V, E, I>
where I: Iterator<Item=Result<V, E>>
{
    fn new(iter: I) -> Self {
        Self { iter, halted: false }
    }
}

impl<V, E, I> Iterator for HaltingIter<V, E, I>
where I: Iterator<Item=Result<V, E>>
{
    type Item = Result<V, E>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.halted { return None; }
        let item = self.iter.next();
        if ! matches!(item, Some(Ok(_))) {
            self.halted = true;
        }
        item
    }
}

impl<V, E, I> FusedIterator for HaltingIter<V, E, I>
where I: Iterator<Item=Result<V, E>>
{}


impl<V> Table<V> {
    /// Returns all values in order, assuming the corresponding keys
    /// are integers in range 1..N.
    /// The first found non-conforming key will result in an `Err` item.
    /// Iterator fuses after an error is encountered.
    pub fn into_continuous_iter(self)
    -> impl Iterator<Item=Result<V, NonContinuousError>> + FusedIterator
    {
        let mut next_index = 1;
        HaltingIter::new(self.into_iter()
            .map(move |(key, value)| match key {
                Key::Index(index) if index == next_index => {
                    next_index += 1;
                    Ok(value)
                },
                _ => Err(NonContinuousError),
            }) )
    }
}


pub(super) mod load {

use crate::{
    common::iexp2,
    table_iter::TableItem,
    load::{Error, TableLoader},
};

use super::{Key, Table, ArrayBuilder};

use super::assoc::Table as AssocTable;

impl<V> Table<V> {
    pub(crate) fn load<T>(items: T) -> Result<Self, T::Error>
    where
        T : TableLoader<Key=Key, Value=V>,
        T::Error : Error,
    {
        let array_len = items.array_len();
        let assoc_loglen = items.assoc_loglen();
        let assoc_len = iexp2(assoc_loglen);
        let mut array = ArrayBuilder::new();
        let mut assoc = AssocTable::new_load_builder(assoc_loglen);
        assoc.set_last_free(items.assoc_last_free());
        let mut array_index = 0;
        let mut assoc_index = 0;
        for item in items {
            let item = item?;
            match (item, array_index < array_len, assoc_index < assoc_len) {
                (Some(TableItem::Array(value)), true, _) => {
                    array_index += 1;
                    array.push(value);
                },
                (None, true, _) => {
                    array_index += 1;
                    array.push_option(None);
                },
                (Some(TableItem::Array(_)), false, _) =>
                    panic!("unexpected array item"),
                (Some(TableItem::Assoc(assoc_item)), false, true) => {
                    let index = assoc_index;
                    assoc_index += 1;
                    assoc.insert(index, assoc_item);
                },
                (None, false, true) => assoc_index += 1,
                (Some(TableItem::Assoc(_)), true, _) =>
                    panic!("unexpected assoc item"),
                (_, false, false) =>
                    panic!("unexpected item"),
            }
        }
        let mut table = array.build();
        table.extend(assoc.build::<T::Error>()?.into_map_iter());
        Ok(table)
    }
}

}


pub(super) mod dump {

use crate::{
    common::LogSize,
    dump::{Dump, TableDumpIter as TableDumpIterTr},
    table_iter::{TableItem, TableSize},
};

use super::{
    Key, Table,
    add_size_hint,
};

use super::assoc::{
    Table as AssocTable,
    dump::TableDumpIter as AssocTableDumpIter,
};

impl<V: Dump> Table<V> {
    pub(crate) fn dump_iter(&self) -> impl TableDumpIterTr<'_>
    {
        let (array_iter, assoc_iter) = self.array_assoc_iter();
        TableDumpIter::from_array_assoc_iter(array_iter, assoc_iter)
    }
}

struct TableDumpIter<'s, V, I>
where
    V: Dump + 's,
    I : ExactSizeIterator<Item=Option<&'s V>>,
{
    array_iter: Option<I>,
    assoc_iter: Option<AssocTableDumpIter<'s, V>>,
    array_len: u32,
    assoc_loglen: Option<LogSize>,
    assoc_last_free: u32,
}

impl<'s, V, I> TableDumpIter<'s, V, I>
where
    V: Dump + 's,
    I : ExactSizeIterator<Item=Option<&'s V>>,
{
    fn from_array_assoc_iter<J>(array_iter: I, assoc_iter: J) -> Self
    where J : ExactSizeIterator<Item=(Key, &'s V)>
    {
        let array_len = array_iter.len().try_into()
            .expect("array length should not be that large");
        let assoc_table = AssocTable::from_map_iter(assoc_iter);
        let assoc_loglen = assoc_table.loglen();
        let assoc_last_free = assoc_table.last_free();
        Self {
            array_iter: Some(array_iter),
            assoc_iter: Some(assoc_table.dump_iter()),
            array_len, assoc_loglen, assoc_last_free,
        }
    }
}

impl<'s, V, I> Iterator for TableDumpIter<'s, V, I>
where
    V: Dump + 's,
    I : ExactSizeIterator<Item=Option<&'s V>>,
{
    type Item = Option<TableItem<Key, &'s V>>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut array_iter) = self.array_iter {
            if let Some(item) = array_iter.next() {
                return Some(item.map(TableItem::Array));
            }
            self.array_iter = None;
        }
        if let Some(ref mut assoc_iter) = self.assoc_iter {
            if let Some(item) = assoc_iter.next() {
                return Some(item.map(TableItem::Assoc));
            }
            self.assoc_iter = None;
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        add_size_hint(
            self.array_iter.as_ref().map_or((0, Some(0)), Iterator::size_hint),
            self.assoc_iter.as_ref().map_or((0, Some(0)), Iterator::size_hint),
        )
    }
}

impl<'s, V, I> ExactSizeIterator for TableDumpIter<'s, V, I>
where
    V: Dump + 's,
    I : ExactSizeIterator<Item=Option<&'s V>>,
{
    fn len(&self) -> usize {
        usize::checked_add(
            self.array_iter.as_ref().map_or(0, ExactSizeIterator::len),
            self.assoc_iter.as_ref().map_or(0, ExactSizeIterator::len),
        ).unwrap()
    }
}

impl<'s, V, I> TableSize for TableDumpIter<'s, V, I>
where
    V: Dump + 's,
    I : ExactSizeIterator<Item=Option<&'s V>>,
{
    fn array_len(&self) -> u32 {
        self.array_len
    }

    fn assoc_loglen(&self) -> Option<LogSize> {
        self.assoc_loglen
    }

    fn assoc_last_free(&self) -> u32 {
        self.assoc_last_free
    }
}

impl<'s, V, I> TableDumpIterTr<'s> for TableDumpIter<'s, V, I>
where
    V: Dump + 's,
    I : ExactSizeIterator<Item=Option<&'s V>>,
{
    type Key = Key;
    type Value = V;
}

}


pub(super) mod de {

use std::marker::PhantomData;

use serde::{Deserialize, de};

use crate::serde::{DeserializeOption, OptionSerdeWrap};

use super::Table;

impl<'de, V> Deserialize<'de> for Table<V>
where V: DeserializeOption<'de>
{
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: de::Deserializer<'de>
    {
        de.deserialize_any(TableVisitor::new())
    }
}

pub(in super::super) struct TableVisitor<V>(PhantomData<V>);

impl<V> TableVisitor<V> {
    pub(crate) fn new() -> Self { Self(PhantomData) }
}

impl<'de, V> de::Visitor<'de> for TableVisitor<V>
where V: DeserializeOption<'de>
{
    type Value = Table<V>;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "a sequence or a map")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(Table::new())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: de::SeqAccess<'de>
    {
        let mut table_builder = Table::array_builder();
        while let Some(OptionSerdeWrap(value)) = seq.next_element()? {
            table_builder.push_option(value);
        }
        Ok(table_builder.build())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: de::MapAccess<'de>
    {
        let mut table = Table::new();
        while let Some((key, value)) = map.next_entry()? {
            table.push_item(key, value);
        }
        table.sort_items();
        Ok(table)
    }

}

}


pub(super) mod ser {

use serde::{Serialize, ser};

use crate::serde::{SerializeOption, OptionRefSerdeWrap};

use super::Table;

impl<V> Serialize for Table<V>
where V: SerializeOption
{
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: ser::Serializer
    {
        let (array_iter, assoc_iter) = self.array_assoc_iter();
        if assoc_iter.len() == 0 && array_iter.len() > 0 {
            ser.collect_seq(array_iter.map(OptionRefSerdeWrap))
        } else {
            ser.collect_map(self.iter())
        }
    }
}

}


#[cfg(test)]
mod test {

use crate::string::Str;

use super::{Key, Table};

use super::dedup_assign;

#[test]
fn test_dedup_assign() {
    #![allow(clippy::unreadable_literal)]
    #[derive(Debug, PartialEq, Eq)]
    struct DroppedI32(Option<i32>);
    impl DroppedI32 {
        fn new(value: i32) -> Self { Self(Some(value)) }
        fn get(&self) -> i32 { self.0.unwrap() }
    }
    impl Drop for DroppedI32 {
        fn drop(&mut self) {
            assert!(self.0.take().is_some())
        }
    }
    let dedup = |mut v: Vec<DroppedI32>| {
        dedup_assign(&mut v, |x, y| x.get() % 10 == y.get() % 10);
        v
    };
    let d = DroppedI32::new;
    assert_eq!(dedup(vec![d( 1), d( 2), d( 3), d( 4)]), vec![d( 1), d( 2), d(3), d(4)]);
    assert_eq!(dedup(vec![d( 1), d(11), d( 3), d(33)]), vec![d(11), d(33)]);
    assert_eq!(dedup(vec![d(11), d( 1), d( 3), d(32)]), vec![d( 1), d( 3), d(32)]);
    assert_eq!(dedup(vec![d(11), d( 1), d( 3), d(31)]), vec![d( 1), d( 3), d(31)]);
    assert_eq!(
        dedup(vec![d(1), d(11), d(111), d(1111), d(11111), d(111111), d(1111111), d(11111111), d(111111111)]),
        vec![d(111111111)] );
    assert_eq!(
        dedup(vec![d(1), d(11), d(3), d(33), d(1), d(11), d(2), d(22)]),
        vec![d(11), d(33), d(11), d(22)] );
}

#[test]
fn test_insert_remove() {
    let mut test_keys = Vec::new();
    for i in -20 ..= 20 {
        test_keys.push(Key::Index(i));
    }
    for s in 'a' ..= 'z' {
        test_keys.push(Key::Name(Str::from(String::from(s).as_str())));
    }
    let mut table = Table::new();
    for key in &test_keys {
        assert!(table.insert(key.clone(), key.clone()).is_none());
    }
    for key in &test_keys {
        assert!(table.insert(key.clone(), key.clone()).is_some());
    }
    for key in &test_keys {
        assert_eq!(table.remove(key).as_ref(), Some(key));
    }
}

}

