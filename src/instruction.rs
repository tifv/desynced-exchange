#![allow(clippy::use_self)]

use serde::{
    ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer
};

use crate::{
    load::error::Error as LoadError,
    value::{self as v, Key, TableIntoError as TableError, LimitedVec},
    table::ilog2_ceil,
    serde::{
        DeIdentifier,
        ExtraFields, FieldNames,
    },
    operand::{Operand, Jump},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExtraValue {
    Boolean(bool),
    Integer(i32),
    Float(f64),
    String(String),
}

impl TryFrom<v::Value> for ExtraValue {
    type Error = LoadError;

    fn try_from(value: v::Value) -> Result<ExtraValue, Self::Error> {
        Ok(match value {
            v::Value::Boolean(value) => ExtraValue::Boolean(value),
            v::Value::Integer(value) => ExtraValue::Integer(value),
            v::Value::Float  (value) => ExtraValue::Float  (value),
            v::Value::String (value) => ExtraValue::String (value),
            v::Value::Table(_) => return Err(LoadError::from(
                "an extra value should not be a table" )),
        })
    }
}

impl From<ExtraValue> for v::Value {
    fn from(value: ExtraValue) -> v::Value {
        match value {
            ExtraValue::Boolean(value) => v::Value::Boolean(value),
            ExtraValue::Integer(value) => v::Value::Integer(value),
            ExtraValue::Float  (value) => v::Value::Float  (value),
            ExtraValue::String (value) => v::Value::String (value),
        }
    }
}

define_field_names!(
    pub ExtraInstructionNames,
    [
        "c", "txt", "sub",
        "cmt", "nx", "ny",
    ]
);

pub type ExtraInstructionFields =
    ExtraFields<ExtraInstructionNames, ExtraValue>;

// subroutines can create instructions with an arbitrary number of parameters
const INSTRUCTION_MAX_ARGS: usize = 256;

#[derive(Debug, Clone)]
pub struct Instruction {
    pub operation: String,
    pub args: Vec<Operand>,
    pub next: Jump,
    pub extra: ExtraInstructionFields,
}

impl TryFrom<v::Value> for Instruction {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Instruction, Self::Error> {
        let v::Value::Table(table) = value else {
            return Err(LoadError::from(
                "instruction should be represented by a table value" ));
        };
        Instruction::try_from(table)
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
    args: Vec<Operand>,
    next: Option<Jump>,
    extra: ExtraInstructionFields,
}

impl InstructionBuilder {

    fn build_from(table: v::Table) -> Result<Instruction, LoadError> {
        const MAX_ARGS: usize = INSTRUCTION_MAX_ARGS;
        let mut this = Self::default();
        let array: LimitedVec<MAX_ARGS, _> = table.try_into_seq_and_named(
            |name, value| match name.as_str() {
                "op"   => this.set_operation (value),
                "next" => this.set_next      (value),
                "args" => Err(Self::err_unexpected_key(Key::from("args"))),
                key => {
                    if this.extra.insert(key, value.try_into()?).is_some() {
                        unreachable!("duplicate key");
                    } else {
                        Ok(())
                    }
                },
            },
            Self::err_from_table_index )?;
        let array = array.get();
        assert!(this.args.is_empty());
        this.args.reserve_exact(array.len());
        for value in array {
            this.args.push(Operand::try_from(value)?);
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
        "instruction representation should have \
         argument indices in a range `1..`: {index:?}" )) }

    fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
        "instruction representation should not have {key:?} key" )) }

    fn set_operation(&mut self, value: v::Value) -> Result<(), LoadError> {
        let v::Value::String(value) = value else {
            return Err(LoadError::from(
                "instruction's operation should be a string" ));
        };
        self.operation = Some(value); Ok(())
    }

    fn set_next(&mut self, value: v::Value) -> Result<(), LoadError> {
        self.next = Some(Jump::try_from(Some(value))?); Ok(())
    }

    fn finish(self) -> Result<Instruction, LoadError> {
        let Self {
            operation, args, next,
            extra,
        } = self;
        let next = Jump::unwrap_option(next);
        let Some(operation) = operation else {
            return Err(LoadError::from(
                "Operation must be represented with a non-nil `op` field" ));
        };
        Ok(Instruction {
            operation, args, next,
            extra,
        })
    }
}

impl<'de> serde::de::Visitor<'de> for InstructionBuilder {
    type Value = Instruction;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "an instruction")
    }

    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        use serde::de::Error as _;
        while let Some(key) = map.next_key::<DeIdentifier>()? {
            match &*key {
                "op"    => self.operation = Some(map.next_value()?),
                "args"  => self.args      = map.next_value()?,
                "next"  => self.next      = Some(map.next_value()?),
                key => {
                    self.extra.insert(key, map.next_value()?);
                },
            }
        }
        self.finish().map_err(A::Error::custom)
    }

}

impl From<Instruction> for v::Value {
    fn from(this: Instruction) -> v::Value {
        let mut table = v::TableDumpBuilder::new(
            Some( this.args.len().try_into()
                .expect("length should fit") ),
            ilog2_ceil(
                2 // operation, next
                + this.extra.len()
            ),
        );
        table.array_extend( this.args.into_iter()
            .map(Option::<v::Value>::from) );
        table.assoc_insert("op", Some(v::Value::String(this.operation)));
        if let Some(next_value) = this.next.into() {
            table.assoc_insert("next", Some(next_value));
        } else {
            table.assoc_insert_dead("next");
        }
        for (key, value) in this.extra {
            table.assoc_insert(key, Some(v::Value::from(value)));
        }
        v::Value::Table(table.finish())
    }
}

impl Serialize for Instruction {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        let mut ser = ser.serialize_struct(
            "Instruction",
            2 // op, next,
            + usize::from(!self.args.is_empty())
            + self.extra.len(),
        )?;
        ser.serialize_field("op", &self.operation)?;
        if !self.args.is_empty() {
            ser.serialize_field("args", &self.args)?;
        } else {
            ser.skip_field("args")?;
        }
        ser.serialize_field("next", &self.next)?;
        self.extra.serialize_into_struct::<S>(&mut ser)?;
        ser.end()
    }
}

impl<'de> Deserialize<'de> for Instruction {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>
    {
        de.deserialize_struct( "Instruction", &[],
            InstructionBuilder::default() )
    }
}

