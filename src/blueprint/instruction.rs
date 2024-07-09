#![allow(clippy::use_self)]

use std::collections::btree_map::BTreeMap as SortedMap;

use serde::{
    ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer
};

use crate::{
    error::LoadError,
    Str,
    common::serde::Identifier,
    value::{Key, Value, Table, ArrayBuilder as TableArrayBuilder},
};

use super::operand::{Operand, Jump};

#[derive(Debug, Clone)]
pub struct Instruction {
    pub operation: Str,
    pub args: Vec<Operand>,
    pub next: Jump,
    pub extra: SortedMap<Str, Value>,
    pub comment: Option<Str>,
    pub offset: Option<(f64, f64)>,
}

impl TryFrom<Value> for Instruction {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Instruction, Self::Error> {
        let Value::Table(table) = value else {
            return Err(LoadError::from(
                "instruction should be represented by a table value" ));
        };
        Instruction::try_from(table)
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
    operation: Option<Str>,
    args: Vec<Operand>,
    next: Option<Jump>,
    extra: SortedMap<Str, Value>,
    comment: Option<Str>,
    offset: (Option<f64>, Option<f64>),
}

impl InstructionBuilder {

    fn build_from(table: Table) -> Result<Instruction, LoadError> {
        let mut this = Self::default();
        let mut array = Vec::new();
        // Technically, instructions can have unlimited number
        // of arguments, and all of them can be None. But if
        // we do not limit the number of arguments somehow, we can be
        // tricked out of memory by a very large index.
        let max_index = i32::try_from(table.len() * 2 + 256)
            .unwrap_or(i32::MAX);
        for (key, value) in table {
            match key {
                Key::Index(index) if (1 ..= max_index).contains(&index)
                => {
                    let Ok(index) = usize::try_from(index - 1)
                        else { unreachable!(); };
                    if array.len() <= index {
                        array.resize_with(index + 1, || None);
                    }
                    array[index] = Some(value);
                },
                Key::Index(index) if index <= 0 =>
                    return Err(Self::err_unexpected_key(key)),
                Key::Index(index) =>
                    return Err(Self::err_non_continuous(index)),
                Key::Name(name) => match name.as_ref() {
                    "op"   => this.set_operation (value)?,
                    "next" => this.set_next      (value)?,
                    "cmt"  => this.set_comment   (value)?,
                    "nx"   => this.set_offset_x  (value)?,
                    "ny"   => this.set_offset_y  (value)?,
                    _ => {
                        let None = this.extra.insert(name, value) else {
                            unreachable!("duplicate key shouldn't be");
                        };
                    },
                },
            }
        }
        this.args.reserve_exact(array.len());
        for value in array {
            this.args.push(Operand::try_from(value)?);
        }
        this.build()
    }

    fn err_non_continuous(index: i32) -> LoadError { LoadError::from(format!(
        "instruction representation should have \
         argument indices in a range `1..N` (for a resonable N), \
         not {index:?}" )) }

    fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
        "instruction representation should not have {key:?} key" )) }

    fn set_operation(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::String(value) = value else {
            return Err(LoadError::from(
                "instruction's operation should be a string" ));
        };
        self.operation = Some(value); Ok(())
    }

    fn set_next(&mut self, value: Value) -> Result<(), LoadError> {
        self.next = Some(Jump::try_from(Some(value))?); Ok(())
    }

    fn set_comment(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::String(value) = value else {
            return Err(LoadError::from(
                "instruction's comment should be a string" ));
        };
        self.comment = Some(value); Ok(())
    }

    fn set_float(field: &mut Option<f64>, value: Value)
    -> Result<(), LoadError> {
        let Value::Float(value) = value else {
            return Err(LoadError::from(
                "instruction's offset should be a float" ));
        };
        *field = Some(value); Ok(())
    }

    fn set_offset_x(&mut self, value: Value) -> Result<(), LoadError> {
        Self::set_float(&mut self.offset.0, value)
    }

    fn set_offset_y(&mut self, value: Value) -> Result<(), LoadError> {
        Self::set_float(&mut self.offset.1, value)
    }

    fn build(self) -> Result<Instruction, LoadError> {
        let Self {
            operation, args, next,
            extra,
            comment,
            offset,
        } = self;
        let next = Jump::unwrap_option(next);
        let Some(operation) = operation else {
            return Err(LoadError::from(
                "Operation must be represented with a non-nil `op` field" ));
        };
        Ok(Instruction {
            operation, args, next,
            extra,
            comment,
            offset: Option::zip(offset.0, offset.1),
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
        while let Some(name) = map.next_key::<Identifier>()?.map(Str::from) {
            match name.as_ref() {
                "op"      => self.operation = Some(map.next_value()?),
                "args"    => self.args      = map.next_value()?,
                "next"    => self.next      = Some(map.next_value()?),
                "comment" => self.comment   = Some(map.next_value()?),
                "offset"  => self.offset    = Some(map.next_value::<>()?).unzip(),
                "extra"   => self.extra     = map.next_value()?,
                _ => return Err(A::Error::custom(
                    format!("instruction should not have “{name:?}” key") ))
            }
        }
        self.build().map_err(A::Error::custom)
    }

}

impl From<Instruction> for Value {
    fn from(this: Instruction) -> Value {
        let mut table_array = TableArrayBuilder::new();
        table_array.extend( this.args.into_iter()
            .map(Option::<Value>::from) );
        let mut table = table_array.build().into_builder();
        table.extend([
            ("op"  , Some(Value::String(this.operation))),
            ("next", Option::<Value>::from(this.next)),
            ("cmt" , this.comment.map(Value::String)),
            ("nx"  , this.offset.map(|(x,_)| Value::Float(x))),
            ("ny"  , this.offset.map(|(_,y)| Value::Float(y))),
        ].into_iter().filter_map(|(name, value)| {
            let value = value?;
            Some((Key::from(name), value))
        }).chain(this.extra.into_iter().map(|(name, value)| {
            if matches!(name.as_ref(), "op" | "next" | "cmt" | "nx" | "ny") {
                panic!("key {name:?} should not be in extra keys");
            }
            (Key::Name(name), value)
        })));
        Value::Table(table.build())
    }
}

impl Serialize for Instruction {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
    {
        let mut ser = ser.serialize_struct(
            "Instruction",
            2 // op and next
            + usize::from(!self.args.is_empty())
            + usize::from(!self.extra.is_empty())
            + usize::from(self.comment.is_some())
            + usize::from(self.offset.is_some())
        )?;

        ser.serialize_field("op", &self.operation)?;

        if !self.args.is_empty() {
            ser.serialize_field("args", &self.args)?;
        } else { ser.skip_field("args")?; }

        ser.serialize_field("next", &self.next)?;

        if !self.extra.is_empty() {
            ser.serialize_field("extra", &self.extra)?;
        } else { ser.skip_field("extra")?; }

        if let Some(ref comment) = self.comment {
            ser.serialize_field("comment", comment)?;
        } else { ser.skip_field("comment")?; }

        if let Some(ref offset) = self.offset {
            ser.serialize_field("offset", offset)?;
        } else { ser.skip_field("offset")?; }

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

