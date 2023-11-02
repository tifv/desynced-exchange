#![allow(clippy::use_self)]

use std::collections::HashMap;

use crate::{
    value::{self, Key, Value, Table, TableIntoError},
    table::u32_to_usize,
};

mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[error("Load error: {reason}")]
    pub struct Error {
        reason: String,
    }

    impl From<&str> for Error {
        fn from(reason: &str) -> Self {
            Self{reason: String::from(reason)}
        }
    }

    impl From<String> for Error {
        fn from(reason: String) -> Self {
            Self{reason}
        }
    }
}

use error::Error;

#[derive(Debug, Clone, Default)]
pub struct Behavior {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parameters: Vec<Parameter>,
    pub instructions: Vec<Instruction>,
    pub subroutines: Vec<Behavior>,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: Option<String>,
    pub is_input: bool,
}

impl TryFrom<Value> for Behavior {
    type Error = Error;
    fn try_from(value: Value) -> Result<Behavior, Self::Error> {
        let Value::Table(table) = value else {
            return Err(Error::from(
                "behavior should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<Table> for Behavior {
    type Error = Error;
    fn try_from(table: Table) -> Result<Behavior, Self::Error> {
        BehaviorBuilder::build_from(table)
    }
}

#[derive(Default)]
struct BehaviorBuilder {
    name: Option<String>,
    description: Option<String>,
    parameters: Vec<Parameter>,
    parameter_names: Option<Table>,
    instructions: Vec<Instruction>,
    subroutines: Vec<Behavior>,
}

impl BehaviorBuilder {

    fn build_from(table: Table) -> Result<Behavior, Error> {
        let mut this = BehaviorBuilder::default();
        let vector: Vec<_> = table.try_into_seq_and_named(
            |name, value| match name.as_str() {
                "name"       => this.set_name           (value),
                "desc"       => this.set_description    (value),
                "parameters" => this.set_parameters     (value),
                "pnames"     => this.set_parameter_names(value),
                "subs"       => this.set_subroutines    (value),
                _ => Err(Self::err_unexpected_key(Key::Name(name))),
            },
            Self::err_from_table_index )?;
        this.instructions.reserve_exact(vector.len());
        for value in vector {
            this.instructions.push(Instruction::try_from(value)?);
        }
        this.finish()
    }

    fn err_from_table_index(error: TableIntoError) -> Error {
        match error {
            TableIntoError::NonContinuous(index) =>
                Self::err_non_continuous(index),
            TableIntoError::UnexpectedKey(key) =>
                Self::err_unexpected_key(key),
        }
    }

    fn err_non_continuous(index: i32) -> Error { Error::from(format!(
        "behavior representation should have \
         instruction indices in a continuous range `1..n`: {index:?}" )) }

    fn err_unexpected_key(key: Key) -> Error { Error::from(format!(
        "behavior representation should not have {key:?} key" )) }

    fn set_name(&mut self, name: Value) -> Result<(), Error> {
        let Value::String(name) = name else {
            return Err(Error::from(
                "behavor's name should be a string" ));
        };
        self.name = Some(name); Ok(())
    }

    fn set_description(&mut self, description: Value) -> Result<(), Error> {
        let Value::String(description) = description else {
            return Err(Error::from(
                "behavor's description should be a string" ));
        };
        self.description = Some(description); Ok(())
    }

    fn set_parameters(&mut self, parameters: Value) -> Result<(), Error> {
        let Value::Table(parameters) = parameters else {
            return Err(Self::err_parameters());
        };
        let parameters = Vec::try_from(parameters)
            .map_err(|error| Self::err_parameters())?;
        let this = &mut self.parameters;
        *this = Vec::with_capacity(parameters.len());
        for value in parameters {
            let Value::Boolean(value) = value else {
                return Err(Self::err_parameters());
            };
            this.push(Parameter{is_input: value, name: None});
        }
        Ok(())
    }
    fn err_parameters() -> Error { Error::from(
        "behavior's parameters should be \
         a continuous array of booleans" ) }

    fn set_parameter_names(&mut self, parameter_names: Value)
    -> Result<(), Error> {
        let Value::Table(parameter_names) = parameter_names else {
            return Err(Self::err_param_names());
        };
        self.parameter_names = Some(parameter_names);
        Ok(())
    }
    fn reconcile_parameter_names(
        parameters: &mut [Parameter],
        parameter_names: Table,
    ) -> Result<(), Error> {
        for (index, value) in parameter_names {
            let Some(index) = index.as_index()
                .map(usize::try_from).and_then(Result::ok)
                .filter(|&x| x > 0) 
            else {
                return Err(Self::err_param_names());
            };
            if index >= parameters.len() {
                return Err(Error::from(
                    "the number of behavior's parameters is inconsistent with \
                    the number of parameter names" ));
            }
            let Value::String(value) = value else {
                return Err(Self::err_param_names());
            };
            parameters[index].name = Some(value);
        }
        Ok(())
    }
    fn err_param_names() -> Error { Error::from(
        "behavior's parameter names should be \
         an array of strings or nils" ) }

    fn set_subroutines(&mut self, subroutines: Value)
    -> Result<(), Error> {
        let Value::Table(subroutines) = subroutines else {
            return Err(Self::err_subroutines());
        };
        let subroutines = Vec::try_from(subroutines)
            .map_err(|error| Self::err_subroutines())?;
        let this = &mut self.subroutines;
        *this = Vec::with_capacity(subroutines.len());
        for value in subroutines {
            this.push(Behavior::try_from(value)?);
        }
        Ok(())
    }
    fn err_subroutines() -> Error { Error::from(
        "behavior's subroutines should be \
         a continuous array" ) }

    fn finish(self) -> Result<Behavior, Error> {
        let Self{
            name, description,
            mut parameters, parameter_names,
            instructions,
            subroutines,
        } = self;
        if let Some(parameter_names) = parameter_names {
            Self::reconcile_parameter_names(&mut parameters, parameter_names)?;
        }
        Ok(Behavior{
            name, description,
            parameters,
            instructions,
            subroutines,
        })
    }

}

impl From<Behavior> for Value {
    fn from(this: Behavior) -> Value {
        let mut table = Table::builder();
        table.expect_array_len(this.instructions.len());
        for (i, instruction) in this.instructions.into_iter().enumerate() {
            let index: i32 = (i + 1).try_into()
                .expect("instruction count should not overflow");
            table.insert(Key::Index(index), Some(Value::from(instruction)));
        }
        if let Some(name) = this.name {
            table.insert_name("name", Some(Value::String(name)));
        }
        if let Some(description) = this.description {
            table.insert_name("desc", Some(Value::String(description)));
        }
        if !this.parameters.is_empty() {
            table.insert_name("parameters", Some(
                this.parameters.iter()
                    .map(|param| Some(Value::Boolean(param.is_input)))
                    .collect::<Value>()
            ));
            table.insert_name("pnames", Some(
                this.parameters.into_iter()
                    .map(|param| param.name.map(Value::String))
                    .collect::<Value>()
            ));
        }
        if !this.subroutines.is_empty() {
            table.insert_name("subs", Some(
                this.subroutines.into_iter()
                    .map(|sub| Some(Value::from(sub)))
                    .collect::<Value>()
            ));
        }
        Value::Table(table.finish())
    }
}

// the worst known case is `"switch"` operation
const INSTRUCTION_MAX_ARGS: usize = 16;

#[derive(Debug, Clone)]
pub struct Instruction {
    pub operation: String,
    pub args: Vec<Option<Operand>>,
    pub next: Option<InstructionIndex>,
    pub comment: Option<String>,
    pub repr_offset: Option<(f64, f64)>,

    // uncommon parameters (`c`, `txt`, `sub`)
    pub variant: Option<i32>,
    pub text: Option<String>,
    pub subroutine: Option<i32>,
}

impl TryFrom<Value> for Instruction {
    type Error = Error;
    fn try_from(value: Value) -> Result<Instruction, Self::Error> {
        let Value::Table(table) = value else {
            return Err(Error::from(
                "instruction should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<Table> for Instruction {
    type Error = Error;
    fn try_from(table: Table) -> Result<Instruction, Self::Error> {
        InstructionBuilder::build_from(table)
    }
}

#[derive(Default)]
struct InstructionBuilder {
    operation: Option<String>,
    args: Vec<Option<Operand>>,
    next: Option<InstructionIndex>,
    comment: Option<String>,
    repr_offset_x: Option<f64>,
    repr_offset_y: Option<f64>,

    // uncommon parameters (`c`, `txt`, `sub`)
    variant: Option<i32>,
    text: Option<String>,
    subroutine: Option<i32>,
}

impl InstructionBuilder {

    fn build_from(table: Table) -> Result<Instruction, Error> {
        const MAX_ARGS: usize = INSTRUCTION_MAX_ARGS;
        let mut this = InstructionBuilder::default();
        let array: Box<[_; MAX_ARGS]>
        = table.try_into_seq_and_named(
            |name, value| match name.as_str() {
                "op"  => this.set_operation (value),
                "next"=> this.set_next      (value),
                "cmt" => this.set_comment   (value),
                "nx"  => this.set_offset_x(value),
                "ny"  => this.set_offset_y(value),
                "c"   => this.set_variant   (value),
                "txt" => this.set_text      (value),
                "sub" => this.set_subroutine(value),
                _ => Err(Self::err_unexpected_key(Key::Name(name))),
            },
            Self::err_from_table_index )?;
        for (index, value) in array.into_iter().enumerate() {
            let Some(value) = value else { continue };
            if this.args.len() <= index {
                this.args.resize_with(index + 1, || None);
            }
            this.args[index] = Some(Operand::try_from(value)?);
        }
        this.finish()
    }

    fn err_from_table_index(error: TableIntoError) -> Error {
        match error {
            TableIntoError::NonContinuous(index) =>
                Self::err_non_continuous(index),
            TableIntoError::UnexpectedKey(key) =>
                Self::err_unexpected_key(key),
        }
    }

    fn err_non_continuous(index: i32) -> Error { Error::from(format!(
        "behavior representation should have \
         instruction indices in a continuous range `1..n`: {index:?}" )) }

    fn err_unexpected_key(key: Key) -> Error { Error::from(format!(
        "behavior representation should not have {key:?} key" )) }

    fn set_operation(&mut self, value: Value) -> Result<(), Error> {
        let Value::String(value) = value else {
            return Err(Error::from(
                "instruction's operation should be a string" ));
        };
        self.operation = Some(value); Ok(())
    }

    fn set_variant(&mut self, value: Value) -> Result<(), Error> {
        let Value::Integer(value) = value else {
            return Err(Error::from(
                "instruction's variant should be an integer" ));
        };
        self.variant = Some(value); Ok(())
    }

    fn set_next(&mut self, value: Value) -> Result<(), Error> {
        self.next = Some(InstructionIndex::try_from(value)?); Ok(())
    }

    fn set_comment(&mut self, value: Value) -> Result<(), Error> {
        let Value::String(value) = value else {
            return Err(Error::from(
                "instruction's comment should be a string" ));
        };
        self.comment = Some(value); Ok(())
    }

    fn set_text(&mut self, value: Value) -> Result<(), Error> {
        let Value::String(value) = value else {
            return Err(Error::from(
                "instruction's text should be a string" ));
        };
        self.text = Some(value); Ok(())
    }

    fn set_subroutine(&mut self, value: Value) -> Result<(), Error> {
        let Value::Integer(value) = value else {
            return Err(Error::from(
                "instruction's subroutine index should be an integer" ));
        };
        self.subroutine = Some(value); Ok(())
    }

    fn set_float(field: &mut Option<f64>, value: Value) -> Result<(), Error> {
        let Value::Float(value) = value else {
            return Err(Error::from(
                "instruction's offset should be a float" ));
        };
        *field = Some(value); Ok(())
    }

    fn set_offset_x(&mut self, value: Value) -> Result<(), Error> {
        Self::set_float(&mut self.repr_offset_x, value)
    }

    fn set_offset_y(&mut self, value: Value) -> Result<(), Error> {
        Self::set_float(&mut self.repr_offset_y, value)
    }

    fn finish(self) -> Result<Instruction, Error> {
        let Self{
            operation, args, next,
            comment,
            repr_offset_x, repr_offset_y,
            variant, text, subroutine,
        } = self;
        let Some(operation) = operation else {
            return Err(Error::from(
                "Operation must be represented with a non-nil `op` field" ));
        };
        let repr_offset = match (repr_offset_x, repr_offset_y) {
            (Some(x), Some(y)) => Some((x, y)),
            (None, None) => None,
            _ => return Err(Error::from(
                "Offset coordinates (`nx` and `ny` fields) should either \
                both be present or both not." )),
        };
        Ok(Instruction {
            operation, args, next,
            comment,
            repr_offset,
            variant, text, subroutine,
        })
    }
}

impl From<Instruction> for Value {
    fn from(this: Instruction) -> Value {
        let mut table = Table::builder();
        for (i, operand) in this.args.into_iter().enumerate() {
            let index: i32 = (i + 1).try_into()
                .expect("arg count should not overflow");
            table.insert(Key::Index(index), operand.map(Value::from));
        }
        table.insert_name("op", Some(Value::String(this.operation)));
        if let Some(next) = this.next {
            table.insert_name("next", Some(Value::from(next)));
        } else {
            table.insert_assoc_dead_name("next");
        }
        if let Some(variant) = this.variant {
            table.insert_name("c", Some(Value::Integer(variant)));
        }
        if let Some(text) = this.text {
            table.insert_name("txt", Some(Value::String(text)));
        }
        if let Some(subroutine) = this.subroutine {
            table.insert_name("sub", Some(Value::Integer(subroutine)));
        }
        if let Some(comment) = this.comment {
            table.insert_name("cmt", Some(Value::String(comment)));
        }
        if let Some((x, y)) = this.repr_offset {
            table.insert_name("nx", Some(Value::Float(x)));
            table.insert_name("ny", Some(Value::Float(y)));
        }
        Value::Table(table.finish())
    }
}

#[derive(Debug, Clone)]
pub enum InstructionIndex {

    Return,

    /// `1`-based index in the instruction list.
    Index(i32),

    // `Option::<InstructionIndex>::None` indicates the next instruction
    // in the instruction list
}

impl TryFrom<Value> for InstructionIndex {
    type Error = Error;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value {
            Value::Boolean(false) => Self::Return,
            Value::Integer(index) if index > 0 => Self::Index(index),
            _ => return Err(Error::from(
                "instruction jump reference should be either `false` or
                 a positive integer" ))
        })
    }
}

impl From<InstructionIndex> for Value {
    fn from(this: InstructionIndex) -> Self {
        match this {
            InstructionIndex::Index(index) => Value::Integer(index),
            InstructionIndex::Return => Value::Boolean(false),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Operand {

    // a next instruction in a branching instruction
    Jump(InstructionIndex),

    Place(Place),

    Value(OpValue),

    // this can indicate either instruction or place index,
    // depending on operation.
    Index(i32),

}

impl TryFrom<Value> for Operand {
    type Error = Error;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value {
            Value::Boolean(false) => Operand::Jump(InstructionIndex::Return),
            Value::Integer(index @ 1 ..= i32::MAX) => Operand::Index(index),
            Value::Integer(index @ -4 ..= -1) =>
                Operand::Place(Place::Register(Register::try_from(index)?)),
            Value::String(name) => Operand::Place(Place::Variable(name)),
            Value::Table(table) => Operand::Value(OpValue::try_from(table)?),
            Value::Float(_) => return Err(Error::from(
                "operand cannot be a float" )),
            Value::Boolean(true) => return Err(Error::from(
                "operand cannot be `true`" )),
            Value::Integer(index @ i32::MIN ..= 0) =>
                return Err(Error::from(
                    "operand cannot be a negative number \
                     except for register codes" )),
        })
    }
}

impl From<Operand> for Value {
    fn from(this: Operand) -> Value {
        match this {
            Operand::Jump(index) => Value::from(index),
            Operand::Place(place) => Value::from(place),
            Operand::Value(value) => Value::from(value),
            Operand::Index(index) => Value::Integer(index),
        }
    }
}

/// Load or store argument
#[derive(Debug, Clone)]
pub enum Place {
    Parameter(i32),
    Register(Register),
    Variable(String),
}

impl TryFrom<Value> for Place {
    type Error = Error;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value {
            Value::Integer(index @ 1 ..= i32::MAX) => Place::Parameter(index),
            Value::Integer(index @ -4 ..= -1) =>
                Place::Register(Register::try_from(index)?),
            Value::String(name) => Place::Variable(name),
            Value::Integer(index @ i32::MIN ..= 0) =>
                return Err(Error::from(
                    "operand cannot be a negative number \
                     except for register codes" )),
            _ => return Err(Error::from(
                "operand should be a string or an integer" )),
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
#[derive(Debug, Clone)]
pub enum Register {
    Signal = -4,
    Visual = -3,
    Store  = -2,
    Goto   = -1,
}

impl TryFrom<Value> for Register {
    type Error = Error;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value {
            Value::Integer(index) => Self::try_from(index)?,
            _ => return Err(Error::from(
                "register should be encoded by an integer" )),
        })
    }
}

impl TryFrom<i32> for Register {
    type Error = Error;
    #[inline]
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(match value {
            -4 => Self::Signal,
            -3 => Self::Visual,
            -2 => Self::Store,
            -1 => Self::Goto,
            _ => return Err(Error::from(
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

/// The kind of values that can be stored in a register
#[derive(Debug, Clone)]
pub enum OpValue {
    Number(i32),
    Item(String),
    ItemCount(String, i32),
    Coord(Coord),
    CoordCount(Coord, i32),
}

impl TryFrom<Value> for OpValue {
    type Error = Error;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Value::Table(table) = value else {
            return Err(Error::from(
                "value operand should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<Table> for OpValue {
    type Error = Error;
    fn try_from(table: Table) -> Result<Self, Self::Error> {
        fn err_from_table_index(error: TableIntoError) -> Error {
            match error {
                TableIntoError::NonContinuous(index) =>
                    err_unexpected_key(Key::Index(index)),
                TableIntoError::UnexpectedKey(key) =>
                    err_unexpected_key(key),
            }
        }
        fn err_unexpected_key(key: Key) -> Error { Error::from(format!(
            "value representation should not have {key:?} key" )) }
        fn id_ok(value: Value) -> Result<String, Error> {
            match value {
                Value::String(id) => Ok(id),
                _ => Err(Error::from("`id` value should be string")),
            }
        }
        fn num_ok(value: Value) -> Result<i32, Error> {
            match value {
                Value::Integer(num) => Ok(num),
                _ => Err(Error::from("`num` value should be integer")),
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
            (None, None, Some(num)) => OpValue::Number(num),
            (Some(id), None, None) => OpValue::Item(id),
            (Some(id), None, Some(num)) => OpValue::ItemCount(id, num),
            (None, Some(coord), None) => OpValue::Coord(coord),
            (None, Some(coord), Some(num)) => OpValue::CoordCount(coord, num),
            (None, None, None) => return Err(Error::from(
                "value representation should have at least one of the fields\
                 `id`, `coord`, `num`" )),
            (Some(_), Some(_), _) => return Err(Error::from(
                "value representation cannot have both\
                 `id` and `coord` fields" )),
        })
    }
}

impl From<OpValue> for Value {
    fn from(this: OpValue) -> Value {
        match this {
            OpValue::Number(number) => {
                let mut table = Table::assoc_builder(Some(0));
                table.insert_name("num", Some(Value::Integer(number)));
                Value::Table(table.finish())
            },
            OpValue::Coord(coord) | OpValue::CoordCount(coord, 0) => {
                let mut table = Table::assoc_builder(Some(0));
                table.insert_name("coord", Some(Value::from(coord)));
                Value::Table(table.finish())
            },
            OpValue::CoordCount(coord, num) => {
                let mut table = Table::assoc_builder(Some(1));
                table.insert_name("coord", Some(Value::from(coord)));
                table.insert_name("num", Some(Value::Integer(num)));
                Value::Table(table.finish())
            },
            OpValue::Item(id) | OpValue::ItemCount(id, 0) => {
                let mut table = Table::assoc_builder(Some(0));
                table.insert_name("id", Some(Value::String(id)));
                Value::Table(table.finish())
            },
            OpValue::ItemCount(id, num) => {
                let mut table = Table::assoc_builder(Some(1));
                table.insert_name("id", Some(Value::String(id)));
                table.insert_name("num", Some(Value::Integer(num)));
                Value::Table(table.finish())
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl TryFrom<Value> for Coord {
    type Error = Error;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Value::Table(table) = value else {
            return Err(Error::from(
                "coord should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<Table> for Coord {
    type Error = Error;
    fn try_from(table: Table) -> Result<Self, Self::Error> {
        fn err_from_table_index(error: TableIntoError) -> Error {
            match error {
                TableIntoError::NonContinuous(index) =>
                    err_unexpected_key(Key::Index(index)),
                TableIntoError::UnexpectedKey(key) =>
                    err_unexpected_key(key),
            }
        }
        fn err_unexpected_key(key: Key) -> Error { Error::from(format!(
            "coord representation should not have {key:?} field" )) }
        fn i32_ok(value: Value) -> Result<i32, Error> {
            match value {
                Value::Integer(z) => Ok(z),
                _ => Err(Error::from(
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
            return Err(Error::from("coord must have `x` and `y` fields"));
        };
        Ok(Coord{x, y})
    }
}

impl From<Coord> for Value {
    fn from(this: Coord) -> Value {
        let mut table = Table::assoc_builder(Some(1));
        table.insert(Key::from("x"), Some(Value::Integer(this.x)));
        table.insert(Key::from("y"), Some(Value::Integer(this.y)));
        Value::Table(table.finish())
    }
}

