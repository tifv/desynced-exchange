#![allow(clippy::use_self)]

use serde::{
    Serialize,
    Deserialize,
};

use crate::{
    error::LoadError,
    Str,
    common::{
        u32_to_usize,
        serde::option_some as serde_option_some,
    },
    value::{Key, Value, Table, ArrayBuilder as TableArrayBuilder},
};

use super::Instruction;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Behavior {

    #[serde( default,
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub name: Option<Str>,

    #[serde( default,
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub description: Option<Str>,

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
    pub name: Option<Str>,
    pub is_output: bool,
}

impl TryFrom<Value> for Behavior {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Behavior, Self::Error> {
        let Value::Table(table) = value else {
            return Err(LoadError::from(
                "behavior should be represented by a table value" ));
        };
        Behavior::try_from(table)
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
    name: Option<Str>,
    description: Option<Str>,
    parameters: Vec<Parameter>,
    parameter_names: Option<Table>,
    instructions: Vec<Instruction>,
    subroutines: Vec<Behavior>,
}

impl BehaviorBuilder {

    fn build_from(table: Table) -> Result<Behavior, LoadError> {
        let mut this = Self::default();
        let mut array = Vec::new();
        for (key, value) in table {
            match key {
                Key::Index(index) if index > 0 &&
                    u32_to_usize((index - 1) as u32) == array.len()
                => array.push(value),
                Key::Index(index) =>
                    return Err(Self::err_non_continuous(index)),
                Key::Name(name) => match name.as_ref() {
                    "name"       => this.set_name           (value)?,
                    "desc"       => this.set_description    (value)?,
                    "parameters" => this.set_parameters     (value)?,
                    "pnames"     => this.set_parameter_names(value)?,
                    "subs"       => this.set_subroutines    (value)?,
                    _ => return Err(Self::err_unexpected_key(Key::Name(name))),
                },
            }
        }
        this.instructions.reserve_exact(array.len());
        for value in array {
            this.instructions.push(Instruction::try_from(value)?);
        }
        this.build()
    }

    fn err_non_continuous(index: i32) -> LoadError { LoadError::from(format!(
        "behavior representation should have \
         instruction indices in a continuous range `1..n`: {index:?}" )) }

    fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
        "behavior representation should not have {key:?} key" )) }

    fn set_name(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::String(value) = value else {
            return Err(LoadError::from(
                "behavor's name should be a string" ));
        };
        self.name = Some(value); Ok(())
    }

    fn set_description(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::String(value) = value else {
            return Err(LoadError::from(
                "behavor's description should be a string" ));
        };
        self.description = Some(value); Ok(())
    }

    fn set_parameters(&mut self, value: Value) -> Result<(), LoadError> {
        let Value::Table(table) = value else {
            return Err(Self::err_parameters());
        };
        for item in table.into_continuous_iter() {
            let item = item.map_err(|_error| Self::err_parameters())?;
            let Value::Boolean(is_output) = item else {
                return Err(Self::err_parameters());
            };
            self.parameters.push(Parameter { is_output, name: None });
        }
        Ok(())
    }
    fn err_parameters() -> LoadError { LoadError::from(
        "behavior's parameters should be \
         a continuous array of booleans" ) }

    fn set_parameter_names(&mut self, value: Value)
    -> Result<(), LoadError> {
        let Value::Table(table) = value else {
            return Err(Self::err_param_names());
        };
        self.parameter_names = Some(table);
        Ok(())
    }

    fn reconcile_parameter_names(
        parameters: &mut [Parameter],
        parameter_names: Table,
    ) -> Result<(), LoadError> {
        for (index, value) in parameter_names {
            #[allow(clippy::unnecessary_lazy_evaluations)]
            let Some(index) = index.as_index()
                .filter(|&x| x > 0).map(|x| x - 1)
                .map(usize::try_from).and_then(Result::ok)
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

    fn set_subroutines(&mut self, value: Value)
    -> Result<(), LoadError> {
        let Value::Table(table) = value else {
            return Err(Self::err_subroutines());
        };
        for item in table.into_continuous_iter() {
            let item = item.map_err(|_error| Self::err_subroutines())?;
            self.subroutines.push(Behavior::try_from(item)?);
        }
        Ok(())
    }
    fn err_subroutines() -> LoadError { LoadError::from(
        "behavior's subroutines should be \
         a continuous array" ) }

    fn build(self) -> Result<Behavior, LoadError> {
        let Self {
            name, description,
            mut parameters, parameter_names,
            instructions,
            subroutines,
        } = self;
        if let Some(parameter_names) = parameter_names {
            Self::reconcile_parameter_names(&mut parameters, parameter_names)?;
        }
        Ok(Behavior {
            name, description,
            parameters,
            instructions,
            subroutines,
        })
    }

}

impl From<Behavior> for Value {
    fn from(this: Behavior) -> Value {
        let Behavior {
            instructions,
            name: behavior_name, description,
            parameters,
            subroutines,
        } = this;
        let mut table_array = TableArrayBuilder::new();
        table_array.extend( instructions.into_iter()
            .map(Value::from) );
        let mut table = table_array.build().into_builder();
        table.extend([
            ("name"      , behavior_name.map(Value::String)),
            ("desc"      , description.map(Value::String)),
            ("parameters", if parameters.is_empty() { None } else {
                Some(Value::Table( parameters.iter()
                    .map(|param| Some(Value::Boolean(param.is_output)))
                    .collect::<TableArrayBuilder<_>>().build() ))
            }),
            ("pnames"    , if parameters.is_empty() { None } else {
                Some(Value::Table( parameters.into_iter()
                    .map(|param| param.name.map(Value::String))
                    .collect::<TableArrayBuilder<_>>().build() ))
            }),
            ("subs"      , if subroutines.is_empty() { None } else {
                Some(Value::Table( subroutines.into_iter()
                    .map(Value::from)
                    .collect::<TableArrayBuilder<_>>().build() ))
            }),
        ].into_iter().filter_map(|(name, value)| {
            let value = value?;
            Some((Key::from(name), value))
        }));
        Value::Table(table.build())
    }
}

#[cfg(test)]
mod test {

use super::Behavior;

#[test]
fn test_map_1_de() {
    let s = r#"Behavior(
        name: "Behavior Name",
        instructions: [],
    )"#;
    let _: Behavior = ron::from_str(s).unwrap();
}

}

