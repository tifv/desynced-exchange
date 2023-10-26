use std::collections::HashMap;

use crate::string::Str;

pub enum Value {
    Nil,
    Boolean(bool),
    Integer(i32),
    String(Str),
    Table(HashMap<Key, Value>)
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Index(i32),
    Name(Str),
}
