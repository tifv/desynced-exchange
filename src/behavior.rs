#![allow(clippy::use_self)]

use serde::{Deserialize, Serialize};

use crate::{
    load::error::Error as LoadError,
    value::{Key, Value, Table, TableIntoError},
    table::ilog2_ceil,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Behavior {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parameters: Vec<Parameter>,
    pub instructions: Vec<Instruction>,
    pub subroutines: Vec<Behavior>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Parameter {
    pub name: Option<String>,
    pub is_input: bool,
}

impl TryFrom<Value> for Behavior {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Behavior, Self::Error> {
        let Value::Table(table) = value else {
            return Err(LoadError::from(
                "behavior should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<Table> for Behavior {
    type Error = LoadError;
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

    fn build_from(table: Table) -> Result<Behavior, LoadError> {
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

    fn err_from_table_index(error: TableIntoError) -> LoadError {
        match error {
            TableIntoError::NonContinuous(index) =>
                Self::err_non_continuous(index),
            TableIntoError::UnexpectedKey(key) =>
                Self::err_unexpected_key(key),
        }
    }

    fn err_non_continuous(index: i32) -> LoadError { LoadError::from(format!(
        "behavior representation should have \
         instruction indices in a continuous range `1..n`: {index:?}" )) }

    fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
        "behavior representation should not have {key:?} key" )) }

    fn set_name(&mut self, name: Value) -> Result<(), LoadError> {
        let Value::String(name) = name else {
            return Err(LoadError::from(
                "behavor's name should be a string" ));
        };
        self.name = Some(name); Ok(())
    }

    fn set_description(&mut self, description: Value) -> Result<(), LoadError> {
        let Value::String(description) = description else {
            return Err(LoadError::from(
                "behavor's description should be a string" ));
        };
        self.description = Some(description); Ok(())
    }

    fn set_parameters(&mut self, parameters: Value) -> Result<(), LoadError> {
        let Value::Table(parameters) = parameters else {
            return Err(Self::err_parameters());
        };
        let parameters = Vec::try_from(parameters)
            .map_err(|_error| Self::err_parameters())?;
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
    fn err_parameters() -> LoadError { LoadError::from(
        "behavior's parameters should be \
         a continuous array of booleans" ) }

    fn set_parameter_names(&mut self, parameter_names: Value)
    -> Result<(), LoadError> {
        let Value::Table(parameter_names) = parameter_names else {
            return Err(Self::err_param_names());
        };
        self.parameter_names = Some(parameter_names);
        Ok(())
    }
    fn reconcile_parameter_names(
        parameters: &mut [Parameter],
        parameter_names: Table,
    ) -> Result<(), LoadError> {
        for (index, value) in parameter_names {
            let Some(index) = index.as_index()
                .map(usize::try_from).and_then(Result::ok)
                .filter(|&x| x > 0) 
            else {
                return Err(Self::err_param_names());
            };
            if index >= parameters.len() {
                return Err(LoadError::from(
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
    fn err_param_names() -> LoadError { LoadError::from(
        "behavior's parameter names should be \
         an array of strings or nils" ) }

    fn set_subroutines(&mut self, subroutines: Value)
    -> Result<(), LoadError> {
        let Value::Table(subroutines) = subroutines else {
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

impl From<Behavior> for Value {
    fn from(this: Behavior) -> Value {
        let mut table = Table::dump_builder(
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
            .map(Value::from).map(Some) );
        if let Some(name) = this.name {
            table.assoc_insert_name("name", Some(Value::String(name)));
        }
        if let Some(description) = this.description {
            table.assoc_insert_name("desc", Some(Value::String(description)));
        }
        if !this.parameters.is_empty() {
            table.assoc_insert_name("parameters", Some(
                this.parameters.iter()
                    .map(|param| Some(Value::Boolean(param.is_input)))
                    .collect::<Value>()
            ));
            table.assoc_insert_name("pnames", Some(
                this.parameters.into_iter()
                    .map(|param| param.name.map(Value::String))
                    .collect::<Value>()
            ));
        }
        if !this.subroutines.is_empty() {
            table.assoc_insert_name("subs", Some(
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Instruction, Self::Error> {
        let Value::Table(table) = value else {
            return Err(LoadError::from(
                "instruction should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<Table> for Instruction {
    type Error = LoadError;
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

    fn build_from(table: Table) -> Result<Instruction, LoadError> {
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

    fn err_from_table_index(error: TableIntoError) -> LoadError {
        match error {
            TableIntoError::NonContinuous(index) =>
                Self::err_non_continuous(index),
            TableIntoError::UnexpectedKey(key) =>
                Self::err_unexpected_key(key),
        }
    }

    fn err_non_continuous(index: i32) -> LoadError { LoadError::from(format!(
        "behavior representation should have \
         instruction indices in a continuous range `1..n`: {index:?}" )) }

    fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
        "behavior representation should not have {key:?} key" )) }

    fn set_operation(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::String(value) = value else {
            return Err(LoadError::from(
                "instruction's operation should be a string" ));
        };
        self.operation = Some(value); Ok(())
    }

    fn set_variant(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::Integer(value) = value else {
            return Err(LoadError::from(
                "instruction's variant should be an integer" ));
        };
        self.variant = Some(value); Ok(())
    }

    fn set_next(&mut self, value: Value) -> Result<(), LoadError> {
        self.next = Some(InstructionIndex::try_from(value)?); Ok(())
    }

    fn set_comment(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::String(value) = value else {
            return Err(LoadError::from(
                "instruction's comment should be a string" ));
        };
        self.comment = Some(value); Ok(())
    }

    fn set_text(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::String(value) = value else {
            return Err(LoadError::from(
                "instruction's text should be a string" ));
        };
        self.text = Some(value); Ok(())
    }

    fn set_subroutine(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::Integer(value) = value else {
            return Err(LoadError::from(
                "instruction's subroutine index should be an integer" ));
        };
        self.subroutine = Some(value); Ok(())
    }

    fn set_float(field: &mut Option<f64>, value: Value) -> Result<(), LoadError> {
        let Value::Float(value) = value else {
            return Err(LoadError::from(
                "instruction's offset should be a float" ));
        };
        *field = Some(value); Ok(())
    }

    fn set_offset_x(&mut self, value: Value) -> Result<(), LoadError> {
        Self::set_float(&mut self.repr_offset_x, value)
    }

    fn set_offset_y(&mut self, value: Value) -> Result<(), LoadError> {
        Self::set_float(&mut self.repr_offset_y, value)
    }

    fn finish(self) -> Result<Instruction, LoadError> {
        let Self{
            operation, args, next,
            comment,
            repr_offset_x, repr_offset_y,
            variant, text, subroutine,
        } = self;
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

impl From<Instruction> for Value {
    fn from(this: Instruction) -> Value {
        let mut table = Table::dump_builder(
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
            .map(|arg| arg.map(Value::from)) );
        table.assoc_insert_name("op", Some(Value::String(this.operation)));
        if let Some(next) = this.next {
            table.assoc_insert_name("next", Some(Value::from(next)));
        } else {
            table.assoc_insert_dead_name("next");
        }
        if let Some(comment) = this.comment {
            table.assoc_insert_name("cmt", Some(Value::String(comment)));
        }
        if let Some((x, y)) = this.repr_offset {
            table.assoc_insert_name("nx", Some(Value::Float(x)));
            table.assoc_insert_name("ny", Some(Value::Float(y)));
        }
        if let Some(variant) = this.variant {
            table.assoc_insert_name("c", Some(Value::Integer(variant)));
        }
        if let Some(text) = this.text {
            table.assoc_insert_name("txt", Some(Value::String(text)));
        }
        if let Some(subroutine) = this.subroutine {
            table.assoc_insert_name("sub", Some(Value::Integer(subroutine)));
        }
        Value::Table(table.finish())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum InstructionIndex {

    Return,

    /// `1`-based index in the instruction list.
    Index(i32),

    // `Option::<InstructionIndex>::None` indicates the next instruction
    // in the instruction list
}

impl TryFrom<Value> for InstructionIndex {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value {
            Value::Boolean(false) => Self::Return,
            Value::Integer(index) if index > 0 => Self::Index(index),
            _ => return Err(LoadError::from(
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value {
            Value::Boolean(false) => Operand::Jump(InstructionIndex::Return),
            Value::Integer(index @ 1 ..= i32::MAX) => Operand::Index(index),
            Value::Integer(index @ -4 ..= -1) =>
                Operand::Place(Place::Register(Register::try_from(index)?)),
            Value::String(name) => Operand::Place(Place::Variable(name)),
            Value::Table(table) => Operand::Value(OpValue::try_from(table)?),
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

/// Place arguments to instructions
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Place {
    Parameter(i32),
    Register(Register),
    Variable(String),
}

impl TryFrom<Value> for Place {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value {
            Value::Integer(index @ 1 ..= i32::MAX) => Place::Parameter(index),
            Value::Integer(index @ -4 ..= -1) =>
                Place::Register(Register::try_from(index)?),
            Value::String(name) => Place::Variable(name),
            Value::Integer(i32::MIN ..= 0) =>
                return Err(LoadError::from(
                    "operand cannot be a negative number \
                     except for register codes" )),
            _ => return Err(LoadError::from(
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Register {
    Signal = -4,
    Visual = -3,
    Store  = -2,
    Goto   = -1,
}

impl TryFrom<Value> for Register {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value {
            Value::Integer(index) => Self::try_from(index)?,
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

impl From<Register> for Value {
    fn from(this: Register) -> Value {
        Value::Integer(this as i32)
    }
}

/// Value arguments to operations
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum OpValue {
    Number(i32),
    Item(String),
    ItemCount(String, i32),
    Coord(Coord),
    CoordCount(Coord, i32),
}

impl TryFrom<Value> for OpValue {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Value::Table(table) = value else {
            return Err(LoadError::from(
                "value operand should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<Table> for OpValue {
    type Error = LoadError;
    fn try_from(table: Table) -> Result<Self, Self::Error> {
        fn err_from_table_index(error: TableIntoError) -> LoadError {
            match error {
                TableIntoError::NonContinuous(index) =>
                    err_unexpected_key(Key::Index(index)),
                TableIntoError::UnexpectedKey(key) =>
                    err_unexpected_key(key),
            }
        }
        fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
            "value representation should not have {key:?} key" )) }
        fn id_ok(value: Value) -> Result<String, LoadError> {
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
                let mut table = Table::dump_builder(Some(0), Some(0));
                table.assoc_insert_name("num", Some(Value::Integer(number)));
                Value::Table(table.finish())
            },
            OpValue::Coord(coord) | OpValue::CoordCount(coord, 0) => {
                let mut table = Table::dump_builder(Some(0), Some(0));
                table.assoc_insert_name("coord", Some(Value::from(coord)));
                Value::Table(table.finish())
            },
            OpValue::CoordCount(coord, num) => {
                let mut table = Table::dump_builder(Some(0), Some(1));
                table.assoc_insert_name("coord", Some(Value::from(coord)));
                table.assoc_insert_name("num", Some(Value::Integer(num)));
                Value::Table(table.finish())
            },
            OpValue::Item(id) | OpValue::ItemCount(id, 0) => {
                let mut table = Table::dump_builder(Some(0), Some(0));
                table.assoc_insert_name("id", Some(Value::String(id)));
                Value::Table(table.finish())
            },
            OpValue::ItemCount(id, num) => {
                let mut table = Table::dump_builder(Some(0), Some(1));
                table.assoc_insert_name("id", Some(Value::String(id)));
                table.assoc_insert_name("num", Some(Value::Integer(num)));
                Value::Table(table.finish())
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl TryFrom<Value> for Coord {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Value::Table(table) = value else {
            return Err(LoadError::from(
                "coord should be represented by a table value" ));
        };
        Self::try_from(table)
    }
}

impl TryFrom<Table> for Coord {
    type Error = LoadError;
    fn try_from(table: Table) -> Result<Self, Self::Error> {
        fn err_from_table_index(error: TableIntoError) -> LoadError {
            match error {
                TableIntoError::NonContinuous(index) =>
                    err_unexpected_key(Key::Index(index)),
                TableIntoError::UnexpectedKey(key) =>
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

impl From<Coord> for Value {
    fn from(this: Coord) -> Value {
        let mut table = Table::dump_builder(Some(0), Some(1));
        table.assoc_insert(Key::from("x"), Some(Value::Integer(this.x)));
        table.assoc_insert(Key::from("y"), Some(Value::Integer(this.y)));
        Value::Table(table.finish())
    }
}

