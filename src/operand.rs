#![allow(clippy::use_self)]

use std::marker::PhantomData;

use serde::{
    Deserialize, Serialize,
};

use crate::{
    load::error::Error as LoadError,
    value::{self as v, Key, TableIntoError as TableError},
};

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Operand {

    // this can indicate either `Jump::Next` or a lack of value/place,
    // depending on operation.
    #[serde(rename="Unset")]
    UnknownUnset,

    // this can indicate either `Jump::Return` or a lack of value/place,
    // depending on operation.
    #[serde(rename="Skipped")]
    UnknownSkipped,

    // this can indicate either `Jump` or `Place`,
    // depending on operation.
    #[serde(rename="Index")]
    UnknownIndex(i32),

    // a next instruction in a branching instruction
    #[serde(untagged)]
    Jump(Jump),

    #[serde( untagged,
        serialize_with="serde_option_place::serialize" )]
    Place(Option<Place>),

    #[serde( untagged,
        serialize_with="Value::serialize_option" )]
    Value(Option<Value>),

}

impl Operand {
    #[must_use]
    pub fn unwrap_option(this: Option<Self>) -> Self {
        if let Some(this) = this { return this; }
        Self::UnknownUnset
    }
    pub fn make_jump(&mut self) -> Result<(), LoadError> {
        match *self {
            Self::Jump(_) => (),
            Self::Place(_) | Self::Value(_) => return Err(LoadError::from(
                "operand cannot be interpreted as a jump" )),
            Self::UnknownUnset => *self = Self::Jump(Jump::Next),
            Self::UnknownSkipped => *self = Self::Jump(Jump::Return),
            Self::UnknownIndex(index) => *self = Self::Jump(Jump::Jump(index)),
        }
        Ok(())
    }
    pub fn make_place(&mut self) -> Result<(), LoadError> {
        match *self {
            Self::Place(_) => (),
            Self::Jump(_) | Self::Value(_) => return Err(LoadError::from(
                "operand cannot be interpreted as a place" )),
            Self::UnknownUnset | Self::UnknownSkipped =>
                *self = Self::Place(None),
            Self::UnknownIndex(index) =>
                *self = Self::Place(Some(Place::try_from(index)?)),
        }
        Ok(())
    }
    pub fn make_value(&mut self) -> Result<(), LoadError> {
        match *self {
            Self::Value(_) => (),
            Self::Jump(_) | Self::Place(_) | Self::UnknownIndex(_) =>
                return Err(LoadError::from(
                    "operand cannot be interpreted as a value" )),
            Self::UnknownUnset | Self::UnknownSkipped =>
                *self = Self::Value(None),
        }
        Ok(())
    }
}

struct OperandVisitor;

impl<'de> serde::de::Visitor<'de> for OperandVisitor {
    type Value = Operand;
    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result
    {
        write!(fmt, "an operand")
    }
    fn visit_enum<A>(self, data: A) -> Result<Operand, A::Error>
    where A: serde::de::EnumAccess<'de>
    {
        use serde::de::VariantAccess;
        #[derive(Deserialize)]
        enum OperandType {
            Unset, Skipped, Index,
            Return, Next, Jump,
            SkippedPlace, Parameter, Register, Variable,
            SkippedValue, Number, Item, ItemCount, Coord, CoordCount,
        }
        use OperandType as T;
        let (op_name, contents) = data.variant::<OperandType>()?;
        Ok(match op_name {
            T::Unset => { contents.unit_variant()?;
                Operand::UnknownUnset },
            T::Skipped => { contents.unit_variant()?;
                Operand::UnknownSkipped },
            T::Index => {
                let index = contents.newtype_variant()?;
                Operand::UnknownIndex(index) },
            T::Return => { contents.unit_variant()?;
                Operand::Jump(Jump::Return) },
            T::Next => { contents.unit_variant()?;
                Operand::Jump(Jump::Next) },
            T::Jump => {
                let index = contents.newtype_variant()?;
                Operand::Jump(Jump::Jump(index)) },
            T::SkippedPlace => { contents.unit_variant()?;
                Operand::Place(None) },
            T::Parameter => {
                let index = contents.newtype_variant()?;
                Operand::Place(Some(Place::Parameter(index))) },
            T::Register => {
                let register = contents.newtype_variant()?;
                Operand::Place(Some(Place::Register(register))) },
            T::Variable => {
                let var_name = contents.newtype_variant()?;
                Operand::Place(Some(Place::Variable(var_name))) },
            T::SkippedValue => { contents.unit_variant()?;
                Operand::Value(None) },
            T::Number => {
                let count = contents.newtype_variant()?;
                Operand::Value(Some(Value::Number(count))) },
            T::Item => {
                let id = contents.newtype_variant()?;
                Operand::Value(Some(Value::Item(id))) },
            T::ItemCount => {
                let (id, count) = contents.tuple_variant(2, PairVisitor::new())?;
                Operand::Value(Some(Value::ItemCount(id, count))) },
            T::Coord => { let coord = contents.newtype_variant()?;
                Operand::Value(Some(Value::Coord(coord))) },
            T::CoordCount => {
                let (coord, count) = contents.tuple_variant(2, PairVisitor::new())?;
                Operand::Value(Some(Value::CoordCount(coord, count))) },
        })
    }
}

struct PairVisitor<V, W>(PhantomData<(V, W)>);

impl<A, B> PairVisitor<A, B> {
    fn new() -> Self { Self(PhantomData) }
}

impl<'de, V, W> serde::de::Visitor<'de> for PairVisitor<V, W>
where V: Deserialize<'de>, W: Deserialize<'de>
{
    type Value = (V, W);
    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "a pair of values")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<(V, W), A::Error>
    where A: serde::de::SeqAccess<'de>
    {
        let custom_err = <A::Error as serde::de::Error>::custom;
        let a = seq.next_element()?.ok_or_else( ||
            custom_err("missing first element of the pair"))?;
        let b = seq.next_element()?.ok_or_else( ||
            custom_err("missing second element of the pair"))?;
        Ok((a, b))
    }
}

impl<'de> Deserialize<'de> for Operand {
    fn deserialize<D>(de: D) -> Result<Operand, D::Error>
    where D: serde::de::Deserializer<'de>
    {
        let visitor = OperandVisitor;
        de.deserialize_enum("Operand", &[], visitor)
    }
}

impl TryFrom<Option<v::Value>> for Operand {
    type Error = LoadError;
    fn try_from(value: Option<v::Value>) -> Result<Operand, Self::Error> {
        Ok(Operand::unwrap_option(
            value.map(Operand::try_from).transpose()?
        ))
    }
}

impl TryFrom<v::Value> for Operand {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Operand, Self::Error> {
        Ok(match value {
            v::Value::Boolean(false) => Operand::UnknownSkipped,
            v::Value::Integer(index @ 1 ..= i32::MAX) =>
                Operand::UnknownIndex(index),
            v::Value::Integer(index @ -4 ..= -1) =>
                Operand::Place(Some(Place::Register(
                    Register::try_from(index)? ))),
            v::Value::String(name) =>
                Operand::Place(Some( Place::Variable(name) )),
            v::Value::Table(table) => Operand::Value(Some(
                Value::try_from(table)? )),
            v::Value::Float(_) => return Err(LoadError::from(
                "operand cannot be a float" )),
            v::Value::Boolean(true) => return Err(LoadError::from(
                "operand cannot be `true`" )),
            v::Value::Integer(i32::MIN ..= 0) =>
                return Err(LoadError::from(
                    "operand cannot be a negative number \
                     except for register codes" )),
        })
    }
}

impl From<Operand> for Option<v::Value> {
    fn from(this: Operand) -> Option<v::Value> {
        match this {
            Operand::Jump(index) => Option::<v::Value>::from(index),
            Operand::Place(Some(place)) => Some(v::Value::from(place)),
            Operand::Value(Some(value)) => Some(v::Value::from(value)),
            Operand::UnknownUnset => None,
            Operand::UnknownSkipped |
            Operand::Place(None) | Operand::Value(None)
                => Some(v::Value::Boolean(false)),
            Operand::UnknownIndex(index) => Some(v::Value::Integer(index)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Jump {
    Return,
    Next,
    /// `1`-based index in the instruction list.
    Jump(i32),
}

impl Jump {
    #[must_use]
    pub fn unwrap_option(this: Option<Self>) -> Self {
        if let Some(this) = this { return this; }
        Self::Next
    }
}

impl TryFrom<Option<v::Value>> for Jump {
    type Error = LoadError;
    fn try_from(value: Option<v::Value>) -> Result<Jump, Self::Error> {
        Ok(Jump::unwrap_option(
            value.map(Jump::try_from).transpose()? ))
    }
}

impl TryFrom<v::Value> for Jump {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Jump, Self::Error> {
        Ok(match value {
            v::Value::Boolean(false) => Jump::Return,
            v::Value::Integer(index) if index > 0 => Jump::Jump(index),
            _ => return Err(LoadError::from(
                "instruction jump reference should be either `false` or
                 a positive integer" ))
        })
    }
}

impl From<Jump> for Option<v::Value> {
    fn from(this: Jump) -> Option<v::Value> {
        match this {
            Jump::Jump(index) => Some(v::Value::Integer(index)),
            Jump::Next => None,
            Jump::Return => Some(v::Value::Boolean(false)),
        }
    }
}

/// Place arguments to instructions
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Place {
    Parameter(i32),
    Register(Register),
    Variable(String),
}

mod serde_option_place {
    use serde::Serialize;
    use super::Place;

    pub(super) fn serialize<S>(this: &Option<Place>, ser: S)
    -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        let Some(this) = this else {
            // XXX what about deserializing this?
            return ser.serialize_unit_variant(
                "Operand", 3, "SkippedPlace" );
        };
        this.serialize(ser)
    }
}

impl TryFrom<v::Value> for Place {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Place, Self::Error> {
        Ok(match value {
            v::Value::Integer(index) =>
                return Place::try_from(index),
            v::Value::String(name) => Place::Variable(name),
            _ => return Err(LoadError::from(
                "operand should be a string or an integer" )),
    })
    }
}

impl TryFrom<i32> for Place {
    type Error = LoadError;
    fn try_from(value: i32) -> Result<Place, Self::Error> {
        Ok(match value {
            index @ 1 ..= i32::MAX => Place::Parameter(index),
            index @ -4 ..= -1 => Place::Register(Register::try_from(index)?),
            i32::MIN ..= 0 => return Err(LoadError::from(
                "operand cannot be a negative number \
                 except for register codes" )),
        })
    }
}

impl From<Place> for v::Value {
    fn from(this: Place) -> v::Value {
        match this {
            Place::Parameter(index) => v::Value::Integer(index),
            Place::Register(register) => v::Value::from(register),
            Place::Variable(name) => v::Value::String(name),
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Register {
    Signal = -4,
    Visual = -3,
    Store  = -2,
    Goto   = -1,
}

impl TryFrom<v::Value> for Register {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Register, Self::Error> {
        Ok(match value {
            v::Value::Integer(index) => Register::try_from(index)?,
            _ => return Err(LoadError::from(
                "register should be encoded by an integer" )),
        })
    }
}

impl TryFrom<i32> for Register {
    type Error = LoadError;
    #[inline]
    fn try_from(value: i32) -> Result<Register, Self::Error> {
        Ok(match value {
            -4 => Register::Signal,
            -3 => Register::Visual,
            -2 => Register::Store,
            -1 => Register::Goto,
            _ => return Err(LoadError::from(
                "register should be encoded by a negative integer \
                 in `-4 .. -1` range" )),
        })
    }
}

impl From<Register> for v::Value {
    fn from(this: Register) -> v::Value {
        v::Value::Integer(this as i32)
    }
}

/// Value arguments to operations
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Value {
    Number(i32),
    Item(String),
    ItemCount(String, i32),
    Coord(Coord),
    CoordCount(Coord, i32),
}

impl Value {
    fn serialize_option<S>(this: &Option<Value>, ser: S)
    -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        let Some(this) = this else {
            // XXX what about deserializing this?
            return ser.serialize_unit_variant(
                "Operand", 3, "SkippedValue" );
        };
        this.serialize(ser)
    }
}

impl TryFrom<v::Value> for Value {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Value, Self::Error> {
        let v::Value::Table(table) = value else {
            return Err(LoadError::from(
                "value operand should be represented by a table value" ));
        };
        Value::try_from(table)
    }
}

impl TryFrom<v::Table> for Value {
    type Error = LoadError;
    fn try_from(table: v::Table) -> Result<Value, Self::Error> {
        fn err_from_table_index(error: TableError) -> LoadError {
            match error {
                TableError::NonContinuous(index) =>
                    err_unexpected_key(Key::Index(index)),
                TableError::UnexpectedKey(key) =>
                    err_unexpected_key(key),
            }
        }
        fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
            "value representation should not have {key:?} key" )) }
        fn id_ok(value: v::Value) -> Result<String, LoadError> {
            match value {
                v::Value::String(id) => Ok(id),
                _ => Err(LoadError::from("`id` value should be string")),
            }
        }
        fn num_ok(value: v::Value) -> Result<i32, LoadError> {
            match value {
                v::Value::Integer(num) => Ok(num),
                _ => Err(LoadError::from("`num` value should be integer")),
            }
        }
        let (mut id, mut coord, mut num) = (None, None, None);
        table.try_into_named(
            |name, value| { match name.as_str() {
                "id" => id = Some(id_ok(value)?),
                "coord" => coord = Some(Coord::try_from(value)?),
                "num" => num = Some(num_ok(value)?),
                _ => return Err(err_unexpected_key(Key::Name(name))),
            }; Ok(()) },
            err_from_table_index )?;
        Ok(match (id, coord, num) {
            (None, None, Some(num)) => Value::Number(num),
            (Some(id), None, None) => Value::Item(id),
            (Some(id), None, Some(num)) => Value::ItemCount(id, num),
            (None, Some(coord), None) => Value::Coord(coord),
            (None, Some(coord), Some(num)) => Value::CoordCount(coord, num),
            (None, None, None) => return Err(LoadError::from(
                "value representation should have at least one of the fields\
                 `id`, `coord`, `num`" )),
            (Some(_), Some(_), _) => return Err(LoadError::from(
                "value representation cannot have both\
                 `id` and `coord` fields" )),
        })
    }
}

impl From<Value> for v::Value {
    fn from(this: Value) -> v::Value {
        match this {
            Value::Number(number) => {
                let mut table = v::Table::dump_builder(Some(0), Some(0));
                table.assoc_insert_name("num", Some(v::Value::Integer(number)));
                v::Value::Table(table.finish())
            },
            Value::Coord(coord) | Value::CoordCount(coord, 0) => {
                let mut table = v::Table::dump_builder(Some(0), Some(0));
                table.assoc_insert_name("coord", Some(v::Value::from(coord)));
                v::Value::Table(table.finish())
            },
            Value::CoordCount(coord, num) => {
                let mut table = v::Table::dump_builder(Some(0), Some(1));
                table.assoc_insert_name("coord", Some(v::Value::from(coord)));
                table.assoc_insert_name("num", Some(v::Value::Integer(num)));
                v::Value::Table(table.finish())
            },
            Value::Item(id) | Value::ItemCount(id, 0) => {
                let mut table = v::Table::dump_builder(Some(0), Some(0));
                table.assoc_insert_name("id", Some(v::Value::String(id)));
                v::Value::Table(table.finish())
            },
            Value::ItemCount(id, num) => {
                let mut table = v::Table::dump_builder(Some(0), Some(1));
                table.assoc_insert_name("id", Some(v::Value::String(id)));
                table.assoc_insert_name("num", Some(v::Value::Integer(num)));
                v::Value::Table(table.finish())
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl TryFrom<v::Value> for Coord {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Coord, Self::Error> {
        let v::Value::Table(table) = value else {
            return Err(LoadError::from(
                "coord should be represented by a table value" ));
        };
        Coord::try_from(table)
    }
}

impl TryFrom<v::Table> for Coord {
    type Error = LoadError;
    fn try_from(table: v::Table) -> Result<Coord, Self::Error> {
        fn err_from_table_index(error: TableError) -> LoadError {
            match error {
                TableError::NonContinuous(index) =>
                    err_unexpected_key(Key::Index(index)),
                TableError::UnexpectedKey(key) =>
                    err_unexpected_key(key),
            }
        }
        fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
            "coord representation should not have {key:?} field" )) }
        fn i32_ok(value: v::Value) -> Result<i32, LoadError> {
            match value {
                v::Value::Integer(z) => Ok(z),
                _ => Err(LoadError::from(
                    "coord field values should be integers" )),
            }
        }
        let (mut x, mut y) = (None, None);
        table.try_into_named(
            |name, value| { match name.as_str() {
                "x" => x = Some(i32_ok(value)?),
                "y" => y = Some(i32_ok(value)?),
                _ => return Err(err_unexpected_key(Key::Name(name))),
            }; Ok(()) },
            err_from_table_index )?;
        let (Some(x), Some(y)) = (x, y) else {
            return Err(LoadError::from("coord must have `x` and `y` fields"));
        };
        Ok(Coord { x, y })
    }
}

impl From<Coord> for v::Value {
    fn from(this: Coord) -> v::Value {
        let mut table = v::Table::dump_builder(Some(0), Some(1));
        table.assoc_insert(Key::from("x"), Some(v::Value::Integer(this.x)));
        table.assoc_insert(Key::from("y"), Some(v::Value::Integer(this.y)));
        v::Value::Table(table.finish())
    }
}

#[cfg(test)]
mod test {

use super::{Coord, Operand, Place, Register, Value};

#[test]
fn test_operand_serde_ron() {
    for (op, op_str) in [
        (Operand::UnknownUnset,         "Unset"),
        (Operand::UnknownSkipped,       "Skipped"),
        (Operand::UnknownIndex(42),     "Index(42)"),
        (Operand::Place(None),          "SkippedPlace"),
        (Operand::Place(Some(Place::Parameter(42))),
                                        "Parameter(42)" ),
        (Operand::Place(Some(Place::Variable(String::from("ABC")))),
                                        "Variable(\"ABC\")" ),
        (Operand::Place(Some(Place::Register(Register::Signal))),
                                        "Register(Signal)" ),
        (Operand::Value(None),          "SkippedValue"),
        (Operand::Value(Some(Value::Number(42))),
                                        "Number(42)" ),
        (Operand::Value(Some(Value::Item(String::from("coconut")))),
                                        "Item(\"coconut\")" ),
        (Operand::Value(Some(Value::ItemCount(String::from("coconut"), 42))),
                                        "ItemCount(\"coconut\",42)" ),
        (Operand::Value(Some(Value::Coord(Coord { x: 42, y: -42 }))),
                                        "Coord((x:42,y:-42))" ),
        (Operand::Value(Some(Value::CoordCount(Coord { x: 42, y: -42 }, 42))),
                                        "CoordCount((x:42,y:-42),42)" ),
    ] {
        assert_eq!(ron::to_string(&op), Ok(String::from(op_str)));
        assert_eq!(Ok(op), ron::from_str::<Operand>(op_str));
    }
}

}

