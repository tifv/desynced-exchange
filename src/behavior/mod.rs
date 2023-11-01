use crate::value::{Key, Value, Table};

#[derive(Debug, Clone)]
pub struct Behavior {
    name: Option<String>,
    description: Option<String>,
    parameters: Vec<Parameter>,
    instructions: Vec<Instruction>,
    subroutines: Vec<Behavior>,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    name: Option<String>,
    editable: bool,
}

#[derive(Debug, Clone)]
pub struct Instruction {
    operation: String,
    variant: Option<i32>,
    args: Vec<Option<Operand>>,
    next: Option<i32>,
    comment: Option<String>,
    text: Option<String>,
    subroutine: Option<i32>,
    repr_offset: Option<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub enum Operand {
    // the next target in a branching instruction
    Instruction(i32),

    Place(Place),

    Value(OpValue),

    // this can indicate either instruction or place index,
    // depending on operation.
    SomeIndex(i32),
}

/// Load or store argument
#[derive(Debug, Clone)]
pub enum Place {
    Parameter(i32),
    Register(Register),
    Variable(String),
}

#[repr(i32)]
#[derive(Debug, Clone)]
pub enum Register {
    Signal = -4,
    Visual = -3,
    Store  = -2,
    Goto   = -1,
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

#[derive(Debug, Clone)]
pub struct Coord {
    x: i32,
    y: i32,
}

impl From<Behavior> for Value {
    fn from(value: Behavior) -> Self {
        todo!()
    }
}

impl From<Instruction> for Value {
    fn from(this: Instruction) -> Self {
        #![allow(clippy::use_self)]
        let mut table = Table::builder();
        for (i, operand) in this.args.into_iter().enumerate() {
            let index: i32 = (i + 1).try_into()
                .expect("arg count should not overflow");
            table.insert(Key::Index(index), operand.map(Value::from));
        }
        table.insert_name("op", Some(Value::String(this.operation)));
        if let Some(next) = this.next {
            table.insert_name("next", Some(Value::Integer(next)));
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
        Self::Table(table.finish())
    }
}

impl From<Operand> for Value {
    fn from(value: Operand) -> Self {
        match value {
            Operand::Instruction(index) | Operand::SomeIndex(index)
                => Self::Integer(index),
            Operand::Place(place) => Self::from(place),
            Operand::Value(value) => Self::from(value),
        }
    }
}

impl From<Place> for Value {
    fn from(value: Place) -> Self {
        match value {
            Place::Parameter(index) => Self::Integer(index),
            Place::Register(register) => Self::from(register),
            Place::Variable(name) => Self::String(name),
        }
    }
}

impl From<Register> for Value {
    fn from(value: Register) -> Self {
        Self::Integer(value as i32)
    }
}

impl From<OpValue> for Value {
    fn from(value: OpValue) -> Self {
        #![allow(clippy::use_self)]
        match value {
            OpValue::Number(number) => {
                let mut table = Table::assoc_builder(Some(0));
                table.insert_name("num", Some(Value::Integer(number)));
                Self::Table(table.finish())
            },
            OpValue::Coord(coord) | OpValue::CoordCount(coord, 0) => {
                let mut table = Table::assoc_builder(Some(0));
                table.insert_name("coord", Some(Value::from(coord)));
                Self::Table(table.finish())
            },
            OpValue::CoordCount(coord, num) => {
                let mut table = Table::assoc_builder(Some(1));
                table.insert_name("coord", Some(Value::from(coord)));
                table.insert_name("num", Some(Value::Integer(num)));
                Self::Table(table.finish())
            },
            OpValue::Item(id) | OpValue::ItemCount(id, 0) => {
                let mut table = Table::assoc_builder(Some(0));
                table.insert_name("id", Some(Value::String(id)));
                Self::Table(table.finish())
            },
            OpValue::ItemCount(id, num) => {
                let mut table = Table::assoc_builder(Some(1));
                table.insert_name("id", Some(Value::String(id)));
                table.insert_name("num", Some(Value::Integer(num)));
                Self::Table(table.finish())
            },
        }
    }
}

impl From<Coord> for Value {
    fn from(value: Coord) -> Self {
        #![allow(clippy::use_self)]
        let mut table = Table::assoc_builder(Some(1));
        table.insert(Key::from("x"), Some(Value::Integer(value.x)));
        table.insert(Key::from("y"), Some(Value::Integer(value.y)));
        Self::Table(table.finish())
    }
}

