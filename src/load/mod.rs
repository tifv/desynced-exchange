//! A specialized imitation of `serde::ser`.

use std::io::Read;

use thiserror::Error;

use crate::{
    Exchange,
    table::{TableItem, TableSize},
};

mod value;
mod compress;

#[derive(Debug, Error)]
#[error("Load error: {reason}")]
pub struct Error {
    reason: String,
}

impl From<&str> for Error {
    fn from(reason: &str) -> Self {
        Self{reason: String::from(reason)}
    }
}

impl From<String> for Error {
    fn from(reason: String) -> Self {
        Self{reason}
    }
}

macro_rules! error_from_error {
    ($type:ty) => {
        impl From<$type> for Error {
            fn from(value: $type) -> Self {
                Self::from(value.to_string())
            }
        }
    };
}

error_from_error!(std::num::TryFromIntError);
error_from_error!(std::io::Error);
error_from_error!(std::string::FromUtf8Error);

pub fn load_blueprint<P, B>(data: &str) -> Result<Exchange<P, B>, Error>
where P: Load, B: Load,
{
    todo!()
}

pub trait LoadKey : Sized {
    fn load_key<L: KeyLoader>(loader: L) -> Result<Option<Self>, Error>;
}

pub trait Load : Sized {
    fn load<L: Loader>(loader: L) -> Result<Self, Error>;
    fn is_nil(&self) -> bool;
}

pub trait LoadTableIterator : TableSize + Iterator<
    Item=Result<Option<TableItem<Self::Key, Self::Value>>, Error> >
{
    type Key: LoadKey;
    type Value: Load;
}

pub trait KeyBuilder : Sized {
    type Value: LoadKey;
    fn build_integer(self, value: i32) -> Result<Self::Value, Error>;
    fn build_string<R: Read>( self,
        len: u32, value: R,
    ) -> Result<Self::Value, Error>;
}

pub trait Builder : Sized {
    type Key: LoadKey;
    type Value: Load;
    fn build_nil(self) -> Result<Self::Value, Error>;
    fn build_boolean(self, value: bool) -> Result<Self::Value, Error>;
    fn build_integer(self, value: i32) -> Result<Self::Value, Error>;
    fn build_float(self, value: f64) -> Result<Self::Value, Error>;
    fn build_string<R: Read>( self,
        len: u32, value: R,
    ) -> Result<Self::Value, Error>;
    fn build_table<T>(self, items: T) -> Result<Self::Value, Error>
    where T: LoadTableIterator<Key=Self::Key, Value=Self::Value>;
}

pub trait Loader {
    fn load_value<B: Builder>( self,
        builder: B,
    ) -> Result<B::Value, Error>;
}

pub trait KeyLoader : Loader {
    fn load_key<B: KeyBuilder>( self,
        builder: B,
    ) -> Result<Option<B::Value>, Error>;
}


