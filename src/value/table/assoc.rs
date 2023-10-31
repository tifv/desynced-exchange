//! Associative part of the tables

use std::marker::PhantomData;

use crate::{
    table::{ilog2_exact, iexp2, u32_to_usize},
    value::Str,
    load,
};
use super::{Key, KeyRef};

// https://www.lua.org/source/5.4/lstring.c.html#luaS_hash
const fn str_table_hash_with_seed<const SEED: u32>(value: &str) -> u32 {
    let value = value.as_bytes();
    let mut index = value.len();
    assert!(u32::BITS <= usize::BITS && index <= u32::MAX as usize);
    let mut hash = SEED ^ (index as u32);
    let step = (index >> 5) + 1;
    let add = u32::wrapping_add;
    while index >= step {
        let j = match index.checked_sub(1) {
            Some(j) if j < value.len() => j,
            // SAFETY: i dare you
            _ => unsafe { std::hint::unreachable_unchecked() },
        };
        hash ^= add(
            add(hash << 5, hash >> 2),
            value[j] as u32 );
        index -= step;
    }
    hash
}

pub(crate) const fn str_table_hash(value: &str) -> u32 {
    str_table_hash_with_seed::<0x_645D_BFCD>(value)
}

// https://www.lua.org/source/5.4/ltable.c.html#hashint
pub(crate) const fn int_table_hash(value: i32, loglen: u16) -> u32 {
    if loglen == 0 { return 0; }
    if value >= 0 {
        (value % (mask(loglen) as i32)) as u32
    } else {
        (value as u32) % mask(loglen)
    }
}

pub(crate) type Item<V> = crate::table::AssocItem<Key, V>;

pub(crate) enum InsertItem<V> {
    Dead{position: u32},
    Live{key: Key, value: V},
}

impl Key {
    fn position(&self, loglen: u16) -> u32 {
        match *self {
            Self::Index(index) => int_table_hash(index, loglen),
            Self::Name(ref value) => str_table_hash(value) & mask(loglen),
        }
    }
}

impl<V> Item<V> {
    fn from_insert(item: InsertItem<V>, link: i32) -> Self {
        match item {
            InsertItem::Dead{..} => Self::Dead{link: 0},
            InsertItem::Live{key, value} => Self::Live{value, key, link: 0},
        }
    }
    fn main_position(&self, loglen: u16) -> Option<u32> {
        match self {
            Self::Dead{..} => None,
            Self::Live{key, ..} => Some(key.position(loglen))
        }
    }
    fn relocate(mut self, old_index: u32, new_index: u32) -> Self {
        let link_mut = match &mut self {
            Self::Live { link, .. } | Self::Dead { link } => link,
        };
        *link_mut = (*link_mut) + (old_index as i32) - (new_index as i32);
        self
    }
    fn relink(&mut self, new_link: i32) {
        let link_mut = match self {
            Self::Live { link, .. } | Self::Dead { link } => link,
        };
        *link_mut = new_link;
    }
}

impl<V> InsertItem<V> {
    pub fn dead_from_str(value: &str) -> Self {
        Self::Dead{position: str_table_hash(value)}
    }
    fn position(&self, loglen: u16) -> u32 {
        match *self {
            Self::Dead{position} => position & mask(loglen),
            Self::Live{ref key, ..} => key.position(loglen),
        }
    }
}

#[inline]
const fn mask(loglen: u16) -> u32 {
    iexp2(Some(loglen)) - 1
}

#[derive(Debug, Clone)]
pub(super) struct Table<V> {
    // invariant: if `items` is Some than `items.len()`` is a power of two
    items: Option<Box<[Option<Item<V>>]>>,
    last_free: u32,
}

impl<V> Table<V> {
    pub(super) fn new(loglen: Option<u16>) -> Self {
        let size = iexp2(loglen);
        let items = (size > 0).then(|| {
            let mut items = Vec::with_capacity(size as usize);
            items.resize_with(size as usize, || None);
            items
        });
        Self {
            items: items.map(Box::from),
            last_free: size,
        }
    }
    pub(super) fn loglen(&self) -> Option<u16> {
        let items = self.items.as_ref()?;
        // SAFETY: `items` has a size of a power of two
        Some(unsafe { ilog2_exact(items.len()).unwrap_unchecked() })
    }
    pub(super) fn len(&self) -> usize {
        if let Some(items) = self.items.as_ref() {
            items.len()
        } else { 0 }
    }
}

impl<V> Table<V> {
    pub fn insert( &mut self,
        item: InsertItem<V>,
    ) {
        let loglen = self.loglen().expect("the table should have free space");
        let main_index = item.position(loglen);
        if let free @ None = &mut self[main_index] {
            // Lua here would fill dead position as well as free.
            // But we are not Lua: we do not normally make dead positions,
            // and when we do, we don't want to overwrite them.
            *free = Some(Item::from_insert(item, 0));
            return;
        }
        let Some(free_index) = self.find_free_index() else {
            unreachable!("the table should have free space");
        };
        let other_index = self[main_index]
            .as_ref().unwrap()
            .main_position(loglen)
            .unwrap_or(main_index);
        if other_index == main_index {
            let link = free_index as i32 - main_index as i32;
            self[free_index] = Some(
                self[main_index]
                    .replace(Item::from_insert(item, link))
                    .unwrap()
                    .relocate(main_index, free_index)
            );
            return;
        }
        let mut prev_index = other_index;
        loop {
            let link = match self[prev_index]
                .as_ref().unwrap()
            {
                &Item::Dead { link } | &Item::Live { link, .. }
                    if link != 0 => link,
                _ => unreachable!(),
            };
            let next_index = (prev_index as i32 + link) as u32;
            if next_index == main_index {
                break;
            }
            prev_index = next_index;
            // we should have checked that we did not enter a cycle,
            // but in serialize mode this is not necessary
        }
        self[free_index] = Some(
            self[main_index]
                .replace(Item::from_insert(item, 0))
                .unwrap()
                .relocate(main_index, free_index)
        );
        self[prev_index].as_mut().unwrap()
            .relink(free_index as i32 - other_index as i32);
    }
    fn find_free_index(&mut self) -> Option<u32> {
        while self.last_free > 0 {
            self.last_free -= 1;
            if self[self.last_free].is_none() {
                return Some(self.last_free);
            }
        }
        None
    }
    pub fn last_free(&self) -> u32 {
        self.last_free
    }
}

impl<V> Table<V> {
    pub fn dump_iter(&self) -> TableDumpIter<'_, V> {
        TableDumpIter{
            items: self.items.as_ref().map_or_else(
                Default::default,
                |items| items.iter() ),
            loglen: self.loglen(),
            last_free: self.last_free(),
        }
    }    
}

pub(super) struct TableDumpIter<'s, V> {
    items: std::slice::Iter<'s, Option<Item<V>>>,
    loglen: Option<u16>,
    last_free: u32,
}

impl<'s, V> TableDumpIter<'s, V> {
    pub(super) fn loglen(&self) -> Option<u16> {
        self.loglen
    }
    pub(super) fn last_free(&self) -> u32 {
        self.last_free
    }
}

impl<'s, V> Iterator for TableDumpIter<'s, V> {
    type Item = Option<super::TableItem<KeyRef<'s>, &'s V>>;
    fn next(&mut self) -> Option<Self::Item> {
        use std::convert::identity;
        let item = self.items.next()?;
        let Some(item) = item.as_ref() else {
            return Some(None)
        };
        Some(Some(super::TableItem::Assoc(
            item.as_ref().map_key_value(Key::as_ref, identity)
        )))
    }
}

impl<V> IntoIterator for Table<V> {
    type Item = Option<Item<V>>;
    type IntoIter = TableIntoIter<V>;
    fn into_iter(self) -> Self::IntoIter {
        TableIntoIter{items: self.items.map(Vec::from).map(Vec::into_iter)}
    }
}

impl<V> std::ops::Index<u32> for Table<V> {
    type Output = Option<Item<V>>;
    fn index(&self, index: u32) -> &Self::Output {
        let items = self.items.as_ref().unwrap();
        &items[index as usize]
    }
}

impl<V> std::ops::IndexMut<u32> for Table<V> {
    fn index_mut(&mut self, index: u32) -> &mut Self::Output {
        let items = self.items.as_mut().unwrap();
        &mut items[index as usize]
    }
}

pub(super) struct TableIntoIter<V> {
    items: Option<std::vec::IntoIter<Option<Item<V>>>>,
}

impl<V> Iterator for TableIntoIter<V> {
    type Item = Option<Item<V>>;
    fn next(&mut self) -> Option<Self::Item> {
        self.items.as_mut().and_then(Iterator::next)
    }
}

pub(super) struct TableLoadBuidler<V> {
    table: Table<V>,
}

impl<V> TableLoadBuidler<V> {
    pub(super) fn new(loglen: Option<u16>) -> Self {
        Self{table: Table::new(loglen)}
    }
    pub(super) fn finish<E: load::Error>(self) -> Result<Table<V>, E> {
        let Some(items) = self.table.items.as_deref() else {
            return Ok(self.table);
        };
        // SAFETY: `items` has a size of a power of two
        let loglen = unsafe { ilog2_exact(items.len()).unwrap_unchecked() };
        let len = iexp2(Some(loglen));
        let mut unvalidated: Vec<_> = items.iter().map( |item| {
            let item = item.as_ref()?;
            item.main_position(loglen)
        }).collect();
        for position in 0 .. len {
            let mut index = u32_to_usize(position);
            let mut steps = 0;
            loop {
                if unvalidated[index] == Some(position) {
                    unvalidated[index] = None;
                }
                let link = match items[index] {
                    Some(Item::Dead{link} | Item::Live{link, ..})
                        if link != 0 => link,
                    _ => break,
                };
                index = index.wrapping_add((link as isize) as usize);
                steps += 1;
                if steps >= len {
                    return Err(E::from(
                        "node chain should not form a loop" ));
                }
            }
        }
        for position in 0 .. len {
            let mut index = u32_to_usize(position);
            if unvalidated[index].is_some() {
                return Err(E::from(
                    "table key should be in a valid position" ));
            }
        }
        Ok(self.table)
    }
    pub(super) fn insert(&mut self, index: u32, item: Item<V>) {
        let items = self.table.items.as_mut().unwrap();
        let index = u32_to_usize(index);
        let old_item = items[index].replace(item);
        assert!(old_item.is_none());
    }
}

