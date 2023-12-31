//! A specialized imitation of `serde::ser`.

use crate::{
    Exchange,
    table::{TableItem, TableSize},
};

mod reader;
pub(crate) mod value;
pub(crate) mod decompress;

pub trait Error : std::error::Error + for<'s> From<&'s str> {}

pub mod error {
    use thiserror::Error;

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

    error_from_error!(crate::ascii::AsciiError);
    error_from_error!(crate::intlim::IntLimError);
    error_from_error!(std::str::Utf8Error);
    error_from_error!(std::io::Error);

    impl super::Error for Error {}

}

pub trait LoadKey : Sized {
    fn load_key<L: Loader>(loader: L) -> Result<Option<Self>, L::Error>;
}

pub trait Load : Sized {
    fn load<L: Loader>(loader: L) -> Result<Option<Self>, L::Error>;
    // fn is_nil(&self) -> bool;
}

pub trait LoadTableIterator : TableSize + Iterator<
    Item=Result<Option<TableItem<Self::Key, Self::Value>>, Self::Error> >
{
    type Key: LoadKey;
    type Value: Load;
    type Error: Error;
}

pub trait KeyBuilder : Sized {
    type Output;
    fn build_integer<E: Error>(self, value: i32) -> Result<Self::Output, E>;
    fn build_string<E: Error>(self, value: &str) -> Result<Self::Output, E>;
}

pub trait Builder : Sized {
    type Output;
    type Key: LoadKey;
    type Value: Load;
    fn build_nil<E: Error>(self) -> Result<Option<Self::Output>, E> {
        Ok(None)
    }
    fn build_boolean<E: Error>(self, value: bool) -> Result<Option<Self::Output>, E>;
    fn build_integer<E: Error>(self, value: i32) -> Result<Option<Self::Output>, E>;
    fn build_float<E: Error>(self, value: f64) -> Result<Option<Self::Output>, E>;
    fn build_string<E: Error>(self, value: &str) -> Result<Option<Self::Output>, E>;
    fn build_table<T, E: Error>(self, items: T) -> Result<Option<Self::Output>, E>
    where T: LoadTableIterator<Key=Self::Key, Value=Self::Value, Error=E>;
}

pub trait Loader {
    type Error: Error;
    fn load_value<B: Builder>( self,
        builder: B,
    ) -> Result<Option<B::Output>, Self::Error>;
    fn load_key<B: KeyBuilder>( self,
        builder: B,
    ) -> Result<Option<B::Output>, Self::Error>;
}

pub fn load_blueprint<P, B>(exchange: &str)
-> Result<Exchange<Option<P>, Option<B>>, error::Error>
where P: Load, B: Load,
{
    value::decode_blueprint(decompress::decompress(exchange)?)
}

