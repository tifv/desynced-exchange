#![allow(clippy::use_self)]

use std::collections::btree_map::BTreeMap as SortedMap;

use serde::{Deserialize, Serialize};

use crate::{
    error::{LoadError, DumpError},
    Str,
    common::serde::{
        option_some as serde_option_some,
        vec_option_wrap as serde_vec_option_wrap,
    },
    value::{Key, Value as _Value, Table, ArrayBuilder as TableArrayBuilder},
};

pub use crate::Exchange;

mod behavior;
pub use behavior::{Behavior, Parameter};

mod instruction;
pub use instruction::Instruction;

mod operand;
pub use operand::{Operand, Jump, Place, Value};

fn bool_true() -> bool { true }

#[allow(clippy::trivially_copy_pass_by_ref)]
fn bool_is_true(&b: &bool) -> bool { b }

#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Blueprint {

    #[serde( default,
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub name: Option<Str>,

    pub frame: Str,

    #[serde( default="bool_true",
        skip_serializing_if="bool_is_true" )]
    pub powered: bool,

    #[serde( default="bool_true",
        skip_serializing_if="bool_is_true" )]
    pub connected: bool,

    #[serde( default,
        skip_serializing_if="SortedMap::is_empty" )]
    pub logistics: SortedMap<Str, bool>,

    pub components: Vec<Component>,

    #[serde( default,
        skip_serializing_if="Vec::is_empty",
        with="serde_vec_option_wrap" )]
    pub registers: Vec<Option<Value>>,

    #[serde( default,
        skip_serializing_if="Vec::is_empty" )]
    pub links: Vec<(i32, i32)>,

    #[serde( default,
        skip_serializing_if="Vec::is_empty",
        with="serde_vec_option_wrap" )]
    pub locks: Vec<Option<Str>>,

}

impl Default for Blueprint {
    fn default() -> Self {
        Self {
            name: None,
            frame: Str::default(),
            powered: true,
            connected: true,
            locks: Vec::new(),
            logistics: SortedMap::new(),
            components: Vec::new(),
            registers: Vec::new(),
            links: Vec::new(),
        }
    }
}

impl TryFrom<_Value> for Blueprint {
    type Error = LoadError;
    fn try_from(value: _Value) -> Result<Blueprint, Self::Error> {
        let _Value::Table(table) = value else {
            return Err(LoadError::from(
                "blueprint should be represented by a table value" ));
        };
        Blueprint::try_from(table)
    }
}

impl TryFrom<Table> for Blueprint {
    type Error = LoadError;
    fn try_from(table: Table) -> Result<Blueprint, Self::Error> {
        BlueprintBuilder::build_from(table)
    }
}

#[derive(Default)]
struct BlueprintBuilder {
    name: Option<Str>,
    frame: Option<Str>,
    powered: Option<bool>,
    connected: Option<bool>,
    logistics: SortedMap<Str, bool>,
    components: Vec<Component>,
    registers: Vec<Option<Value>>,
    links: Vec<(i32, i32)>,
    locks: Vec<Option<Str>>,
}

impl BlueprintBuilder {

    fn build_from(table: Table) -> Result<Blueprint, LoadError> {
        let mut this = Self::default();
        for (key, value) in table {
            let Key::Name(name) = key else {
                return Err(Self::err_unexpected_key(key));
            };
            match name.as_ref() {
                "name"         => this.set_name        (value)?,
                "frame"        => this.set_frame       (value)?,
                "powered_down" => this.set_powered_down(value)?,
                "disconnected" => this.set_disconnected(value)?,
                "logistics"    => this.set_logistics   (value)?,
                "components"   => this.set_components  (value)?,
                "regs"         => this.set_registers   (value)?,
                "links"        => this.set_links       (value)?,
                "locks"        => this.set_locks       (value)?,
                _ => return Err(Self::err_unexpected_key(Key::Name(name))),
            }
        }
        this.build()
    }

    fn err_unexpected_key(key: Key) -> LoadError { LoadError::from(format!(
        "blueprint representation should not have {key:?} key" )) }

    fn set_name(&mut self, value: _Value) -> Result<(), LoadError> {
        let _Value::String(value) = value else {
            return Err(LoadError::from(
                "blueprints's `name` should be a string" ));
        };
        self.name = Some(value); Ok(())
    }

    fn set_frame(&mut self, value: _Value) -> Result<(), LoadError> {
        let _Value::String(value) = value else {
            return Err(LoadError::from(
                "blueprints's `frame` should be a string" ));
        };
        self.frame = Some(value); Ok(())
    }

    fn set_powered_down(&mut self, value: _Value) -> Result<(), LoadError> {
        let _Value::Boolean(value) = value else {
            return Err(LoadError::from(
                "blueprints's powered status should be a boolean" ));
        };
        self.powered = Some(!value); Ok(())
    }

    fn set_disconnected(&mut self, value: _Value) -> Result<(), LoadError> {
        let _Value::Boolean(value) = value else {
            return Err(LoadError::from(
                "blueprints's connected status should be a boolean" ));
        };
        self.connected = Some(!value); Ok(())
    }

    fn set_logistics(&mut self, value: _Value) -> Result<(), LoadError> {
        let _Value::Table(table) = value else {
            return Err(Self::err_logistics());
        };
        #[allow(clippy::shadow_unrelated)]
        for (key, value) in table {
            let Key::Name(name) = key else {
                return Err(Self::err_logistics());
            };
            let _Value::Boolean(value) = value else {
                return Err(Self::err_logistics());
            };
            self.logistics.insert(name, value);
        }
        Ok(())
    }
    fn err_logistics() -> LoadError { LoadError::from(
        "blueprints's `logistics` should be a table mapping string keys \
         to boolean values" ) }

    fn set_components(&mut self, value: _Value) -> Result<(), LoadError> {
        let _Value::Table(table) = value else {
            return Err(Self::err_components());
        };
        for item in table.into_continuous_iter() {
            let item = item.map_err(|_error| Self::err_components())?;
            self.components.push(Component::try_from(item)?);
        }
        Ok(())
    }
    fn err_components() -> LoadError { LoadError::from(
        "blueprints's `components` should be a continuous array of tables" ) }

    fn set_registers(&mut self, value: _Value) -> Result<(), LoadError> {
        let _Value::Table(table) = value else {
            return Err(Self::err_registers());
        };
        let max_len = table.len().saturating_mul(2).saturating_add(256);
        let table = table.into_array_iter();
        if table.len() > max_len {
            return Err(LoadError::from(
                "unrealistically large number of blueprint registers"));
        }
        for item in table {
            self.registers.push(item.map(Value::try_from).transpose()?);
        }
        Ok(())
    }
    fn err_registers() -> LoadError { LoadError::from(
        "blueprints's `registers` should be an array of values" ) }

    fn set_links(&mut self, value: _Value) -> Result<(), LoadError> {
        let _Value::Table(table) = value else {
            return Err(Self::err_links());
        };
        for item in table.into_continuous_iter() {
            let item = item.map_err(|_error| Self::err_links())?;
            let _Value::Table(item) = item else {
                return Err(Self::err_links());
            };
            let mut item = item.into_continuous_iter();
            let Some(Ok(_Value::Integer(x))) = item.next() else {
                return Err(Self::err_links());
            };
            let Some(Ok(_Value::Integer(y))) = item.next() else {
                return Err(Self::err_links());
            };
            if item.next().is_some() {
                return Err(Self::err_links());
            };
            self.links.push((x, y));
        }
        Ok(())
    }
    fn err_links() -> LoadError { LoadError::from(
        "blueprints's `links` should be an array of pairs of integers" ) }

    fn set_locks(&mut self, value: _Value) -> Result<(), LoadError> {
        let _Value::Table(table) = value else {
            return Err(Self::err_locks());
        };
        let max_len = table.len().saturating_mul(2).saturating_add(256);
        let table = table.into_array_iter();
        if table.len() > max_len {
            return Err(LoadError::from(
                "unrealistically large number of blueprint locks"));
        }
        for item in table {
            let item = match item {
                None => None,
                Some(_Value::String(name)) => Some(name),
                _ => return Err(Self::err_locks()),
            };
            self.locks.push(item);
        }
        Ok(())
    }
    fn err_locks() -> LoadError { LoadError::from(
        "blueprints's `locks` should be a sparse array of item ids" ) }

    fn build(self) -> Result<Blueprint, LoadError> {
        let Self {
            name,
            frame, powered, connected, logistics,
            components, registers, links,
            locks,
        } = self;
        let Some(frame) = frame else {
            return Err(LoadError::from(
                "Blueprint must have a `frame` defined" ));
        };
        for (i, j) in links.iter().copied() {
            for x in [i, j] {
                if x <= 0 || x as usize > registers.len() {
                    return Err(LoadError::from(
                        "Link index is incorrect" ));
                }
            }
        }
        Ok(Blueprint {
            name,
            frame,
            powered: powered.unwrap_or(true),
            connected: connected.unwrap_or(true),
            logistics,
            components,
            registers,
            links,
            locks,
        })
    }
}

impl From<Blueprint> for _Value {
    fn from(this: Blueprint) -> _Value {
        use TableArrayBuilder as ArrayBuilder;
        let Blueprint {
            name: blueprint_name,
            frame, powered, connected, logistics,
            components, registers, links,
            locks,
        } = this;
        #[allow(clippy::from_iter_instead_of_collect)]
        _Value::Table(Table::from_iter([
            ("name"        , blueprint_name.map(_Value::String)),
            ("frame"       , Some(_Value::String(frame))),
            ("powered_down", (!powered).then_some(_Value::Boolean(true))),
            ("disconnected", (!connected).then_some(_Value::Boolean(true))),
            ("logistics"   , if logistics.is_empty() { None } else { Some(
                _Value::Table(Table::from_iter(
                    logistics.into_iter().map( |(key, setting)|
                        (Key::Name(key), _Value::Boolean(setting))
                    )
                ))
            ) }),
            ("components"  , if components.is_empty() { None } else { Some(
                _Value::Table(ArrayBuilder::from_iter(
                    components.into_iter().map(_Value::from)
                ).build())
            ) }),
            ("regs"        , if registers.is_empty() { None } else { Some(
                _Value::Table(ArrayBuilder::from_iter(
                    registers.into_iter()
                        .map(|value| value.map(_Value::from))
                ).build())
            ) }),
            ("links"       , if links.is_empty() { None } else { Some(
                _Value::Table(ArrayBuilder::from_iter(
                    links.into_iter().map( |(x, y)| {
                        let x = _Value::Integer(x);
                        let y = _Value::Integer(y);
                        _Value::Table(ArrayBuilder::from_iter([x, y]).build())
                    })
                ).build())
            ) }),
            ("locks"       , if locks.is_empty() { None } else { Some(
                _Value::Table(ArrayBuilder::from_iter(
                    locks.into_iter().map(|value| value.map(_Value::String))
                ).build())
            ) }),
        ].into_iter().filter_map(|(name, value)| {
            let value = value?;
            Some((Key::from(name), value))
        })))
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Component {

    pub item: Str,

    pub index: i32,

    #[serde( default,
        skip_serializing_if="Option::is_none",
        with="serde_option_some" )]
    pub behavior: Option<Behavior>,

    #[serde( default,
        skip_serializing_if="Vec::is_empty",
        with="serde_vec_option_wrap" )]
    pub registers: Vec<Option<Value>>,

}

impl TryFrom<_Value> for Component {
    type Error = LoadError;
    fn try_from(value: _Value) -> Result<Self, Self::Error> {
        let _Value::Table(table) = value else {
            return Err(LoadError::from(
                "component should be represented by a table value" ));
        };
        Component::try_from(table)
    }
}

impl TryFrom<Table> for Component {
    type Error = LoadError;
    fn try_from(table: Table) -> Result<Component, Self::Error> {
        let mut this = Self::default();
        let mut table = table.into_continuous_iter();
        let _Value::String(item) = table.next()
            .and_then(Result::ok)
            .ok_or_else(|| Self::Error::from(
                "component should have an item id" ))?
        else {
            return Err(Self::Error::from(
                "component's item id should be a string" ));
        };
        this.item = item;
        let _Value::Integer(index) = table.next()
            .and_then(Result::ok)
            .ok_or_else(|| Self::Error::from(
                "component should have an index" ))?
        else {
            return Err(Self::Error::from(
                "component's index should be an integer" ));
        };
        this.index = index;
        if let Some(behavior) = table.next()
            .transpose()
            .map_err(|_error| Self::Error::from(
                "component should either have a behavior \
                 or no third parameter at all" ))?
        {
            let _Value::Table(behavior) = behavior else {
                return Err(Self::Error::from(
                    "component's behavior should be represented \
                     by a table value" ));
            };
            this.behavior = Some(Behavior::try_from(behavior)?);
        }
        Ok(this)
    }
}

impl From<Component> for _Value {
    fn from(this: Component) -> _Value {
        let Component {
            item,
            index,
            behavior,
            registers: _registers,
        } = this;
        _Value::Table(TableArrayBuilder::from_iter([
            Some(_Value::String(item)),
            Some(_Value::Integer(index)),
            behavior.map(_Value::from),
        ]).build())
    }
}

pub fn load_blueprint(exchange: &str)
-> Result<Exchange<Blueprint, Behavior>, LoadError>
{
    type V = _Value;
    let value = crate::loader::load_blueprint::<V, V, LoadError>(exchange)?;
    let value = value.transpose().ok_or_else(|| LoadError::from(
        "Blueprint or behavior should not be represented with nil" ))?;
    value.map(Blueprint::try_from, Behavior::try_from).transpose()
}

pub fn dump_blueprint(blueprint: Exchange<Blueprint, Behavior>)
-> Result<String, DumpError>
{
    type V = _Value;
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

