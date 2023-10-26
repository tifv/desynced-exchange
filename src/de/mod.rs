use serde::de;

use thiserror::Error;

use crate::string::Str;

mod binary;
mod compress;

#[derive(Debug, Error)]
#[error("Serialization error: {reason}")]
pub struct Error {
    reason: Str,
}

impl From<&'static str> for Error {
    fn from(reason: &'static str) -> Self {
        Self{reason: Str::Name(reason)}
    }
}

impl From<Str> for Error {
    fn from(reason: Str) -> Self {
        Self{reason}
    }
}

impl de::Error for Error {
    fn custom<T>(msg: T) -> Self where T: std::fmt::Display {
        Self::from(Str::from(msg.to_string()))
    }
}

#[cfg(test)]
mod test {

}
