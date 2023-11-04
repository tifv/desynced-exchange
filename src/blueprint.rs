use crate::{
    Exchange,
    load::error::Error as LoadError,
    dump::error::Error as DumpError,
    value::Value,
    behavior::Behavior,
};

pub struct Blueprint {
    _whatever: ()
}

impl TryFrom<Value> for Blueprint {
    type Error = LoadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl From<Blueprint> for Value {
    fn from(value: Blueprint) -> Self {
        todo!()
    }
}

pub fn load_blueprint(exchange: &str)
-> Result<Exchange<Blueprint, Behavior>, LoadError>
{
    let value = crate::load::load_blueprint::<Value, Value>(exchange)?;
    let value = value.transpose().ok_or_else(|| LoadError::from(
        "Blueprint or behavior should not be represented with nil" ))?;
    value.map(Blueprint::try_from, Behavior::try_from).transpose()
}

pub fn dump_blueprint(blueprint: Exchange<Blueprint, Behavior>)
-> Result<String, DumpError>
{
    let value = blueprint.map(Blueprint::into, Behavior::into)
        .map(Some, Some);
    crate::dump::dump_blueprint::<Value, Value>(value)
}

#[cfg(test)]
mod test {
    use crate::Exchange;

    use super::{load_blueprint, dump_blueprint};

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
}

