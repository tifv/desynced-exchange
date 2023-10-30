mod string;
mod table;

pub use string::Str;
pub use table::Table;

use crate::{
    dump,
    load, table::{TableItem, AssocItem, iexp2},
};

use self::table::TableLoadBuilder;

pub enum Value {
    Nil,
    Boolean(bool),
    Integer(i32),
    Float(f64),
    String(Str),
    Table(Table<Value>)
}

type Key = crate::table::Key<i32, Str>;

impl dump::Dump for Value {
    fn dump<DD: dump::Dumper>(&self, dumper: DD) -> Result<DD::Ok, DD::Error> {
        match *self {
            Self::Nil => dumper.dump_nil(),
            Self::Boolean(value) =>
                dumper.dump_boolean(value),
            Self::Integer(value) =>
                dumper.dump_integer(value),
            Self::Float(value) =>
                dumper.dump_float(value),
            Self::String(ref value) =>
                dumper.dump_string(value),
            Self::Table(ref table) =>
                dumper.dump_table(table.dump_iter()),
        }
    }
}

impl load::LoadKey for Key {
    #[inline]
    fn load_key<KLL: load::KeyLoader>(loader: KLL)
    -> Result<Option<Self>, load::Error> {
        loader.load_key(KeyBuilder)
    }
}

impl load::Load for Value {
    #[inline]
    fn load<LL: load::Loader>(loader: LL)
    -> Result<Self, load::Error> {
        loader.load_value(ValueBuilder)
    }
    #[inline]
    fn is_nil(&self) -> bool { matches!(self, Self::Nil) }
}

struct KeyBuilder;

impl load::KeyBuilder for KeyBuilder {
    type Value = Key;

    #[inline]
    fn build_integer(self, value: i32) -> Result<Self::Value, load::Error> {
        Ok(Key::Index(value))
    }

    #[inline]
    fn build_string<R: std::io::Read>( self,
        len: u32, mut read: R,
    ) -> Result<Self::Value, load::Error> {
        Ok(Key::Name(self::string::str_from_len_read(len, &mut read)?))
    }
}

struct ValueBuilder;

impl load::Builder for ValueBuilder {
    type Key = Key;
    type Value = Value;

    #[inline]
    fn build_nil(self) -> Result<Self::Value, load::Error> {
        Ok(Value::Nil)
    }

    #[inline]
    fn build_boolean(self, value: bool) -> Result<Self::Value, load::Error> {
        Ok(Value::Boolean(value))
    }

    #[inline]
    fn build_integer(self, value: i32) -> Result<Self::Value, load::Error> {
        Ok(Value::Integer(value))
    }

    #[inline]
    fn build_float(self, value: f64) -> Result<Self::Value, load::Error> {
        Ok(Value::Float(value))
    }

    #[inline]
    fn build_string<R: std::io::Read>( self,
        len: u32, mut read: R,
    ) -> Result<Self::Value, load::Error> {
        Ok(Value::String(self::string::str_from_len_read(len, &mut read)?))
    }

    fn build_table<T>(self, items: T) -> Result<Self::Value, load::Error>
    where T: load::LoadTableIterator<Key=Self::Key, Value=Self::Value> {
        let array_len = items.array_len();
        let assoc_loglen = items.assoc_loglen();
        let assoc_len = iexp2(assoc_loglen);
        let mut table = TableLoadBuilder::new(array_len, assoc_loglen);
        let mut array_index = 0;
        let mut assoc_index = 0;
        for item in items {
            let item = item?;
            match (item, array_index < array_len, assoc_index < assoc_len) {
                (Some(TableItem::Array(value)), true, _) => {
                    let index = array_index;
                    array_index += 1;
                    table.array_insert(index, value);
                },
                (None, true, _) => array_index += 1,
                (Some(TableItem::Array(_)), false, _) =>
                    panic!("unexpected array item"),
                (Some(TableItem::Assoc(assoc_item)), false, true) => {
                    let index = assoc_index;
                    assoc_index += 1;
                    table.assoc_insert(index, assoc_item);
                },
                (None, false, true) => assoc_index += 1,
                (Some(TableItem::Assoc(_)), true, _) =>
                    panic!("unexpected assoc item"),
                (_, false, false) =>
                    panic!("unexpected item"),
            }
        }
        Ok(Value::Table(table.finish()?))
    }

}
