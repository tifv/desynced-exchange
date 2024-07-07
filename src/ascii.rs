#![allow(clippy::use_self)]

use std::mem::transmute;

use thiserror::Error;

#[derive(Debug, Error)]
#[error("Ascii decoding error")]
pub(crate) struct AsciiError;

pub(crate) trait IsAscii {
    fn is_ascii(&self) -> bool;
}

pub(crate) const fn u8_is_ascii(value: u8) -> bool {
    matches!(value, b'\x00' ..= b'\x7F')
}

pub(crate) const fn char_is_ascii(value: char) -> bool {
    matches!(value, '\x00' ..= '\x7F')
}

pub(crate) const fn bytes_is_ascii(value: &[u8]) -> bool {
    let mut i = 0;
    while i < value.len() {
        if u8_is_ascii(value[i]) { i += 1; continue };
        return false;
    }
    true
}

pub(crate) const fn str_is_ascii(value: &str) -> bool {
    bytes_is_ascii(value.as_bytes())
}

impl IsAscii for u8 {
    #[inline]
    fn is_ascii(&self) -> bool {
        u8_is_ascii(*self)
    }
}

impl IsAscii for char {
    #[inline]
    fn is_ascii(&self) -> bool {
        char_is_ascii(*self)
    }
}

impl IsAscii for [u8] {
    #[inline]
    fn is_ascii(&self) -> bool {
        self.iter().all(IsAscii::is_ascii)
    }
}

impl<const N: usize> IsAscii for [u8; N] {
    #[inline]
    fn is_ascii(&self) -> bool {
        <[u8] as IsAscii>::is_ascii(self)
    }
}

impl IsAscii for Vec<u8> {
    #[inline]
    fn is_ascii(&self) -> bool {
        <[u8] as IsAscii>::is_ascii(self)
    }
}

impl IsAscii for str {
    #[inline]
    fn is_ascii(&self) -> bool {
        IsAscii::is_ascii(self.as_bytes())
    }
}

impl IsAscii for String {
    #[inline]
    fn is_ascii(&self) -> bool {
        <str as IsAscii>::is_ascii(self)
    }
}

/// A byte that holds ASCII value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub(crate) struct Ascii(u8);

impl Ascii {
    pub(crate) const unsafe fn from_byte_unchecked(
        value: u8,
    ) -> Self {
        //! # Safety
        //! The caller guarantees that the value is within
        //! the ASCII range.
        Ascii(value)
    }
}

impl std::ops::Deref for Ascii {
    type Target = u8;
    fn deref(&self) -> &u8 { &self.0 }
}

impl AsRef<u8> for Ascii {
    fn as_ref(&self) -> &u8 { self }
}

impl TryFrom<u8> for Ascii {
    type Error = AsciiError;
    #[inline]
    fn try_from(value: u8) -> Result<Self, AsciiError> {
        value.is_ascii()
            .then_some(Self(value))
            .ok_or(AsciiError)
    }
}

impl From<Ascii> for u8 {
    #[inline]
    fn from(value: Ascii) -> u8 {
        value.0
    }
}

impl TryFrom<char> for Ascii {
    type Error = AsciiError;
    #[inline]
    fn try_from(value: char) -> Result<Self, AsciiError> {
        value.is_ascii()
            .then_some(Self(value as u8))
            .ok_or(AsciiError)
    }
}

impl From<Ascii> for char {
    #[allow(clippy::use_self)]
    #[inline]
    fn from(value: Ascii) -> char {
        value.0 as char
    }
}

/// Mostly useful for enabling conversions of `[Ascii]`
/// into `str` and `[u8]`.
#[repr(transparent)]
pub(crate) struct AsciiStr(pub [Ascii]);

impl AsciiStr {
    const fn from_slice(value: &[Ascii]) -> &AsciiStr {
        // SAFETY: `AsciiStr` is transparent to `[Ascii]`
        unsafe {
            &*(value as *const [Ascii] as *const AsciiStr)
        }
    }
    pub(crate) const unsafe fn from_bytes_unchecked(
        value: &[u8],
    ) -> &AsciiStr {
        //! # Safety
        //! The caller guarantees that the value points to bytes within
        //! the ASCII range.
        // SAFETY: `Ascii` is transparent to `u8` and
        // the ASCII guarantee is provided by the caller.
        AsciiStr::from_slice(unsafe {
            &*(value as *const [u8] as *const [Ascii])
        })
    }
    pub(crate) unsafe fn from_bytes_unchecked_mut<'s>(
        value: &'s mut [u8],
    ) -> &'s mut AsciiStr {
        //! # Safety
        //! The caller guarantees that the value points to bytes within
        //! the ASCII range.
        // SAFETY: `Ascii` is transparent to `u8` and
        // the ASCII guarantee is provided by the caller.
        <&'s mut AsciiStr as From<&'s mut [Ascii]>>::from(unsafe {
            &mut *(value as *mut [u8] as *mut [Ascii])
        })
    }
}

impl std::ops::Deref for AsciiStr {
    type Target = [Ascii];
    fn deref(&self) -> &[Ascii] { &self.0 }
}

impl std::ops::DerefMut for AsciiStr {
    fn deref_mut(&mut self) -> &mut [Ascii] { &mut self.0 }
}

impl<'s> From<&'s [Ascii]> for &'s AsciiStr {
    fn from(value: &'s [Ascii]) -> &'s AsciiStr {
        AsciiStr::from_slice(value)
    }
}

impl<'s> From<&'s mut [Ascii]> for &'s mut AsciiStr {
    fn from(value: &'s mut [Ascii]) -> &'s mut AsciiStr {
        // SAFETY: `AsciiStr` is transparent to `[Ascii]`
        unsafe {
            &mut *(value as *mut [Ascii] as *mut AsciiStr)
        }
    }
}

impl<'s> TryFrom<&'s [u8]> for &'s AsciiStr {
    type Error = AsciiError;
    fn try_from(value: &'s [u8]) -> Result<Self, AsciiError> {
        value.is_ascii().then(||
            // SAFETY: we just checked for ASCII values
            unsafe { AsciiStr::from_bytes_unchecked(value) }
        ).ok_or(AsciiError)
    }
}

impl<'s> TryFrom<&'s mut [u8]> for &'s mut AsciiStr {
    type Error = AsciiError;
    fn try_from(value: &'s mut [u8]) -> Result<Self, AsciiError> {
        value.is_ascii().then(||
            // SAFETY: we just checked for ASCII values
            unsafe { AsciiStr::from_bytes_unchecked_mut(value) }
        ).ok_or(AsciiError)
    }
}

impl<'s> TryFrom<&'s str> for &'s AsciiStr {
    type Error = AsciiError;
    fn try_from(value: &'s str) -> Result<Self, AsciiError> {
        value.is_ascii().then(||
            // SAFETY: we just checked for ASCII values
            unsafe { AsciiStr::from_bytes_unchecked(value.as_bytes()) }
        ).ok_or(AsciiError)
    }
}

impl<'s> TryFrom<&'s mut str> for &'s mut AsciiStr {
    type Error = AsciiError;
    fn try_from(value: &'s mut str) -> Result<Self, AsciiError> {
        value.is_ascii().then(|| {
            // SAFETY: we will only allow writing ASCII to this
            let bytes = unsafe { value.as_bytes_mut() };
            // SAFETY: we just checked for ASCII values
            unsafe { AsciiStr::from_bytes_unchecked_mut(bytes) }
        } ).ok_or(AsciiError)
    }
}

impl<'s> From<&'s AsciiStr> for &'s [u8] {
    fn from(value: &'s AsciiStr) -> &'s [u8] {
        // SAFETY: `AsciiStr` is transparent to `[u8]`
        unsafe { &*(value as *const AsciiStr as *const [u8])  }
    }
}

impl<'s> From<&'s AsciiStr> for &'s str {
    fn from(value: &'s AsciiStr) -> &'s str {
        // SAFETY: all ASCII values are correct UTF-8
        unsafe { std::str::from_utf8_unchecked(value.into()) }
    }
}

#[repr(transparent)]
pub(crate) struct AsciiArray<const N: usize>(pub [Ascii; N]);

impl<const N: usize> AsciiArray<N> {
    #[allow(dead_code)]
    pub(crate) unsafe fn from_bytes_unchecked(
        value: [u8; N],
    ) -> AsciiArray<N> {
        //! # Safety
        //! The caller guarantees that the value contains bytes within
        //! the ASCII range.
        AsciiArray(value.map(Ascii))
    }
}

impl<const N: usize> std::ops::Deref for AsciiArray<N> {
    type Target = [Ascii; N];
    fn deref(&self) -> &[Ascii; N] { &self.0 }
}

impl<const N: usize> std::ops::DerefMut for AsciiArray<N> {
    fn deref_mut(&mut self) -> &mut [Ascii; N] { &mut self.0 }
}

impl<const N: usize> AsciiArray<N> {
    pub(crate) fn into_array(self) -> [Ascii; N] { self.0 }
}

impl<const N: usize> From<[Ascii; N]> for AsciiArray<N> {
    fn from(value: [Ascii; N]) -> Self {
        AsciiArray(value)
    }
}

impl<const N: usize> TryFrom<[u8; N]> for AsciiArray<N> {
    type Error = AsciiError;
    fn try_from(value: [u8; N]) -> Result<Self, AsciiError> {
        value.is_ascii()
            .then(|| AsciiArray(value.map(Ascii)) )
            .ok_or(AsciiError)
    }
}

impl<const N: usize> From<AsciiArray<N>> for [u8; N] {
    fn from(value: AsciiArray<N>) -> [u8; N] {
        value.into_array().map(Ascii::into)
    }
}

#[repr(transparent)]
pub(crate) struct AsciiString(pub Vec<Ascii>);

impl AsciiString {
    pub(crate) unsafe fn from_bytes_unchecked(value: Vec<u8>) -> Self {
        //! # Safety
        //! The caller guarantees that the vector contains bytes only within
        //! the ASCII range.
        // SAFETY: `Ascii` is transparent to `u8` and
        // the ASCII guarantee is provided by the caller.
        AsciiString(unsafe {
            transmute::<Vec<u8>, Vec<Ascii>>(value)
        })
    }
    pub(crate) unsafe fn from_string_unchecked(value: String) -> Self {
        //! # Safety
        //! The caller guarantees that the string contains only
        //! ASCII characters.
        // SAFETY: `AsciiStr` is transparent to `[u8]` and
        // the ASCII guarantee is provided by the caller.
        AsciiString(unsafe {
            transmute::<Vec<u8>, Vec<Ascii>>(value.into_bytes())
        })
    }
}

impl std::ops::Deref for AsciiString {
    type Target = Vec<Ascii>;
    fn deref(&self) -> &Vec<Ascii> { &self.0 }
}

impl std::ops::DerefMut for AsciiString {
    fn deref_mut(&mut self) -> &mut Vec<Ascii> { &mut self.0 }
}

impl TryFrom<Vec<u8>> for AsciiString {
    type Error = AsciiError;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        value.is_ascii().then(|| {
            // SAFETY: we have just checked for ASCII values
            unsafe { AsciiString::from_bytes_unchecked(value) }
        } ).ok_or(AsciiError)
    }
}

impl TryFrom<String> for AsciiString {
    type Error = AsciiError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.is_ascii().then(|| {
            // SAFETY: we have just checked for ASCII values
            unsafe { AsciiString::from_string_unchecked(value) }
        } ).ok_or(AsciiError)
    }
}

impl From<AsciiString> for Vec<u8> {
    fn from(value: AsciiString) -> Vec<u8> {
        // SAFETY: `Ascii` is transparent to `u8`
        unsafe { transmute::<Vec<Ascii>, Vec<u8>>(value.0) }
    }
}

impl From<AsciiString> for String {
    fn from(value: AsciiString) -> String {
        // SAFETY: `Ascii` is transparent to `u8`
        let vec = unsafe { transmute::<Vec<Ascii>, Vec<u8>>(value.0) };
        // SAFETY: We know that there are only ASCII values there
        unsafe { String::from_utf8_unchecked(vec) }
    }
}

macro_rules! ascii_char {
    ($value:literal) => { {
        const VALUE: $crate::ascii::Ascii = {
            assert!($crate::ascii::char_is_ascii($value));
            // SAFETY: we have just checked for ASCII value
            unsafe { $crate::ascii::Ascii::from_byte_unchecked(
                $value as u8
            ) }
        };
        VALUE
    } };
}

pub(crate) use ascii_char as char;

macro_rules! ascii_str {
    ($value:literal) => { {
        const VALUE: &'static $crate::ascii::AsciiStr = {
            let value: &'static str = $value;
            assert!($crate::ascii::str_is_ascii(value));
            // SAFETY: we have just checked for ASCII values
            unsafe { $crate::ascii::AsciiStr::from_bytes_unchecked(
                value.as_bytes()
            ) }
        };
        VALUE
    } };
}

pub(crate) use ascii_str as str;

