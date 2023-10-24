use serde::ser;

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

impl ser::Error for Error {
    fn custom<T>(msg: T) -> Self where T: std::fmt::Display {
        Self::from(Str::from(msg.to_string()))
    }
}

#[cfg(test)]
mod test {

    use serde::ser::Serialize as Se;

    use super::binary::Serializer as BinSer;

    use ron::{Value as V, Number as N, Map};

    struct HexBytes<'s>(&'s [u8]);
    impl<'s> std::fmt::Debug for HexBytes<'s> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let mut prev = false;
            for x in self.0 {
                if prev {
                    f.write_str(" ")?;
                }
                f.write_str(&format!("{x:02X}"))?;
                // std::fmt::UpperHex::fmt(x, f)?;
                prev = true;
            }
            Ok(())
        }
    }

    #[test]
    fn test_binary_serialize() {
        let mut ser = BinSer::new();
        let value: V = ron::from_str(r#"
            {"a": "b", 1 : 2, 2 : 3, 0 : 5, 42: [1, 2, 3, 4, 5]}
        "#).unwrap();
        value.serialize(&mut ser).unwrap();
        assert_eq!(
            format!("{:?}", HexBytes(&ser.into_output())),
            "asdf",
        );
    }
}