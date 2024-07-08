use serde::{Deserialize, Serialize};

use crate::{
    error::{LoadError, DumpError},
    value::Value,
    operand::OpValue,
};

pub use crate::{
    Exchange,
    behavior::Behavior,
};

fn bool_true() -> bool { true }

#[allow(clippy::trivially_copy_pass_by_ref)]
fn bool_is_true(&b: &bool) -> bool { b }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Blueprint {

    pub frame: String,

    #[serde( default="bool_true",
        skip_serializing_if="bool_is_true" )]
    pub powered: bool,

    #[serde( default,
        skip_serializing_if="Vec::is_empty" )]
    pub locks: Vec<Option<String>>,

    #[serde( default,
        skip_serializing_if="Vec::is_empty" )]
    pub logistics: Vec<(String,bool)>,

    pub components: Vec<Component>,

    #[serde( default,
        skip_serializing_if="Vec::is_empty" )]
    pub registers: Vec<Option<OpValue>>,

    #[serde( default,
        skip_serializing_if="Vec::is_empty" )]
    pub links: Vec<(i32,i32)>,

}

impl TryFrom<Value> for Blueprint {
    type Error = LoadError;
    fn try_from(_value: Value) -> Result<Self, Self::Error> {
        Err(LoadError::from(
            "Structural representation of blueprints is not yet supported" ))
    }
}

impl From<Blueprint> for Value {
    fn from(_value: Blueprint) -> Self {
        todo!(
            "Structural representation of blueprints is not yet supported" )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Component {
    item: String,
    index: i32,
    registers: Vec<Option<OpValue>>,
    behavior: Option<Behavior>,
}

pub fn load_blueprint(exchange: &str)
-> Result<Exchange<Blueprint, Behavior>, LoadError>
{
    type V = Value;
    let value = crate::loader::load_blueprint::<V, V, LoadError>(exchange)?;
    let value = value.transpose().ok_or_else(|| LoadError::from(
        "Blueprint or behavior should not be represented with nil" ))?;
    value.map(Blueprint::try_from, Behavior::try_from).transpose()
}

pub fn dump_blueprint(blueprint: Exchange<Blueprint, Behavior>)
-> Result<String, DumpError>
{
    type V = Value;
    let value = blueprint.map(Blueprint::into, Behavior::into)
        .map(Some, Some);
    crate::dumper::dump_blueprint::<V, V>(value)
}

#[cfg(test)]
mod test {
    use crate::Exchange;

    use super::{load_blueprint, dump_blueprint};

    #[test]
    fn test_load_error() {
        let exchange = "asdf";
        let Err(_) = load_blueprint(exchange)
            else { panic!("should be an error") };
    }

    #[test]
    fn test_load_behavior_1_unit() {
        let exchange = crate::test::EXCHANGE_BEHAVIOR_1_UNIT;
        let Exchange::Behavior(_behavior) =
            load_blueprint(exchange).unwrap()
            else { panic!("should be a behavior") };
    }

    #[test]
    fn test_load_dump_behavior_2() {
        let exchange = crate::test::EXCHANGE_BEHAVIOR_2;
        let Exchange::Behavior(behavior) =
            load_blueprint(exchange).unwrap()
            else { panic!("should be a behavior") };
        dump_blueprint(Exchange::Behavior(behavior)).unwrap();
    }

    #[test]
    fn test_load_behavior_3_param() {
        let exchange = crate::test::EXCHANGE_BEHAVIOR_3_PARAM;
        let Exchange::Behavior(_behavior) =
            load_blueprint(exchange).unwrap()
            else { panic!("should be a behavior") };
    }

    #[test]
    fn test_load_behavior_4_sub() {
        let exchange = crate::test::EXCHANGE_BEHAVIOR_4_SUB;
        let Exchange::Behavior(_behavior) =
            load_blueprint(exchange).unwrap()
            else { panic!("should be a behavior") };
    }

}

