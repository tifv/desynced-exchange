//! A specialized imitation of `serde::ser`.

use crate::table_iter::{TableItem, TableSize};

pub trait Error : std::error::Error + for<'s> From<&'s str> {}

pub trait KeyLoad : Sized {
    fn load_key<L: Loader>(loader: L) -> Result<Option<Self>, L::Error>;
}

pub trait Load : Sized {
    fn load<L: Loader>(loader: L) -> Result<Option<Self>, L::Error>;
}

pub trait KeyBuilder : Sized {
    type Output;
    fn build_integer<E: Error>(self, value: i32) -> Result<Self::Output, E>;
    fn build_string<E: Error>(self, value: &str) -> Result<Self::Output, E>;
}

pub trait Builder : Sized {
    type Output;
    type Key: KeyLoad;
    type Value: Load;
    fn build_nil<E: Error>(self) -> Result<Option<Self::Output>, E> {
        Ok(None)
    }
    fn build_boolean<E: Error>(self, value: bool) -> Result<Option<Self::Output>, E>;
    fn build_integer<E: Error>(self, value: i32) -> Result<Option<Self::Output>, E>;
    fn build_float<E: Error>(self, value: f64) -> Result<Option<Self::Output>, E>;
    fn build_string<E: Error>(self, value: &str) -> Result<Option<Self::Output>, E>;
    fn build_table<T>(self, items: T) -> Result<Option<Self::Output>, T::Error>
    where
        T: TableLoader<Key=Self::Key, Value=Self::Value>,
        T::Error : Error;
}

pub trait Loader {
    type Error: Error;
    fn load_value<B: Builder>( self,
        builder: B,
    ) -> Result<Option<B::Output>, Self::Error>;
    fn load_key<KB: KeyBuilder>( self,
        builder: KB,
    ) -> Result<Option<KB::Output>, Self::Error>;
}

pub trait TableLoader : TableSize + Iterator<
    Item = Result<Option<TableItem<Self::Key, Self::Value>>, Self::Error>
> {
    type Key : KeyLoad;
    type Value : Load;
    type Error : Error;
}

