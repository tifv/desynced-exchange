use serde::{Deserialize, Serialize};

use crate::{
    Exchange,
    load::error::Error as LoadError,
    dump::error::Error as DumpError,
    value as v,
    behavior::{Behavior, Value},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Blueprint {
    pub frame: String,
    pub powered: bool,
    pub locks: Vec<Option<String>>,
    pub logistics: Option<Vec<(String,bool)>>,
    pub components: Vec<Component>,
    pub registers: Vec<Option<Value>>,
    pub links: Vec<(i32,i32)>,
}

impl TryFrom<v::Value> for Blueprint {
    type Error = LoadError;
    fn try_from(value: v::Value) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl From<Blueprint> for v::Value {
    fn from(value: Blueprint) -> Self {
        todo!()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Component {
    item: String,
    index: i32,
    registers: Option<Vec<Option<Value>>>,
    behavior: Option<Behavior>,
}

pub fn load_blueprint(exchange: &str)
-> Result<Exchange<Blueprint, Behavior>, LoadError>
{
    type V = v::Value;
    let value = crate::load::load_blueprint::<V, V>(exchange)?;
    let value = value.transpose().ok_or_else(|| LoadError::from(
        "Blueprint or behavior should not be represented with nil" ))?;
    value.map(Blueprint::try_from, Behavior::try_from).transpose()
}

pub fn dump_blueprint(blueprint: Exchange<Blueprint, Behavior>)
-> Result<String, DumpError>
{
    type V = v::Value;
    let value = blueprint.map(Blueprint::into, Behavior::into)
        .map(Some, Some);
    crate::dump::dump_blueprint::<V, V>(value)
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

