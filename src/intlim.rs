#![allow(clippy::use_self)]

use std::hint::unreachable_unchecked;

use thiserror::Error;

#[derive(Debug, Error)]
#[error("Integer base conversion error")]
pub(crate) struct IntLimError;

use crate::ascii::{self, Ascii};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Invariant: the value in the struct cannot be `>= L`
pub(crate) struct IntLim<const L: u8>(u8);

pub(crate) type Int31 = IntLim<31>;
pub(crate) type Int62 = IntLim<62>;

impl<const L: u8> IntLim<L> {

    pub(crate) const unsafe fn new_unchecked(value: u8) -> Self {
        Self(value)
    }

    #[inline]
    const fn assert_base() {
        assert!(L >= 2);
    }

    #[inline]
    pub(crate) fn zero() -> Self {
        assert!(L > 0);
        Self(0)
    }

    #[inline]
    pub(crate) const fn divrem(value: u32) -> (u32, Self) {
        Self::assert_base();
        let rem = value % (L as u32);
        let div = value / (L as u32);
        (div, Self(rem as u8))
    }

    #[inline]
    pub(crate) const fn be_decompose<const N: usize>(
        value: u32 ) -> (usize, [Self; N])
    {
        Self::assert_base();
        // conversion sanity check
        assert!(u32::BITS <= usize::BITS && N <= u32::MAX as usize);
        // overflow check
        assert!(match (L as u32).checked_pow(N as u32) {
            None => true,
            Some(max) => max > value,
        });
        let mut result = [Self(0); N];
        let mut index: usize = N;
        let mut value: u32 = value;
        while value > 0 {
            if index == 0 {
                // SAFETY: follows from the above overflow check
                unsafe { unreachable_unchecked() }
            }
            index -= 1;
            let (div, rem) = Self::divrem(value);
            value = div;
            result[index] = rem;
        }
        (index, result)
    }

    #[inline]
    pub(crate) const fn be_compose(
        value: &[Self] ) -> Result<u32, IntLimError>
    {
        let mut result: u32 = 0;
        let mut index = 0;
        while index < value.len() {
            let x = value[index];
            let Some(a) = result.checked_mul(L as u32) else {
                return Err(IntLimError);
            };
            let Some(b) = a.checked_add(x.0 as u32) else {
                return Err(IntLimError);
            };
            result = b;
            index += 1;
        }
        Ok(result)
    }

    #[inline]
    pub(crate) const fn sufficient_digits() -> usize {
        (u32::MAX.ilog(L as u32) + 1) as usize
    }

}

impl<const L: u8> TryFrom<u8> for IntLim<L> {
    type Error = IntLimError;
    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        (value < L)
            .then_some(value).map(Self)
            .ok_or(IntLimError)
    }
}

impl<const L: u8> From<IntLim<L>> for u8 {
    #[inline]
    fn from(value: IntLim<L>) -> u8 {
        let value = value.0;
        if value >= L {
            // SAFETY: `IntLim` type invariant
            unsafe { unreachable_unchecked() }
        }
        value
    }
}

#[inline]
pub(crate) fn encode_base62(value: Int62) -> Ascii {
    let value = value.0;
    let encoded = match value {
         0 ..=  9 => b'0' +  value      ,
        10 ..= 35 => b'A' + (value - 10),
        36 ..= 61 => b'a' + (value - 36),
        // SAFETY: `Int62` type invariant
        _ => unsafe { std::hint::unreachable_unchecked() },
    };
    // SAFETY: `encoded` value lies within the ASCII range
    unsafe { Ascii::from_byte_unchecked(encoded) }
}

#[inline]
pub(crate) fn decode_base62(value: Ascii) -> Result<Int62, IntLimError> {
    let value: u8 = value.into();
    let decoded = match value {
        b'0' ..= b'9' =>  value - b'0',
        b'A' ..= b'Z' => (value - b'A') + 10,
        b'a' ..= b'z' => (value - b'a') + 36,
        _ => return Err(IntLimError),
    };
    // SAFETY: `decoded` value is less than 62
    Ok(unsafe { Int62::new_unchecked(decoded) })
}

impl Int31 {
    #[inline]
    pub(super) fn add_31(self) -> Int62 {
        IntLim::<62>(self.0 + 31)
    }
}

impl From<Int31> for Int62 {
    #[inline]
    fn from(value: Int31) -> Int62 {
        Self(value.0)
    }
}

impl Int62 {
    #[inline]
    pub(super) fn try_as_31(self) -> Result<Int31, Int31> {
        match self.0 {
            // SAFETY: `b` lies in range
            b @  0 ..= 30 => Ok (unsafe { Int31::new_unchecked(b) }),
            // SAFETY: `b - 31` lies in range
            b @ 31 ..= 61 => Err(unsafe { Int31::new_unchecked(b - 31) }),
            // SAFETY: `Int62` struct invariant
            62 ..= u8::MAX => unsafe { unreachable_unchecked() }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{ascii::{Ascii, AsciiStr}, intlim::IntLimError};
    use super::{IntLim, Int31, Int62, encode_base62, decode_base62};

    #[test]
    fn test_divrem() {
        const L: u8 = 10;
        assert_eq!(IntLim::<L>::divrem(42), (4, IntLim::<L>(2)));
    }

    #[test]
    fn test_decompose_10() {
        const L: u8 = 10;
        const D: usize = IntLim::<L>::sufficient_digits();
        assert_eq!(D, 10);
        assert_eq!(
            IntLim::<L>::be_decompose::<10>(u32::MAX),
            (0, [4,2,9,4,9,6,7,2,9,5].map(IntLim::<L>)),
        );
    }

    #[test]
    fn test_decompose_62() {
        const L: u8 = 62;
        const D: usize = IntLim::<L>::sufficient_digits();
        assert_eq!(D, 6);
        assert_eq!(
            IntLim::<L>::be_decompose::<D>(u32::MAX),
            (0, [  4, 42, 41, 15, 12,  3].map(IntLim::<L>)),
        );
    }

    #[test]
    fn test_decompose_31() {
        const L: u8 = 31;
        const D: usize = IntLim::<L>::sufficient_digits();
        assert_eq!(D, 7);
        assert_eq!(
            IntLim::<L>::be_decompose::<7>(0),
            (7, [0,0,0,0,0,0,0].map(IntLim::<L>)),
        );
        assert_eq!(
            IntLim::<L>::be_decompose::<7>(u32::MAX),
            (0, [  4, 26,  0, 19, 29, 24,  3].map(IntLim::<L>)),
        );
    }

    #[test]
    fn test_encode_base62() {
        for x in 0 .. 62 {
            let x = Int62::try_from(x).unwrap();
            let c = encode_base62(x);
            assert!(matches!(char::from(c),
                '0' ..= '9' | 'A' ..= 'Z' | 'a' ..= 'z' ))
        }
        for x in 62 .. u8::MAX {
            let IntLimError = Int62::try_from(x).unwrap_err();
        }
    }

    #[test]
    fn test_encode_base62_seq() {
        fn encode(value: u32) -> String {
            const D: usize = Int62::sufficient_digits();
            let x = Int62::be_decompose::<D>(value).1.map(encode_base62);
            String::from(<&str>::from(<&AsciiStr>::from(x.as_slice())))
        }
        assert_eq!(encode(u32::MAX     ), "4gfFC3");
        assert_eq!(encode(1_000_000_000), "15ftgG");
        assert_eq!(encode(    1_000_000), "004C92");
        assert_eq!(encode(        1_000), "0000G8");
        assert_eq!(encode(           62), "000010");
        assert_eq!(encode(           61), "00000z");
        assert_eq!(encode(            1), "000001");
        assert_eq!(encode(            0), "000000");
    }

    #[test]
    fn test_decode_base62() {
        let mut checked = [false; 62];
        for b in u8::MIN ..= u8::MAX {
            let decodable = matches!(b,
                b'0' ..= b'9' | b'A' ..= b'Z' | b'a' ..= b'z' );
            let Ok(c) = Ascii::try_from(b) else {
                assert!(!decodable);
                continue;
            };
            let Ok(x) = decode_base62(c) else {
                assert!(!decodable);
                continue;
            };
            assert!(decodable);
            assert!(u8::from(x) < 62);
            assert!(!checked[u8::from(x) as usize]);
            checked[u8::from(x) as usize] = true;
        }
        assert!(checked.into_iter().all(std::convert::identity));
    }

    #[test]
    fn test_decode_base62_seq() {
        fn decode(value: &str) -> u32 {
            let digits = value.as_bytes().iter().copied()
                .map(|b| Ascii::try_from(b).unwrap())
                .map(|c| decode_base62(c).unwrap() )
                .collect::<Vec<_>>();
            Int62::be_compose(&digits).unwrap()
        }
        assert_eq!(decode("4gfFC3"), u32::MAX     );
        assert_eq!(decode("15ftgG"), 1_000_000_000);
        assert_eq!(decode("004C92"),     1_000_000);
        assert_eq!(decode("0000G8"),         1_000);
        assert_eq!(decode("000010"),            62);
        assert_eq!(decode("00000z"),            61);
        assert_eq!(decode("000001"),             1);
        assert_eq!(decode("000000"),             0);
    }

}

