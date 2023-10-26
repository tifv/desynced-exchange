use std::marker::PhantomData;

use crate::string::Str;
pub(crate) use crate::value::Key;

// https://www.lua.org/source/5.4/lstring.c.html#luaS_hash
const fn str_table_hash_with_seed<const SEED: u32>(value: &str) -> u32 {
    let value = value.as_bytes();
    let mut i = value.len();
    let mut hash = SEED ^ (i as u32);
    let step = (i >> 5) + 1;
    let add = u32::wrapping_add;
    while i >= step {
        let j = match i.checked_sub(1) {
            Some(j) if j < value.len() => j,
            // SAFETY: provable invariant
            _ => unsafe { std::hint::unreachable_unchecked() },
        };
        hash ^= add(
            add(hash << 5, hash >> 2),
            value[j] as u32 );
        i -= step;
    }
    hash
}

pub(crate) const fn str_table_hash(value: &str) -> u32 {
    str_table_hash_with_seed::<0x_645D_BFCD>(value)
}

// https://www.lua.org/source/5.4/ltable.c.html#hashint
pub(crate) const fn int_table_hash(value: i32, logsize: u16) -> u32 {
    if logsize == 0 { return 0; }
    if value >= 0 {
        (value % (mask(logsize) as i32)) as u32
    } else {
        (value as u32) % mask(logsize)
    }
}

pub(crate) enum Item<V> {
    Free,
    Dead{link: i32},
    Live{value: V, key: Key, link: i32},
}

pub(crate) enum InsertItem<V> {
    Dead{position: u32},
    Live{key: Key, value: V},
}

impl Key {
    fn position(&self, logsize: u16) -> u32 {
        match *self {
            Self::Index(index) => int_table_hash(index, logsize),
            Self::Name(ref value) => str_table_hash(value) & mask(logsize),
        }
    }
}

impl<V> Default for Item<V> {
    fn default() -> Self { Self::Free }
}

impl<V> Item<V> {
    fn from_insert(item: InsertItem<V>, link: i32) -> Self {
        match item {
            InsertItem::Dead{..} => Self::Dead{link: 0},
            InsertItem::Live{key, value} => Self::Live{value, key, link: 0},
        }
    }
    fn is_free(&self) -> bool {
        matches!(self, Self::Free)
    }
    fn take(&mut self) -> Self {
        std::mem::take(self)
    }
    fn main_position(&self, logsize: u16) -> Option<u32> {
        match self {
            Self::Free | Self::Dead{..} => None,
            Self::Live{key, ..} => Some(key.position(logsize))
        }
    }
    fn relocate(mut self, old_index: u32, new_index: u32) -> Self {
        let link_mut = match &mut self {
            Self::Live { link, .. } | Self::Dead { link } => link,
            Self::Free => unreachable!(),
        };
        *link_mut = (*link_mut) + (old_index as i32) - (new_index as i32);
        self
    }
    fn relink(&mut self, new_link: i32) {
        let link_mut = match self {
            Self::Live { link, .. } | Self::Dead { link } => link,
            Self::Free => unreachable!(),
        };
        *link_mut = new_link;
    }
}

impl<V> InsertItem<V> {
    fn position(&self, logsize: u16) -> u32 {
        match *self {
            Self::Dead{position} => position & mask(logsize),
            Self::Live{ref key, ..} => key.position(logsize),
        }
    }
}

pub(crate) fn ilog2_ceil(size: usize) -> Option<u16> {
    //! Upper-rounded base 2 logarithm
    let Some(mut ilog2) = size.checked_ilog2() else {
        return None;
    };
    if ilog2 > size.trailing_zeros() {
        ilog2 += 1;
    }
    let Ok(ilog2) = ilog2.try_into() else {
        unreachable!()
    };
    Some(ilog2)
}

unsafe fn ilog2_exact(size: usize) -> u16 {
    //! Upper-rounded base 2 logarithm.
    //! `size` must be a power of two.
    let Some(mut ilog2) = size.checked_ilog2() else {
        // SAFETY: size is nonzero
        unsafe { std::hint::unreachable_unchecked(); }
    };
    if ilog2 > size.trailing_zeros() {
        // SAFETY: size is a power of two
        unsafe { std::hint::unreachable_unchecked(); }
    }
    let Ok(ilog2) = ilog2.try_into() else {
        // SAFETY: `usize` bit length fits into u16
        unsafe { std::hint::unreachable_unchecked(); }
    };
    ilog2
}

#[inline]
const fn iexp2(logsize: Option<u16>) -> u32 {
    let Some(logsize) = logsize else { return 0 };
    match 1_u32.checked_shl(logsize as u32) {
        Some(exp) if exp - 1 <= (i32::MAX as u32) => exp,
        _ => panic!("size should be addressable by i32"),
    }
}

#[inline]
const fn mask(logsize: u16) -> u32 {
    iexp2(Some(logsize)) - 1
}

pub(crate) trait TableMode {}
pub(crate) struct SerializeMode;
impl TableMode for SerializeMode {}
pub(crate) struct DeserializeMode;
impl TableMode for DeserializeMode {}

pub(crate) struct Table<V, M: TableMode> {
    // invariant: items.len() is a power of two
    items: Box<[Item<V>]>,
    last_free: u32,
    mode: PhantomData<M>,
}

impl<V, M: TableMode> Table<V, M> {
    pub fn with_logsize(logsize: u16) -> Self {
        let size = iexp2(Some(logsize));
        let mut items = Vec::with_capacity(size as usize);
        items.resize_with(size as usize, || Item::Free);
        Self {
            items: items.into(),
            last_free: size,
            mode: PhantomData,
        }
    }
    pub fn logsize(&self) -> u16 {
        // SAFETY: `self.items` always have power of two size
        unsafe { ilog2_exact(self.items.len()) }
    }
}

impl<V> Table<V, SerializeMode> {
    pub fn insert( &mut self,
        item: InsertItem<V>,
    ) {
        let logsize = self.logsize();
        let main_index = item.position(logsize);
        if let free @ Item::Free = &mut self.items[main_index as usize] {
            // Lua here would fill dead position as well as free.
            // But we are not Lua: we do not normally make dead positions,
            // and when we do, we don't want to overwrite them.
            *free = Item::from_insert(item, 0);
            return;
        }
        let Some(free_index) = self.find_free_index() else {
            unreachable!("table is full");
        };
        let other_index = self.items[main_index as usize]
            .main_position(logsize)
            .unwrap_or(main_index);
        if other_index == main_index {
            let link = free_index as i32 - main_index as i32;
            self.items[free_index as usize] = std::mem::replace(
                &mut self.items[main_index as usize],
                Item::from_insert(item, link),
            ).relocate(main_index, free_index);
            return;
        }
        let mut prev_index = other_index;
        loop {
            let link = match self.items[prev_index as usize] {
                Item::Dead { link } | Item::Live { link, .. }
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
        self.items[free_index as usize] = std::mem::replace(
            &mut self.items[main_index as usize],
            Item::from_insert(item, 0),
        ).relocate(main_index, free_index);
        self.items[prev_index as usize].relink(
            free_index as i32 - other_index as i32 );
    }
    fn find_free_index(&mut self) -> Option<u32> {
        while self.last_free > 0 {
            self.last_free -= 1;
            if self.items[self.last_free as usize].is_free() {
                return Some(self.last_free);
            }
        }
        None
    }
    pub fn get_last_free(&self) -> u32 {
        self.last_free
    }
}

impl<'s, V, M: TableMode> IntoIterator for &'s Table<V, M> {
    type Item = &'s Item<V>;
    type IntoIter = std::slice::Iter<'s, Item<V>>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}
