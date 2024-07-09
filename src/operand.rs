#![allow(clippy::use_self)]

use serde::{
    Deserialize, de, Serialize,
};

use crate::{
    error::LoadError,
    string::Str,
    serde::{
        Identifier, PairVisitor,
        DeserializeOption, forward_de_to_de_option,
        SerializeOption,
    },
    value::{
        Key, Value as _Value, Table,
    }
};

enum EnumMatchError<'de, E, V> {
    DeErr(E),
    NoMatch(Identifier<'de>, V),
}

trait EnumTryVisitor<'de> : de::Visitor<'de> {
    fn visit_enum_match<V>(self, id: Identifier<'de>, contents: V)
    -> Result<Self::Value, EnumMatchError<'de, V::Error, V>>
    where V: de::VariantAccess<'de>;
}


#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {

    // this can indicate either `Jump::Next` or a lack of value/place,
    // depending on operation.
    UnknownUnset,

    // this can indicate either `Jump::Return` or a lack of value/place,
    // depending on operation.
    UnknownSkipped,

    // this can indicate either `Jump` or `Place`,
    // depending on operation.
    UnknownIndex(i32),

    // a next instruction in a branching instruction
    Jump(Jump),

    Place(Option<Place>),

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

impl TryFrom<Option<_Value>> for Operand {
    type Error = LoadError;
    fn try_from(value: Option<_Value>) -> Result<Operand, Self::Error> {
        Ok(Operand::unwrap_option(
            value.map(Operand::try_from).transpose()?
        ))
    }
}

impl TryFrom<_Value> for Operand {
    type Error = LoadError;
    fn try_from(value: _Value) -> Result<Operand, Self::Error> {
        Ok(match value {
            _Value::Boolean(false) => Operand::UnknownSkipped,
            _Value::Integer(index @ 1 ..= i32::MAX) =>
                Operand::UnknownIndex(index),
            _Value::Integer(index @ -4 ..= -1) =>
                Operand::Place(Some(Place::Register(
                    Register::try_from(index)? ))),
            _Value::String(name) =>
                Operand::Place(Some( Place::Variable(name) )),
            _Value::Table(table) => Operand::Value(Some(
                Value::try_from(table)? )),
            _Value::Float(_) => return Err(LoadError::from(
                "operand cannot be a float" )),
            _Value::Boolean(true) => return Err(LoadError::from(
                "operand cannot be `true`" )),
            _Value::Integer(i32::MIN ..= 0) =>
                return Err(LoadError::from(
                    "operand cannot be a negative number \
                     except for register codes" )),
        })
    }
}

impl From<Operand> for Option<_Value> {
    fn from(this: Operand) -> Option<_Value> {
        match this {
            Operand::Jump(index) => Option::<_Value>::from(index),
            Operand::Place(Some(place)) => Some(_Value::from(place)),
            Operand::Value(Some(value)) => Some(_Value::from(value)),
            Operand::UnknownUnset => None,
            Operand::UnknownSkipped |
            Operand::Place(None) | Operand::Value(None)
                => Some(_Value::Boolean(false)),
            Operand::UnknownIndex(index) => Some(_Value::Integer(index)),
        }
    }
}

impl<'de> Deserialize<'de> for Operand {
    fn deserialize<D>(de: D) -> Result<Operand, D::Error>
    where D: de::Deserializer<'de>
    {
        let visitor = OperandVisitor;
        de.deserialize_enum("Operand", &[], visitor)
    }
}

struct OperandVisitor;

impl<'de> EnumTryVisitor<'de> for OperandVisitor {
    fn visit_enum_match<V>(self, mut id: Identifier<'de>, mut contents: V)
    -> Result<Self::Value, EnumMatchError<'de, V::Error, V>>
    where V: de::VariantAccess<'de>
    {
        use EnumMatchError::{NoMatch, DeErr};
        match id.as_ref() {
            "Unset" => return Ok(Operand::UnknownUnset),
            "Skipped" => return Ok(Operand::UnknownSkipped),
            "Index" => return Ok(Operand::UnknownIndex(
                contents.newtype_variant().map_err(DeErr)? )),
            _ => (),
        }
        macro_rules! try_visitor {
            ($visitor:ident, $variant:path) => {
                (id, contents) = match $visitor.visit_enum_match(id, contents) {
                    Ok(value) => return Ok($variant(value)),
                    Err(DeErr(err)) => return Err(DeErr(err)),
                    Err(NoMatch(id, contents)) => (id, contents),
                };
            };
        }
        try_visitor!(JumpVisitor, Operand::Jump );
        try_visitor!(PlaceVisitor, Operand::Place);
        try_visitor!(ValueVisitor, Operand::Value);
        Err(NoMatch(id, contents))
    }
}

impl<'de> de::Visitor<'de> for OperandVisitor {
    type Value = Operand;
    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result
    {
        write!(fmt, "an operand")
    }
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where A: de::EnumAccess<'de>
    {
        use serde::de::Error as _;
        let (id, contents) = data.variant()?;
        match self.visit_enum_match(id, contents) {
            Ok(value) => Ok(value),
            Err(EnumMatchError::DeErr(error)) => Err(error),
            Err(EnumMatchError::NoMatch(id, _)) => Err(A::Error::custom(
                format!("name {id:?} is not a known Operand variant") )),
        }
    }
}

impl Serialize for Operand {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        match self {
            Operand::UnknownUnset =>
                ser.serialize_unit_variant("Operand", 0, "Unset"),
            Operand::UnknownSkipped =>
                ser.serialize_unit_variant("Operand", 0, "Skipped"),
            Operand::UnknownIndex(index) =>
                ser.serialize_newtype_variant("Operand", 0, "Index", index),
            Operand::Jump (jump) => Jump::serialize(jump, ser),
            Operand::Place(place) => Place::serialize_option(place.as_ref(), ser),
            Operand::Value(value) => Value::serialize_option(value.as_ref(), ser),
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

impl TryFrom<Option<_Value>> for Jump {
    type Error = LoadError;
    fn try_from(value: Option<_Value>) -> Result<Jump, Self::Error> {
        Ok(Jump::unwrap_option(
            value.map(Jump::try_from).transpose()? ))
    }
}

impl TryFrom<_Value> for Jump {
    type Error = LoadError;
    fn try_from(value: _Value) -> Result<Jump, Self::Error> {
        Ok(match value {
            _Value::Boolean(false) => Jump::Return,
            _Value::Integer(index) if index > 0 => Jump::Jump(index),
            _ => return Err(LoadError::from(
                "instruction jump reference should be either `false` or
                 a positive integer" ))
        })
    }
}

impl From<Jump> for Option<_Value> {
    fn from(this: Jump) -> Option<_Value> {
        match this {
            Jump::Jump(index) => Some(_Value::Integer(index)),
            Jump::Next => None,
            Jump::Return => Some(_Value::Boolean(false)),
        }
    }
}

struct JumpVisitor;

impl<'de> EnumTryVisitor<'de> for JumpVisitor {
    fn visit_enum_match<V>(self, id: Identifier<'de>, contents: V)
    -> Result<Self::Value, EnumMatchError<'de, V::Error, V>>
    where V: de::VariantAccess<'de>
    {
        use EnumMatchError::{NoMatch, DeErr};
        Ok(match id.as_ref() {
            "Return" => Jump::Return,
            "Next"   => Jump::Next,
            "Jump"   => Jump::Jump(
                contents.newtype_variant().map_err(DeErr)? ),
            _ => return Err(NoMatch(id, contents)),
        })
    }
}

impl<'de> de::Visitor<'de> for JumpVisitor {
    type Value = Jump;
    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result
    {
        write!(fmt, "a jump operand")
    }
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where A: de::EnumAccess<'de>
    {
        use serde::de::Error as _;
        let (id, contents) = data.variant()?;
        match self.visit_enum_match(id, contents) {
            Ok(value) => Ok(value),
            Err(EnumMatchError::DeErr(error)) => Err(error),
            Err(EnumMatchError::NoMatch(id, _)) => Err(A::Error::custom(
                format!("name {id:?} is not a known Jump variant") )),
        }
    }
}


/// Place arguments to instructions
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Place {
    Parameter(i32),
    Register(Register),
    Variable(Str),
}

impl TryFrom<_Value> for Place {
    type Error = LoadError;
    fn try_from(value: _Value) -> Result<Place, Self::Error> {
        Ok(match value {
            _Value::Integer(index) =>
                return Place::try_from(index),
            _Value::String(name) => Place::Variable(name),
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

impl From<Place> for _Value {
    fn from(this: Place) -> _Value {
        match this {
            Place::Parameter(index) => _Value::Integer(index),
            Place::Register(register) => _Value::from(register),
            Place::Variable(name) => _Value::String(name),
        }
    }
}

impl<'de> DeserializeOption<'de> for Place {
    fn deserialize_option<D>(de: D)
    -> Result<Option<Self>, D::Error>
    where D: de::Deserializer<'de>
    {
        de.deserialize_enum("Place", &[], PlaceVisitor)
    }
}

forward_de_to_de_option!(Place);

struct PlaceVisitor;

impl<'de> EnumTryVisitor<'de> for PlaceVisitor {
    fn visit_enum_match<V>(self, id: Identifier<'de>, contents: V)
    -> Result<Self::Value, EnumMatchError<'de, V::Error, V>>
    where V: de::VariantAccess<'de>
    {
        use EnumMatchError::{NoMatch, DeErr};
        Ok(match id.as_ref() {
            "SkippedPlace" => None,
            "Parameter"    => Some(Place::Parameter(
                contents.newtype_variant().map_err(DeErr)? )),
            "Register"    => Some(Place::Register(
                contents.newtype_variant().map_err(DeErr)? )),
            "Variable"    => Some(Place::Variable(
                contents.newtype_variant().map_err(DeErr)? )),
            _ => return Err(NoMatch(id, contents)),
        })
    }
}

impl<'de> de::Visitor<'de> for PlaceVisitor {
    type Value = Option<Place>;
    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result
    {
        write!(fmt, "a place operand")
    }
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(None)
    }
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where A: de::EnumAccess<'de>
    {
        use serde::de::Error as _;
        let (id, contents) = data.variant()?;
        match self.visit_enum_match(id, contents) {
            Ok(value) => Ok(value),
            Err(EnumMatchError::DeErr(error)) => Err(error),
            Err(EnumMatchError::NoMatch(id, _)) => Err(A::Error::custom(
                format!("name {id:?} is not a known Jump variant") )),
        }
    }
}

impl SerializeOption for Place {
    fn serialize_option<S>(this: Option<&Self>, ser: S)
    -> Result<S::Ok, S::Error>
    where S: serde::Serializer
    {
        let Some(this) = this else {
            return ser.serialize_unit_variant("Place", 0, "SkippedPlace")
        };
        this.serialize(ser)
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

impl TryFrom<_Value> for Register {
    type Error = LoadError;
    fn try_from(value: _Value) -> Result<Register, Self::Error> {
        Ok(match value {
            _Value::Integer(index) => Register::try_from(index)?,
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

impl From<Register> for _Value {
    fn from(this: Register) -> _Value {
        _Value::Integer(this as i32)
    }
}


/// Value arguments to operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Value {
    Number(i32),
    Item(Str),
    ItemCount(Str, i32),
    Coord(Coord),
    CoordCount(Coord, i32),
}

impl TryFrom<_Value> for Value {
    type Error = LoadError;
    fn try_from(value: _Value) -> Result<Value, Self::Error> {
        let _Value::Table(table) = value else {
            return Err(LoadError::from(
                "value operand should be represented by a table value" ));
        };
        Value::try_from(table)
    }
}

impl TryFrom<Table> for Value {
    type Error = LoadError;
    fn try_from(table: Table) -> Result<Value, Self::Error> {
        fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
            "value representation should not have {key:?} key" )) }
        fn id_ok(value: _Value) -> Result<Str, LoadError> {
            match value {
                _Value::String(id) => Ok(id),
                _ => Err(LoadError::from("`id` value should be string")),
            }
        }
        fn num_ok(value: _Value) -> Result<i32, LoadError> {
            match value {
                _Value::Integer(num) => Ok(num),
                _ => Err(LoadError::from("`num` value should be integer")),
            }
        }
        let (mut id, mut coord, mut num) = (None, None, None);
        for (key, value) in table {
            match key.as_name() {
                Some("id")    => id = Some(id_ok(value)?),
                Some("coord") => coord = Some(Coord::try_from(value)?),
                Some("num")   => num = Some(num_ok(value)?),
                _ => return Err(err_unexpected_key(key)),
            }
        }
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

impl From<Value> for _Value {
    fn from(this: Value) -> _Value {
        match this {
            Value::Number(number) =>
                _Value::Table(Table::from_iter([
                    ("num"  , _Value::Integer(number)),
                ])),
            Value::Coord(coord) | Value::CoordCount(coord, 0) =>
                _Value::Table(Table::from_iter([
                    ("coord", _Value::from(coord)),
                ])),
            Value::CoordCount(coord, num) =>
                _Value::Table(Table::from_iter([
                    ("coord", _Value::from(coord)),
                    ("num"  , _Value::Integer(num)),
                ])),
            Value::Item(id) | Value::ItemCount(id, 0) =>
                _Value::Table(Table::from_iter([
                    ("id"   , _Value::String(id)),
                ])),
            Value::ItemCount(id, num) =>
                _Value::Table(Table::from_iter([
                    ("id"   , _Value::String(id)),
                    ("num"  , _Value::Integer(num)),
                ])),
        }
    }
}

impl<'de> DeserializeOption<'de> for Value {
    fn deserialize_option<D>(de: D)
    -> Result<Option<Self>, D::Error>
    where D: de::Deserializer<'de>
    {
        de.deserialize_enum("Value", &[], ValueVisitor)
    }
}

forward_de_to_de_option!(Value);

struct ValueVisitor;

impl<'de> EnumTryVisitor<'de> for ValueVisitor {
    fn visit_enum_match<V>(self, id: Identifier<'de>, contents: V)
    -> Result<Self::Value, EnumMatchError<'de, V::Error, V>>
    where V: de::VariantAccess<'de>
    {
        use EnumMatchError::{NoMatch, DeErr};
        Ok(match id.as_ref() {
            "SkippedValue" => None,
            "Number" => Some(Value::Number(
                contents.newtype_variant().map_err(DeErr)? )),
            "Item" => Some(Value::Item(
                contents.newtype_variant().map_err(DeErr)? )),
            "ItemCount" => {
                let (item, count) =
                    contents.tuple_variant(2, PairVisitor::new()).map_err(DeErr)?;
                Some(Value::ItemCount(item, count))
            },
            "Coord" => Some(Value::Coord(
                contents.newtype_variant().map_err(DeErr)? )),
            "CoordCount" => {
                let (coord, count) =
                    contents.tuple_variant(2, PairVisitor::new()).map_err(DeErr)?;
                Some(Value::CoordCount(coord, count))
            },
            _ => return Err(NoMatch(id, contents)),
        })
    }
}

impl<'de> de::Visitor<'de> for ValueVisitor {
    type Value = Option<Value>;
    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result
    {
        write!(fmt, "a value operand")
    }
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where E: de::Error
    {
        Ok(None)
    }
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where A: de::EnumAccess<'de>
    {
        use serde::de::Error as _;
        let (id, contents) = data.variant()?;
        match self.visit_enum_match(id, contents) {
            Ok(value) => Ok(value),
            Err(EnumMatchError::DeErr(error)) => Err(error),
            Err(EnumMatchError::NoMatch(id, _)) => Err(A::Error::custom(
                format!("name {id:?} is not a known Jump variant") )),
        }
    }
}

impl SerializeOption for Value {
    fn serialize_option<S>(this: Option<&Self>, ser: S)
    -> Result<S::Ok, S::Error>
    where S: serde::Serializer
    {
        let Some(this) = this else {
            return ser.serialize_unit_variant("Value", 0, "SkippedValue");
        };
        this.serialize(ser)
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl TryFrom<_Value> for Coord {
    type Error = LoadError;
    fn try_from(value: _Value) -> Result<Coord, Self::Error> {
        let _Value::Table(table) = value else {
            return Err(LoadError::from(
                "coord should be represented by a table value" ));
        };
        Coord::try_from(table)
    }
}

impl TryFrom<Table> for Coord {
    type Error = LoadError;
    fn try_from(table: Table) -> Result<Coord, Self::Error> {
        fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
            "coord representation should not have {key:?} field" )) }
        fn i32_ok(value: _Value) -> Result<i32, LoadError> {
            match value {
                _Value::Integer(z) => Ok(z),
                _ => Err(LoadError::from(
                    "coord field values should be integers" )),
            }
        }
        let (mut x, mut y) = (None, None);
        for (key, value) in table {
            match key.as_name() {
                Some("x") => x = Some(i32_ok(value)?),
                Some("y") => y = Some(i32_ok(value)?),
                _ => return Err(err_unexpected_key(key)),
            }
        }
        let (Some(x), Some(y)) = (x, y) else {
            return Err(LoadError::from("coord must have `x` and `y` fields"));
        };
        Ok(Coord { x, y })
    }
}

impl From<Coord> for _Value {
    fn from(this: Coord) -> _Value {
        _Value::Table(Table::from_iter([
            ("x", _Value::Integer(this.x)),
            ("y", _Value::Integer(this.y)),
        ]))
    }
}


#[cfg(test)]
mod test {

use crate::string::Str;

use super::{Coord, Operand, Place, Register, Value};

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
        (Operand::Value(Some(Value::Number(42))),
                                        "Number(42)" ),
        (Operand::Value(Some(Value::Item(Str::from("coconut")))),
                                        "Item(\"coconut\")" ),
        (Operand::Value(Some(Value::ItemCount(Str::from("coconut"), 42))),
                                        "ItemCount(\"coconut\",42)" ),
        (Operand::Value(Some(Value::Coord(Coord { x: 42, y: -42 }))),
                                        "Coord((x:42,y:-42))" ),
        (Operand::Value(Some(Value::CoordCount(Coord { x: 42, y: -42 }, 42))),
                                        "CoordCount((x:42,y:-42),42)" ),
    ] {
        let as_str = String::as_str;
        let ron_result = ron::to_string(&o);
        assert_eq!(ron_result.as_ref().map(as_str), Ok(s));
        assert_eq!(Ok(o), ron::from_str(s));
    }
}

}

