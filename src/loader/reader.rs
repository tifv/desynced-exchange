#![allow(dead_code)]

use std::marker::PhantomData;

use crate::{
    common::ptr_sub,
    ascii::{Ascii, AsciiArray, AsciiStr},
};

pub(super) struct Reader<'data> {
    // SAFETY-BEARING invariants:
    // `cursor <= end`;
    // both `end` and `cursor` are within the same allocatied object;
    // `cursor .. end` is initialized and borrowed for `'data`;
    cursor: *const u8,
    end: *const u8,
    lifetime: PhantomData<&'data [u8]>
}

impl<'data> Reader<'data> {

    #[inline]
    pub(super) fn from_slice(slice: &'data [u8]) -> Self {
        let std::ops::Range { start, end } = slice.as_ptr_range();
        Self { cursor: start, end, lifetime: PhantomData }
    }

    #[allow(dead_code)]
    #[inline]
    pub fn len(&self) -> usize {
        // SAFETY: struct invariant
        unsafe { ptr_sub(self.end, self.cursor) }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cursor == self.end
    }

    #[inline]
    pub(super) fn read_byte(&mut self) -> Option<u8> {
        self.read_array().map(|[x]| x)
    }

    #[inline]
    pub(super) fn read_end_byte(&mut self) -> Option<u8> {
        self.read_end_array().map(|[x]| x)
    }

    #[inline]
    pub(super) fn read_array<const N: usize>(&mut self) -> Option<[u8; N]> {
        if self.len() < N { return None; }
        // SAFETY: we have just checked the length of the slice
        let value = unsafe { self.cursor.cast::<[u8; N]>().read() };
        // SAFETY: we have just checked the length of the slice
        self.cursor = unsafe { self.cursor.add(N) };
        Some(value)
    }

    #[inline]
    pub(super) fn read_end_array<const N: usize>(&mut self) -> Option<[u8; N]> {
        if self.len() < N { return None; }
        // SAFETY: we have just checked the length of the slice
        self.end = unsafe { self.end.sub(N) };
        // SAFETY: we have just offset the pointer
        let value = unsafe { self.end.cast::<[u8; N]>().read() };
        Some(value)
    }

    #[inline]
    pub(super) fn read_slice(&mut self, len: usize) -> Option<&'data [u8]> {
        if self.len() < len { return None; }
        // SAFETY: we have just checked the length of the slice
        let slice = unsafe {
            std::slice::from_raw_parts(self.cursor, len)
        };
        // SAFETY: we have just checked the length of the slice
        self.cursor = unsafe { self.cursor.add(len) };
        Some(slice)
    }

    #[inline]
    pub(super) fn read_rest(&mut self) -> &'data [u8] {
        let len = self.len();
        // SAFETY: we have just checked the length of the slice
        let slice = unsafe {
            std::slice::from_raw_parts(self.cursor, len)
        };
        self.cursor = self.end;
        slice
    }

}

pub(super) struct AsciiReader<'data> {
    // SAFETY-BEARING invariant:
    // `inner` actually only contains ASCII bytes
    inner: Reader<'data>,
}

impl<'data> AsciiReader<'data> {

    #[inline]
    pub fn from_slice(slice: &'data [Ascii]) -> Self {
        Self { inner: Reader::from_slice(
            <&AsciiStr>::from(slice).into()
        ) }
    }

    #[allow(dead_code)]
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[inline]
    pub fn read_char(&mut self) -> Option<Ascii> {
        // SAFETY: struct invariant
        Some(unsafe { Ascii::from_byte_unchecked(
            self.inner.read_byte()?
        ) })
    }

    #[inline]
    pub fn read_end_char(&mut self) -> Option<Ascii> {
        // SAFETY: struct invariant
        Some(unsafe { Ascii::from_byte_unchecked(
            self.inner.read_end_byte()?
        ) })
    }

    #[inline]
    pub fn read_array<const N: usize>(&mut self) -> Option<[Ascii; N]> {
        // SAFETY: struct invariant
        Some(unsafe { *AsciiArray::from_bytes_unchecked(
            self.inner.read_array()?
        ) })
    }

    #[inline]
    pub fn read_end_array<const N: usize>(&mut self) -> Option<[Ascii; N]> {
        // SAFETY: struct invariant
        Some(unsafe { *AsciiArray::from_bytes_unchecked(
            self.inner.read_end_array()?
        ) })
    }

    #[inline]
    pub fn read_slice(&mut self, len: usize) -> Option<&'data [Ascii]> {
        // SAFETY: struct invariant
        Some(unsafe { AsciiStr::from_bytes_unchecked(
            self.inner.read_slice(len)?
        ) })
    }

    #[inline]
    pub fn read_rest(&mut self) -> &'data [Ascii] {
        // SAFETY: struct invariant
        unsafe { AsciiStr::from_bytes_unchecked(
            self.inner.read_rest()
        ) }
    }

}

