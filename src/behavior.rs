#![allow(clippy::use_self)]

use serde::{Deserialize, Serialize};

use crate::{
    load::error::Error as LoadError,
    value::{self as v, Key, TableIntoError as TableError, LimitedVec},
    table::ilog2_ceil,
};

mod serde_option_some {
    use serde::{
        Serialize, Serializer,
        Deserialize, Deserializer,
    };

    pub(super) fn serialize<T, S>(value: &Option<T>, ser: S)
    -> Result<S::Ok, S::Error>
    where T: Serialize, S: Serializer {
        value.as_ref().unwrap().serialize(ser)
    }
    pub(super) fn deserialize<'de, T, D>(de: D)
    -> Result<Option<T>, D::Error>
    where  T: Deserialize<'de>, D: Deserializer<'de> {
        Ok(Some(T::deserialize(de)?))
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Behavior {
    #[serde( default,
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub name: Option<String>,
    #[serde( default,
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if="Vec::is_empty")]
    pub parameters: Vec<Parameter>,
    pub instructions: Vec<Instruction>,
    #[serde(default, skip_serializing_if="Vec::is_empty")]
    pub subroutines: Vec<Behavior>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Parameter {
    #[serde( default,
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub name: Option<String>,
    pub is_output: bool,
}

impl TryFrom<v::Value> for Behavior {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Behavior, Self::Error> {
        let v::Value::Table(table) = value else {
            return Err(LoadError::from(
                "behavior should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<v::Table> for Behavior {
    type Error = LoadError;
    fn try_from(table: v::Table) -> Result<Behavior, Self::Error> {
        BehaviorBuilder::build_from(table)
    }
}

#[derive(Default)]
struct BehaviorBuilder {
    name: Option<String>,
    description: Option<String>,
    parameters: Vec<Parameter>,
    parameter_names: Option<v::Table>,
    instructions: Vec<Instruction>,
    subroutines: Vec<Behavior>,
}

impl BehaviorBuilder {

    fn build_from(table: v::Table) -> Result<Behavior, LoadError> {
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

    fn err_from_table_index(error: TableError) -> LoadError {
        match error {
            TableError::NonContinuous(index) =>
                Self::err_non_continuous(index),
            TableError::UnexpectedKey(key) =>
                Self::err_unexpected_key(key),
        }
    }

    fn err_non_continuous(index: i32) -> LoadError { LoadError::from(format!(
        "behavior representation should have \
         instruction indices in a continuous range `1..n`: {index:?}" )) }

    fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
        "behavior representation should not have {key:?} key" )) }

    fn set_name(&mut self, name: v::Value) -> Result<(), LoadError> {
        let v::Value::String(name) = name else {
            return Err(LoadError::from(
                "behavor's name should be a string" ));
        };
        self.name = Some(name); Ok(())
    }

    fn set_description(&mut self, description: v::Value) -> Result<(), LoadError> {
        let v::Value::String(description) = description else {
            return Err(LoadError::from(
                "behavor's description should be a string" ));
        };
        self.description = Some(description); Ok(())
    }

    fn set_parameters(&mut self, parameters: v::Value) -> Result<(), LoadError> {
        let v::Value::Table(parameters) = parameters else {
            return Err(Self::err_parameters());
        };
        let parameters = Vec::try_from(parameters)
            .map_err(|_error| Self::err_parameters())?;
        let this = &mut self.parameters;
        *this = Vec::with_capacity(parameters.len());
        for value in parameters {
            let v::Value::Boolean(value) = value else {
                return Err(Self::err_parameters());
            };
            this.push(Parameter{is_output: value, name: None});
        }
        Ok(())
    }
    fn err_parameters() -> LoadError { LoadError::from(
        "behavior's parameters should be \
         a continuous array of booleans" ) }

    fn set_parameter_names(&mut self, parameter_names: v::Value)
    -> Result<(), LoadError> {
        let v::Value::Table(parameter_names) = parameter_names else {
            return Err(Self::err_param_names());
        };
        self.parameter_names = Some(parameter_names);
        Ok(())
    }
    fn reconcile_parameter_names(
        parameters: &mut [Parameter],
        parameter_names: v::Table,
    ) -> Result<(), LoadError> {
        for (index, value) in parameter_names {
            let Some(index) = index.as_index()
                .map(usize::try_from).and_then(Result::ok)
                .and_then(|x| x.checked_sub(1_usize))
            else {
                return Err(Self::err_param_names());
            };
            if index >= parameters.len() {
                return Err(LoadError::from(
                    "the number of behavior's parameters is inconsistent with \
                    the number of parameter names" ));
            }
            let v::Value::String(value) = value else {
                return Err(Self::err_param_names());
            };
            parameters[index].name = Some(value);
        }
        Ok(())
    }
    fn err_param_names() -> LoadError { LoadError::from(
        "behavior's parameter names should be \
         an array of strings or nils" ) }

    fn set_subroutines(&mut self, subroutines: v::Value)
    -> Result<(), LoadError> {
        let v::Value::Table(subroutines) = subroutines else {
            return Err(Self::err_subroutines());
        };
        let subroutines = Vec::try_from(subroutines)
            .map_err(|_error| Self::err_subroutines())?;
        let this = &mut self.subroutines;
        *this = Vec::with_capacity(subroutines.len());
        for value in subroutines {
            this.push(Behavior::try_from(value)?);
        }
        Ok(())
    }
    fn err_subroutines() -> LoadError { LoadError::from(
        "behavior's subroutines should be \
         a continuous array" ) }

    fn finish(self) -> Result<Behavior, LoadError> {
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

impl From<Behavior> for v::Value {
    fn from(this: Behavior) -> v::Value {
        let mut table = v::Table::dump_builder(
            Some( this.instructions.len().try_into()
                .expect("length should fit") ),
            ilog2_ceil(
                usize::from(this.name.is_some()) +
                usize::from(this.description.is_some()) +
                2 * usize::from(!this.parameters.is_empty()) +
                usize::from(!this.subroutines.is_empty())
            ),
        );
        table.extend( this.instructions.into_iter()
            .map(v::Value::from).map(Some) );
        if let Some(name) = this.name {
            table.assoc_insert_name("name", Some(v::Value::String(name)));
        }
        if let Some(description) = this.description {
            table.assoc_insert_name( "desc",
                Some(v::Value::String(description)) );
        }
        if !this.parameters.is_empty() {
            table.assoc_insert_name("parameters", Some(
                this.parameters.iter()
                    .map(|param| Some(v::Value::Boolean(param.is_output)))
                    .collect::<v::Value>()
            ));
            table.assoc_insert_name("pnames", Some(
                this.parameters.into_iter()
                    .map(|param| param.name.map(v::Value::String))
                    .collect::<v::Value>()
            ));
        }
        if !this.subroutines.is_empty() {
            table.assoc_insert_name("subs", Some(
                this.subroutines.into_iter()
                    .map(|sub| Some(v::Value::from(sub)))
                    .collect::<v::Value>()
            ));
        }
        v::Value::Table(table.finish())
    }
}

// subroutines can create instructions with an arbitrary number of parameters
const INSTRUCTION_MAX_ARGS: usize = 256;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Instruction {

    #[serde(rename="op")]
    pub operation: String,

    pub args: Vec<Operand>,

    pub next: Jump,

    #[serde( default, rename="cmt",
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub comment: Option<String>,

    #[serde( default, rename="offset",
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub repr_offset: Option<(f64, f64)>,

    // uncommon parameters

    #[serde( default, rename="c",
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub variant: Option<i32>,

    #[serde( default, rename="txt",
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub text: Option<String>,

    #[serde( default, rename="sub",
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub subroutine: Option<i32>,

}

impl TryFrom<v::Value> for Instruction {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Instruction, Self::Error> {
        let v::Value::Table(table) = value else {
            return Err(LoadError::from(
                "instruction should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<v::Table> for Instruction {
    type Error = LoadError;
    fn try_from(table: v::Table) -> Result<Instruction, Self::Error> {
        InstructionBuilder::build_from(table)
    }
}

#[derive(Default)]
struct InstructionBuilder {
    operation: Option<String>,
    args: Vec<Option<Operand>>,
    next: Option<Jump>,
    comment: Option<String>,
    repr_offset_x: Option<f64>,
    repr_offset_y: Option<f64>,

    // uncommon parameters (`c`, `txt`, `sub`)
    variant: Option<i32>,
    text: Option<String>,
    subroutine: Option<i32>,
}

impl InstructionBuilder {

    fn build_from(table: v::Table) -> Result<Instruction, LoadError> {
        const MAX_ARGS: usize = INSTRUCTION_MAX_ARGS;
        let mut this = InstructionBuilder::default();
        let array: LimitedVec<MAX_ARGS, _> = table.try_into_seq_and_named(
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
        let array = array.get();
        this.args.resize_with(array.len(), || None);
        for (index, value) in array.into_iter().enumerate() {
            let Some(value) = value else { continue };
            this.args[index] = Some(Operand::try_from(value)?);
        }
        this.finish()
    }

    fn err_from_table_index(error: TableError) -> LoadError {
        match error {
            TableError::NonContinuous(index) =>
                Self::err_non_continuous(index),
            TableError::UnexpectedKey(key) =>
                Self::err_unexpected_key(key),
        }
    }

    fn err_non_continuous(index: i32) -> LoadError { LoadError::from(format!(
        "behavior representation should have \
         instruction indices in a continuous range `1..n`: {index:?}" )) }

    fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
        "behavior representation should not have {key:?} key" )) }

    fn set_operation(&mut self, value: v::Value) -> Result<(), LoadError> {
        let v::Value::String(value) = value else {
            return Err(LoadError::from(
                "instruction's operation should be a string" ));
        };
        self.operation = Some(value); Ok(())
    }

    fn set_variant(&mut self, value: v::Value) -> Result<(), LoadError> {
        let v::Value::Integer(value) = value else {
            return Err(LoadError::from(
                "instruction's variant should be an integer" ));
        };
        self.variant = Some(value); Ok(())
    }

    fn set_next(&mut self, value: v::Value) -> Result<(), LoadError> {
        self.next = Some(Jump::try_from(Some(value))?); Ok(())
    }

    fn set_comment(&mut self, value: v::Value) -> Result<(), LoadError> {
        let v::Value::String(value) = value else {
            return Err(LoadError::from(
                "instruction's comment should be a string" ));
        };
        self.comment = Some(value); Ok(())
    }

    fn set_text(&mut self, value: v::Value) -> Result<(), LoadError> {
        let v::Value::String(value) = value else {
            return Err(LoadError::from(
                "instruction's text should be a string" ));
        };
        self.text = Some(value); Ok(())
    }

    fn set_subroutine(&mut self, value: v::Value) -> Result<(), LoadError> {
        let v::Value::Integer(value) = value else {
            return Err(LoadError::from(
                "instruction's subroutine index should be an integer" ));
        };
        self.subroutine = Some(value); Ok(())
    }

    fn set_float(field: &mut Option<f64>, value: v::Value)
    -> Result<(), LoadError> {
        let v::Value::Float(value) = value else {
            return Err(LoadError::from(
                "instruction's offset should be a float" ));
        };
        *field = Some(value); Ok(())
    }

    fn set_offset_x(&mut self, value: v::Value) -> Result<(), LoadError> {
        Self::set_float(&mut self.repr_offset_x, value)
    }

    fn set_offset_y(&mut self, value: v::Value) -> Result<(), LoadError> {
        Self::set_float(&mut self.repr_offset_y, value)
    }

    fn finish(self) -> Result<Instruction, LoadError> {
        let Self{
            operation, args, next,
            comment,
            repr_offset_x, repr_offset_y,
            variant, text, subroutine,
        } = self;
        let args = args.into_iter()
            .map(Operand::unwrap_option)
            .collect();
        let next = Jump::unwrap_option(next);
        let Some(operation) = operation else {
            return Err(LoadError::from(
                "Operation must be represented with a non-nil `op` field" ));
        };
        let repr_offset = match (repr_offset_x, repr_offset_y) {
            (Some(x), Some(y)) => Some((x, y)),
            (None, None) => None,
            _ => return Err(LoadError::from(
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

impl From<Instruction> for v::Value {
    fn from(this: Instruction) -> v::Value {
        let mut table = v::Table::dump_builder(
            Some( this.args.len().try_into()
                .expect("length should fit") ),
            ilog2_ceil(
                1 + // operation
                1 + // next
                usize::from(this.comment.is_some()) +
                2 * usize::from(this.repr_offset.is_some()) +
                usize::from(this.variant.is_some()) +
                usize::from(this.text.is_some()) +
                usize::from(this.subroutine.is_some())
            ),
        );
        table.extend( this.args.into_iter()
            // .map(|arg| arg.map(v::Value::from)) );
            .map(Option::<v::Value>::from) );
        table.assoc_insert_name("op", Some(v::Value::String(this.operation)));
        if let Some(next_value) = this.next.into() {
            table.assoc_insert_name("next", Some(next_value));
        } else {
            table.assoc_insert_dead_name("next");
        }
        if let Some(comment) = this.comment {
            table.assoc_insert_name("cmt", Some(v::Value::String(comment)));
        }
        if let Some((x, y)) = this.repr_offset {
            table.assoc_insert_name("nx", Some(v::Value::Float(x)));
            table.assoc_insert_name("ny", Some(v::Value::Float(y)));
        }
        if let Some(variant) = this.variant {
            table.assoc_insert_name("c", Some(v::Value::Integer(variant)));
        }
        if let Some(text) = this.text {
            table.assoc_insert_name("txt", Some(v::Value::String(text)));
        }
        if let Some(subroutine) = this.subroutine {
            table.assoc_insert_name( "sub",
                Some(v::Value::Integer(subroutine)) );
        }
        v::Value::Table(table.finish())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

    #[serde(untagged, serialize_with="serde_option_place::serialize")]
    Place(Option<Place>),

    #[serde(untagged, serialize_with="Value::serialize_option")]
    Value(Option<Value>),

}

impl Operand {
    fn unwrap_option(this: Option<Operand>) -> Operand {
        if let Some(this) = this { return this; }
        Operand::UnknownUnset
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

impl TryFrom<Option<v::Value>> for Operand {
    type Error = LoadError;
    fn try_from(value: Option<v::Value>) -> Result<Self, Self::Error> {
        Ok(Self::unwrap_option(
            value.map(Self::try_from).transpose()?
        ))
    }
}

impl TryFrom<v::Value> for Operand {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Self, Self::Error> {
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Jump {
    Return,
    Next,
    /// `1`-based index in the instruction list.
    Jump(i32),
}

impl Jump {
    fn unwrap_option(this: Option<Jump>) -> Jump {
        if let Some(this) = this { return this; }
        Jump::Next
    }
}

impl TryFrom<Option<v::Value>> for Jump {
    type Error = LoadError;
    fn try_from(value: Option<v::Value>) -> Result<Self, Self::Error> {
        Ok(Jump::unwrap_option(
            value.map(Self::try_from).transpose()? ))
    }
}

impl TryFrom<v::Value> for Jump {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Self, Self::Error> {
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
#[derive(Debug, Clone, Deserialize, Serialize)]
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
    fn try_from(value: v::Value) -> Result<Self, Self::Error> {
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
    fn try_from(value: i32) -> Result<Self, Self::Error> {
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Register {
    Signal = -4,
    Visual = -3,
    Store  = -2,
    Goto   = -1,
}

impl TryFrom<v::Value> for Register {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Self, Self::Error> {
        Ok(match value {
            v::Value::Integer(index) => Self::try_from(index)?,
            _ => return Err(LoadError::from(
                "register should be encoded by an integer" )),
        })
    }
}

impl TryFrom<i32> for Register {
    type Error = LoadError;
    #[inline]
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(match value {
            -4 => Self::Signal,
            -3 => Self::Visual,
            -2 => Self::Store,
            -1 => Self::Goto,
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
#[derive(Debug, Clone, Deserialize, Serialize)]
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
    fn try_from(value: v::Value) -> Result<Self, Self::Error> {
        let v::Value::Table(table) = value else {
            return Err(LoadError::from(
                "value operand should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<v::Table> for Value {
    type Error = LoadError;
    fn try_from(table: v::Table) -> Result<Self, Self::Error> {
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl TryFrom<v::Value> for Coord {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Self, Self::Error> {
        let v::Value::Table(table) = value else {
            return Err(LoadError::from(
                "coord should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<v::Table> for Coord {
    type Error = LoadError;
    fn try_from(table: v::Table) -> Result<Self, Self::Error> {
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
        Ok(Coord{x, y})
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

