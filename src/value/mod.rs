mod string;
mod table;

pub use string::Str;
pub use table::Table;

use crate::{
    dump,
    load,
    table::{TableItem, AssocItem, iexp2},
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

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nil => f.write_str("nil"),
            Self::Boolean(value) => value.fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Float(value) => value.fmt(f),
            Self::String(value) => value.fmt(f),
            Self::Table(table) => table.fmt(f),
        }
    }
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
    fn load_key<KLL: load::Loader>(loader: KLL)
    -> Result<Option<Self>, KLL::Error> {
        loader.load_key(KeyBuilder)
    }
}

impl load::Load for Value {
    #[inline]
    fn load<LL: load::Loader>(loader: LL)
    -> Result<Self, LL::Error> {
        loader.load_value(ValueBuilder)
    }
    #[inline]
    fn is_nil(&self) -> bool { matches!(self, Self::Nil) }
}

struct KeyBuilder;

impl load::KeyBuilder for KeyBuilder {
    type Value = Key;

    #[inline]
    fn build_integer<E: load::Error>(self, value: i32) -> Result<Self::Value, E> {
        Ok(Key::Index(value))
    }

    #[inline]
    fn build_string<E: load::Error>( self,
        value: &str
    ) -> Result<Self::Value, E> {
        Ok(Key::Name(
            self::string::Str::from(value)
        ))
    }
}

struct ValueBuilder;

impl load::Builder for ValueBuilder {
    type Key = Key;
    type Value = Value;

    #[inline]
    fn build_nil<E: load::Error>(self) -> Result<Self::Value, E> {
        Ok(Value::Nil)
    }

    #[inline]
    fn build_boolean<E: load::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Boolean(value))
    }

    #[inline]
    fn build_integer<E: load::Error>(self, value: i32) -> Result<Self::Value, E> {
        Ok(Value::Integer(value))
    }

    #[inline]
    fn build_float<E: load::Error>(self, value: f64) -> Result<Self::Value, E> {
        Ok(Value::Float(value))
    }

    #[inline]
    fn build_string<E: load::Error>( self,
        value: &str,
    ) -> Result<Self::Value, E> {
        Ok(Value::String(
            self::string::Str::from(value)
        ))
    }

    fn build_table<T, E: load::Error>(self, items: T) -> Result<Self::Value, E>
    where T: load::LoadTableIterator<Key=Self::Key, Value=Self::Value, Error=E> {
        let array_len = items.array_len();
        let assoc_loglen = items.assoc_loglen();
        let assoc_len = iexp2(assoc_loglen);
        let mut table = TableLoadBuilder::new(array_len, assoc_loglen);
        table.set_last_free(items.assoc_last_free());
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
        Ok(Value::Table(table.finish::<E>()?))
    }

}

#[cfg(test)]
pub(crate) mod test {
    use super::Value;

    use crate::{test, dump, load};

    #[test]
    fn test_1_load() {
        let decompress = load::decompress::decompress;
        let decode = load::value::decode_blueprint::<Value, Value>;
        let encode = dump::value::encode_blueprint::<Value, Value>;
        let compress = dump::compress::compress;

        let exchange = test::EXCHANGE_BEHAVIOR_1_UNIT;
        let encoded = decompress(exchange)
            .unwrap();
        let value = decode(encoded.clone())
            .unwrap();
        let reencoded = encode(value)
            .unwrap();
        assert_eq!(encoded, reencoded);
        let reexchange = compress(reencoded.as_deref());
        let revalue = decode(decompress(&reexchange).unwrap()).unwrap();
        assert_eq!(reencoded, encode(revalue).unwrap());
    }

}

