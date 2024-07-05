#![allow(dead_code)]

use crate::{
    common::ptr_sub,
    ascii::{Ascii, AsciiArray, AsciiStr, AsciiString}
};

pub(super) struct Writer {
    // SAFETY-BEARING invariants:
    // `start <= cursor <= end`;
    // `start .. end` is an object allocated by `Vec<u8>`;
    // …and we own it;
    // `start .. cursor` is initialized.
    start: *mut u8,
    cursor: *mut u8,
    end: *mut u8,
}

impl Writer {

    #[inline]
    fn dangling() -> Self {
        let start = std::ptr::NonNull::dangling().as_ptr();
        Self { start, cursor: start, end: start }
    }

    #[inline]
    pub fn from_vec(vec: Vec<u8>) -> Self {
        let mut vec = vec;
        let start = vec.as_mut_ptr();
        // SAFETY: known properties of `Vec`
        let cursor = unsafe { start.add(vec.len()) };
        // SAFETY: known properties of `Vec`
        let end = unsafe { start.add(vec.capacity()) };
        std::mem::forget(vec);
        Self { start, cursor, end }
    }

    #[inline]
    pub fn new() -> Self {
        Self::from_vec(Vec::new())
    }

    #[inline]
    pub fn with_capacity(len: usize) -> Self {
        Self::from_vec(Vec::with_capacity(len))
    }

    #[inline]
    pub fn into_vec(self) -> Vec<u8> {
        let Self { start, cursor, end } = self;
        std::mem::forget(self);
        // SAFETY: struct invariant
        let len = unsafe { ptr_sub(cursor, start) };
        // SAFETY: struct invariant
        let cap = unsafe { ptr_sub(end, start) };
        // SAFETY: struct invariant
        unsafe { Vec::from_raw_parts(start, len, cap) }
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let this = std::mem::replace(self, Self::dangling());
        let mut vec = this.into_vec();
        vec.reserve(additional);
        let dangling = std::mem::replace(self, Self::from_vec(vec));
        std::mem::forget(dangling);
    }

    #[inline]
    fn free_cap(&self) -> usize {
        // SAFETY: struct invariant
        unsafe { ptr_sub(self.end, self.cursor) }
    }

    #[inline]
    pub fn write_byte(&mut self, value: u8) {
        self.write_array::<1>([value])
    }

    #[inline]
    pub fn write_array<const N: usize>(&mut self, value: [u8; N]) {
        if self.free_cap() < N {
            self.reserve(N);
        }
        // SAFETY: we have just checked the free capacity
        unsafe {
            self.cursor.cast::<[u8; N]>().write(value);
        }
        // SAFETY: we have just checked the free capacity
        self.cursor = unsafe { self.cursor.add(N) };
    }

    #[inline]
    pub fn write_slice(&mut self, value: &[u8]) {
        let len = value.len();
        if self.free_cap() < len {
            self.reserve(len);
        }
        // SAFETY: we have just checked the free capacity;
        // also, we own our slice, so it is indeed nonoverlapping.
        unsafe {
            std::ptr::copy_nonoverlapping(value.as_ptr(), self.cursor, len);
        }
        // SAFETY: we have just checked the free capacity.
        self.cursor = unsafe { self.cursor.add(len) };
    }

}

impl Drop for Writer {
    #[inline]
    fn drop(&mut self) {
        let this = std::mem::replace(self, Self::dangling());
        std::mem::drop(this.into_vec());
    }
}

pub(super) struct AsciiWriter {
    // SAFETY-BEARING invariant:
    // `inner` actually only receives ASCII bytes
    inner: Writer,
}

impl AsciiWriter {

    #[inline]
    pub fn from_string(string: AsciiString) -> Self {
        Self { inner: Writer::from_vec(string.into()) }
    }

    #[inline]
    pub fn new() -> Self {
        Self { inner: Writer::new() }
    }

    #[inline]
    pub fn with_capacity(len: usize) -> Self {
        Self { inner: Writer::with_capacity(len) }
    }

    pub fn into_string(self) -> AsciiString {
        let vec = self.inner.into_vec();
        // SAFETY: struct invariant
        unsafe {
            AsciiString::from_bytes_unchecked(vec)
        }
    }

    pub fn write_byte(&mut self, value: Ascii) {
        self.inner.write_byte(value.into())
    }

    pub fn write_array<const N: usize>(&mut self, value: [Ascii; N]) {
        self.inner.write_array::<N>(AsciiArray::from(value).into())
    }

    pub fn write_slice(&mut self, value: &[Ascii]) {
        self.inner.write_slice(<&AsciiStr>::from(value).into())
    }

}

