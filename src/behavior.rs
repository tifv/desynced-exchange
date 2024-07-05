#![allow(clippy::use_self)]

use serde::{
    Serialize,
    Deserialize,
};

use crate::{
    error::LoadError,
    common::ilog2_ceil,
    string::Str,
    value::{self as v, Key, TableIntoError as TableError},
    serde::option_some as serde_option_some,
    instruction::Instruction,
};

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

impl TryFrom<v::Value> for Behavior {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Behavior, Self::Error> {
        let v::Value::Table(table) = value else {
            return Err(LoadError::from(
                "behavior should be represented by a table value" ));
        };
        Behavior::try_from(table)
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
    name: Option<Str>,
    description: Option<Str>,
    parameters: Vec<Parameter>,
    parameter_names: Option<v::Table>,
    instructions: Vec<Instruction>,
    subroutines: Vec<Behavior>,
}

impl BehaviorBuilder {

    fn build_from(table: v::Table) -> Result<Behavior, LoadError> {
        let mut this = Self::default();
        let vector: Vec<_> = table.try_into_seq_and_named(
            |name, value| match name.as_ref() {
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
            this.push(Parameter { is_output: value, name: None });
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

impl From<Behavior> for v::Value {
    fn from(this: Behavior) -> v::Value {
        let mut table = v::TableBuilder::new(
            this.instructions.len().try_into()
                .expect("length should fit"),
            ilog2_ceil(
                usize::from(this.name.is_some()) +
                usize::from(this.description.is_some()) +
                2 * usize::from(!this.parameters.is_empty()) +
                usize::from(!this.subroutines.is_empty())
            ),
        );
        table.array_extend( this.instructions.into_iter()
            .map(v::Value::from).map(Some) );
        if let Some(name) = this.name {
            table.assoc_insert("name", Some(v::Value::String(name)));
        }
        if let Some(description) = this.description {
            table.assoc_insert( "desc",
                Some(v::Value::String(description)) );
        }
        if !this.parameters.is_empty() {
            table.assoc_insert("parameters", Some(v::Value::Table(
                this.parameters.iter()
                    .map(|param| Some(v::Value::Boolean(param.is_output)))
                    .collect::<v::TableArrayBuilder<_>>().finish()
            )));
            table.assoc_insert("pnames", Some(v::Value::Table(
                this.parameters.into_iter()
                    .map(|param| param.name.map(v::Value::String))
                    .collect::<v::TableArrayBuilder<_>>().finish()
            )));
        }
        if !this.subroutines.is_empty() {
            table.assoc_insert("subs", Some(v::Value::Table(
                this.subroutines.into_iter()
                    .map(|sub| Some(v::Value::from(sub)))
                    .collect::<v::TableArrayBuilder<_>>().finish()
            )));
        }
        v::Value::Table(table.finish())
    }
}

#[cfg(test)]
mod test {

use super::Behavior;

#[test]
fn test_map_1() {
    let s = r#"Behavior(
        name: "Behavior Name",
        instructions: [],
    )"#;
    // eprintln!("{s:?}");
    let _: Behavior = ron::from_str(s).unwrap();
}

#[test]
fn test_map_2() {
    // XXX this is not actually a valid blueprint
    // (removed parameters in the subroutine)
    let s = r#"
    Behavior(
    name: "Test Behavior",
    parameters:
    [(is_output:false,),(is_output:true,),(is_output:true,),],
    instructions: [
    ( op: "remap_value", args: [ Index(1), Number(100), Number(200),
    Number(1000), Number(3000), Index(2), ], next: Next, ),
    ( op: "get_self", args: [ Variable("A"), ], next: Next, ),
    ( op: "call", args: [ Variable("A"), Variable("B"), Unset, Unset,
    Index(2), ], next: Next, sub: 1, ),
    ( op: "lock", next: Next, ),
    ( op: "set_reg", args: [ Variable("B"), Index(3), ], next: Next, ), ],
    subroutines: [ ( name: "Test Subroutine",
    parameters: [ ( is_output: false ), ( is_output: true )],
    instructions: [
    ( op: "unlock", next: Next, ),
    ( op: "check_grid_effeciency", args: [ Index(4), Index(1), ], next:
    Next, ),
    ( op: "set_reg", args: [ Number(1), Index(2), ], next: Return, ),
    ( op: "set_reg", args: [ Number(2), Index(2), ], next: Next, ),
    ],),],)
    "#;
    // eprintln!("{s:?}");
    let _: Behavior = ron::from_str(s).unwrap();
}

}

