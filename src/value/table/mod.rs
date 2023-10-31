mod assoc;

use std::{iter::Enumerate, mem::MaybeUninit};

use crate::{
    table::{
        Key as GenericKey, KeyRef,
        TableItem,
        u32_to_usize,
    },
    dump::{self, DumpKey, Dump, Dumper, KeyDumper},
    load,
};

use super::Str;

use self::assoc::{
    Table as AssocTable, Item as AssocItem,
    TableLoadBuidler as AssocTableLoadBuilder,
    TableDumpBuidler as AssocTableDumpBuilder,
};

type Key = GenericKey<i32, Str>;

#[derive(Clone)]
pub struct Table<V> {
    array: Vec<Option<V>>,
    assoc: AssocTable<V>,
}

impl<V> Table<V> {
    pub fn dump_iter(&self) -> TableDumpIter<'_, V>
    {
        TableDumpIter{
            array: Some(self.array.iter()),
            assoc: self.assoc.dump_iter(),
        }
    }
}

/*
impl<V> IntoIterator for Table<V> {
    type Item = (Key, V);
    type IntoIter = TableIntoIter<V>;
    fn into_iter(self) -> Self::IntoIter {
        TableIntoIter{
            array_iter: Some(self.array.into_iter().enumerate()),
            assoc_iter: Some(self.assoc.into_iter()),
        }
    }
}
*/

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

impl<'s, V: Dump> crate::dump::DumpTableIterator for TableDumpIter<'s, V> {
    type Key = KeyRef<'s>;
    type Value = &'s V;
}

/*

pub struct TableIntoIter<V> {
    array_iter: Option<Enumerate<std::vec::IntoIter<Option<V>>>>,
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
            let Some(AssocItem::Live{value, key, ..}) = item
                else { continue };
            return Some((key.clone(), value));
        }
        None
    }
}

*/

pub(super) struct TableLoadBuilder<V> {
    array: Vec<Option<V>>,
    assoc: AssocTableLoadBuilder<V>,
}

impl<V> TableLoadBuilder<V> {
    pub(super) fn new(array_len: u32, assoc_loglen: Option<u16>) -> Self {
        let mut array = Vec::with_capacity(u32_to_usize(array_len));
        array.resize_with(u32_to_usize(array_len), || None);
        Self {
            array,
            assoc: AssocTableLoadBuilder::new(assoc_loglen),
        }
    }
    pub(super) fn finish<E: load::Error>(self) -> Result<Table<V>, E> {
        let Self{array, assoc} = self;
        let assoc = assoc.finish::<E>()?;
        Ok(Table{array, assoc})
    }
    pub(super) fn array_insert(&mut self, index: u32, value: V) {
        //! `index` is 0-based
        let index = u32_to_usize(index);
        let old_value = self.array[index].replace(value);
        assert!(old_value.is_none());
    }
    pub(super) fn assoc_insert(&mut self, index: u32, item: AssocItem<V>) {
        //! `index` is 0-based
        self.assoc.insert(index, item);
    }
}

