#![allow(clippy::use_self)]

use std::marker::PhantomData;

use serde::{
    Serialize,
    Deserialize,
};

use crate::{
    error::LoadError,
    string::Str,
    value::{
        TableIntoError as TableError,
        Key, Value,
        Table, TableBuilder,
    },
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
        serialize_with="OpValue::serialize_option" )]
    Value(Option<OpValue>),

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
                Operand::Value(Some(OpValue::Number(count))) },
            T::Item => {
                let id = contents.newtype_variant()?;
                Operand::Value(Some(OpValue::Item(id))) },
            T::ItemCount => {
                let (id, count) = contents.tuple_variant(2, PairVisitor::new())?;
                Operand::Value(Some(OpValue::ItemCount(id, count))) },
            T::Coord => { let coord = contents.newtype_variant()?;
                Operand::Value(Some(OpValue::Coord(coord))) },
            T::CoordCount => {
                let (coord, count) = contents.tuple_variant(2, PairVisitor::new())?;
                Operand::Value(Some(OpValue::CoordCount(coord, count))) },
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

impl TryFrom<Option<Value>> for Operand {
    type Error = LoadError;
    fn try_from(value: Option<Value>) -> Result<Operand, Self::Error> {
        Ok(Operand::unwrap_option(
            value.map(Operand::try_from).transpose()?
        ))
    }
}

impl TryFrom<Value> for Operand {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Operand, Self::Error> {
        Ok(match value {
            Value::Boolean(false) => Operand::UnknownSkipped,
            Value::Integer(index @ 1 ..= i32::MAX) =>
                Operand::UnknownIndex(index),
            Value::Integer(index @ -4 ..= -1) =>
                Operand::Place(Some(Place::Register(
                    Register::try_from(index)? ))),
            Value::String(name) =>
                Operand::Place(Some( Place::Variable(name) )),
            Value::Table(table) => Operand::Value(Some(
                OpValue::try_from(table)? )),
            Value::Float(_) => return Err(LoadError::from(
                "operand cannot be a float" )),
            Value::Boolean(true) => return Err(LoadError::from(
                "operand cannot be `true`" )),
            Value::Integer(i32::MIN ..= 0) =>
                return Err(LoadError::from(
                    "operand cannot be a negative number \
                     except for register codes" )),
        })
    }
}

impl From<Operand> for Option<Value> {
    fn from(this: Operand) -> Option<Value> {
        match this {
            Operand::Jump(index) => Option::<Value>::from(index),
            Operand::Place(Some(place)) => Some(Value::from(place)),
            Operand::Value(Some(value)) => Some(Value::from(value)),
            Operand::UnknownUnset => None,
            Operand::UnknownSkipped |
            Operand::Place(None) | Operand::Value(None)
                => Some(Value::Boolean(false)),
            Operand::UnknownIndex(index) => Some(Value::Integer(index)),
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

impl TryFrom<Option<Value>> for Jump {
    type Error = LoadError;
    fn try_from(value: Option<Value>) -> Result<Jump, Self::Error> {
        Ok(Jump::unwrap_option(
            value.map(Jump::try_from).transpose()? ))
    }
}

impl TryFrom<Value> for Jump {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Jump, Self::Error> {
        Ok(match value {
            Value::Boolean(false) => Jump::Return,
            Value::Integer(index) if index > 0 => Jump::Jump(index),
            _ => return Err(LoadError::from(
                "instruction jump reference should be either `false` or
                 a positive integer" ))
        })
    }
}

impl From<Jump> for Option<Value> {
    fn from(this: Jump) -> Option<Value> {
        match this {
            Jump::Jump(index) => Some(Value::Integer(index)),
            Jump::Next => None,
            Jump::Return => Some(Value::Boolean(false)),
        }
    }
}

/// Place arguments to instructions
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Place {
    Parameter(i32),
    Register(Register),
    Variable(Str),
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

impl TryFrom<Value> for Place {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Place, Self::Error> {
        Ok(match value {
            Value::Integer(index) =>
                return Place::try_from(index),
            Value::String(name) => Place::Variable(name),
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

impl From<Place> for Value {
    fn from(this: Place) -> Value {
        match this {
            Place::Parameter(index) => Value::Integer(index),
            Place::Register(register) => Value::from(register),
            Place::Variable(name) => Value::String(name),
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

impl TryFrom<Value> for Register {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Register, Self::Error> {
        Ok(match value {
            Value::Integer(index) => Register::try_from(index)?,
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

impl From<Register> for Value {
    fn from(this: Register) -> Value {
        Value::Integer(this as i32)
    }
}

/// Value arguments to operations
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum OpValue {
    Number(i32),
    Item(Str),
    ItemCount(Str, i32),
    Coord(Coord),
    CoordCount(Coord, i32),
}

impl OpValue {
    fn serialize_option<S>(this: &Option<OpValue>, ser: S)
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

impl TryFrom<Value> for OpValue {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<OpValue, Self::Error> {
        let Value::Table(table) = value else {
            return Err(LoadError::from(
                "value operand should be represented by a table value" ));
        };
        OpValue::try_from(table)
    }
}

impl TryFrom<Table> for OpValue {
    type Error = LoadError;
    fn try_from(table: Table) -> Result<OpValue, Self::Error> {
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
        fn id_ok(value: Value) -> Result<Str, LoadError> {
            match value {
                Value::String(id) => Ok(id),
                _ => Err(LoadError::from("`id` value should be string")),
            }
        }
        fn num_ok(value: Value) -> Result<i32, LoadError> {
            match value {
                Value::Integer(num) => Ok(num),
                _ => Err(LoadError::from("`num` value should be integer")),
            }
        }
        let (mut id, mut coord, mut num) = (None, None, None);
        table.try_into_named(
            |name, value| { match name.as_ref() {
                "id" => id = Some(id_ok(value)?),
                "coord" => coord = Some(Coord::try_from(value)?),
                "num" => num = Some(num_ok(value)?),
                _ => return Err(err_unexpected_key(Key::Name(name))),
            }; Ok(()) },
            err_from_table_index )?;
        Ok(match (id, coord, num) {
            (None, None, Some(num)) => OpValue::Number(num),
            (Some(id), None, None) => OpValue::Item(id),
            (Some(id), None, Some(num)) => OpValue::ItemCount(id, num),
            (None, Some(coord), None) => OpValue::Coord(coord),
            (None, Some(coord), Some(num)) => OpValue::CoordCount(coord, num),
            (None, None, None) => return Err(LoadError::from(
                "value representation should have at least one of the fields\
                 `id`, `coord`, `num`" )),
            (Some(_), Some(_), _) => return Err(LoadError::from(
                "value representation cannot have both\
                 `id` and `coord` fields" )),
        })
    }
}

impl From<OpValue> for Value {
    fn from(this: OpValue) -> Value {
        match this {
            OpValue::Number(number) => {
                let mut table = TableBuilder::new(0, Some(0));
                table.assoc_insert("num", Some(Value::Integer(number)));
                Value::Table(table.finish())
            },
            OpValue::Coord(coord) | OpValue::CoordCount(coord, 0) => {
                let mut table = TableBuilder::new(0, Some(0));
                table.assoc_insert("coord", Some(Value::from(coord)));
                Value::Table(table.finish())
            },
            OpValue::CoordCount(coord, num) => {
                let mut table = TableBuilder::new(0, Some(1));
                table.assoc_insert("coord", Some(Value::from(coord)));
                table.assoc_insert("num", Some(Value::Integer(num)));
                Value::Table(table.finish())
            },
            OpValue::Item(id) | OpValue::ItemCount(id, 0) => {
                let mut table = TableBuilder::new(0, Some(0));
                table.assoc_insert("id", Some(Value::String(id)));
                Value::Table(table.finish())
            },
            OpValue::ItemCount(id, num) => {
                let mut table = TableBuilder::new(0, Some(1));
                table.assoc_insert("id", Some(Value::String(id)));
                table.assoc_insert("num", Some(Value::Integer(num)));
                Value::Table(table.finish())
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl TryFrom<Value> for Coord {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Coord, Self::Error> {
        let Value::Table(table) = value else {
            return Err(LoadError::from(
                "coord should be represented by a table value" ));
        };
        Coord::try_from(table)
    }
}

impl TryFrom<Table> for Coord {
    type Error = LoadError;
    fn try_from(table: Table) -> Result<Coord, Self::Error> {
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
        fn i32_ok(value: Value) -> Result<i32, LoadError> {
            match value {
                Value::Integer(z) => Ok(z),
                _ => Err(LoadError::from(
                    "coord field values should be integers" )),
            }
        }
        let (mut x, mut y) = (None, None);
        table.try_into_named(
            |name, value| { match name.as_ref() {
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

impl From<Coord> for Value {
    fn from(this: Coord) -> Value {
        let mut table = TableBuilder::new(0, Some(1));
        table.assoc_insert("x", Some(Value::Integer(this.x)));
        table.assoc_insert("y", Some(Value::Integer(this.y)));
        Value::Table(table.finish())
    }
}

#[cfg(test)]
mod test {

use crate::string::Str;

use super::{Coord, Operand, Place, Register, OpValue};

#[test]
fn test_operand_serde_ron() {
    for (o, s) in [
        (Operand::UnknownUnset,         "Unset"),
        (Operand::UnknownSkipped,       "Skipped"),
        (Operand::UnknownIndex(42),     "Index(42)"),
        (Operand::Place(None),          "SkippedPlace"),
        (Operand::Place(Some(Place::Parameter(42))),
                                        "Parameter(42)" ),
        (Operand::Place(Some(Place::Variable(Str::from("ABC")))),
                                        "Variable(\"ABC\")" ),
        (Operand::Place(Some(Place::Register(Register::Signal))),
                                        "Register(Signal)" ),
        (Operand::Value(None),          "SkippedValue"),
        (Operand::Value(Some(OpValue::Number(42))),
                                        "Number(42)" ),
        (Operand::Value(Some(OpValue::Item(Str::from("coconut")))),
                                        "Item(\"coconut\")" ),
        (Operand::Value(Some(OpValue::ItemCount(Str::from("coconut"), 42))),
                                        "ItemCount(\"coconut\",42)" ),
        (Operand::Value(Some(OpValue::Coord(Coord { x: 42, y: -42 }))),
                                        "Coord((x:42,y:-42))" ),
        (Operand::Value(Some(OpValue::CoordCount(Coord { x: 42, y: -42 }, 42))),
                                        "CoordCount((x:42,y:-42),42)" ),
    ] {
        let as_str = String::as_str;
        let ron_result = ron::to_string(&o);
        assert_eq!(ron_result.as_ref().map(as_str), Ok(s));
        assert_eq!(Ok(o), ron::from_str(s));
    }
}

}

