use thiserror::Error;

macro_rules! error_from_error {
    ($toerror:ty: <- $fromerror:ty) => {
        impl From<$fromerror> for $toerror {
            fn from(value: $fromerror) -> $toerror {
                Self::from(value.to_string())
            }
        }
    };
}

#[derive(Debug, Error)]
#[error("Load error: {reason}")]
pub struct LoadError {
    reason: String,
}

impl crate::load::Error for LoadError {}

impl From<&str> for LoadError {
    fn from(reason: &str) -> Self {
        Self { reason: String::from(reason) }
    }
}

impl From<String> for LoadError {
    fn from(reason: String) -> Self {
        Self { reason }
    }
}

error_from_error!(LoadError: <- crate::common::ascii::AsciiError);
error_from_error!(LoadError: <- crate::common::intlim::IntLimError);
error_from_error!(LoadError: <- std::str::Utf8Error);
error_from_error!(LoadError: <- std::io::Error);


#[derive(Debug, Error)]
#[error("Dump error: {reason}")]
pub struct DumpError {
    reason: String,
}

impl crate::dump::Error for DumpError {}

impl From<&str> for DumpError {
    fn from(reason: &str) -> Self {
        Self { reason: String::from(reason) }
    }
}

impl From<String> for DumpError {
    fn from(reason: String) -> Self {
        Self { reason }
    }
}

