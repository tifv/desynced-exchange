#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::undocumented_unsafe_blocks)]
#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::hint::unreachable_unchecked;

use crate::ascii::{self, Ascii, AsciiArray, AsciiStr, AsciiString};

pub(super) struct Writer {
    start: *mut u8,
    cursor: *mut u8,
    end: *mut u8,
}

#[inline]
unsafe fn ptr_sub<T>(more: *mut T, less: *mut T) -> usize {
    let diff = more.offset_from(less);
    if diff < 0 {
        unreachable_unchecked()
    }
    diff as usize
}

impl Writer {

    #[inline]
    fn dangling() -> Self {
        let start = std::ptr::NonNull::dangling().as_ptr();
        Self{start, cursor: start, end: start}
    }

    #[inline]
    pub fn from_vec(vec: Vec<u8>) -> Self {
        let mut vec = vec;
        unsafe {
            let start = vec.as_mut_ptr();
            let cursor = start.add(vec.len());
            let end = start.add(vec.capacity());
            std::mem::forget(vec);
            Self{start, cursor, end}
        }
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
        unsafe {
            let Self{start, cursor, end} = self;
            let len = ptr_sub(cursor, start);
            let cap = ptr_sub(end, start);
            Vec::from_raw_parts(start, len, cap)
        }
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let this = std::mem::replace(self, Self::dangling());
        let mut vec = this.into_vec();
        vec.reserve(additional);
        Self{..} = std::mem::replace(self, Self::from_vec(vec));
    }

    #[inline]
    pub fn write_byte(&mut self, value: u8) {
        self.write_array::<1>([value])
    }

    #[inline]
    pub fn write_array<const N: usize>(&mut self, value: [u8; N]) {
        unsafe {
            if ptr_sub(self.cursor, self.start) < N {
                self.reserve(N);
            }
            self.cursor.cast::<[u8; N]>().write(value);
            self.cursor = self.cursor.add(N);
        }
    }

    #[inline]
    pub fn write_slice(&mut self, value: &[u8]) {
        let len = value.len();
        unsafe {
            if ptr_sub(self.cursor, self.start) < len {
                self.reserve(len);
            }
            std::ptr::copy_nonoverlapping(value.as_ptr(), self.cursor, len);
            self.cursor = self.cursor.add(len);
        }
    }

}

impl Drop for Writer {
    #[inline]
    fn drop(&mut self) {
        let this = std::mem::replace(self, Self::dangling());
        this.into_vec();
    }
}

pub(super) struct AsciiWriter {
    inner: Writer,
}

impl AsciiWriter {

    #[inline]
    pub fn from_string(string: AsciiString) -> Self {
        Self{inner: Writer::from_vec(string.into())}
    }

    #[inline]
    pub fn new() -> Self {
        Self{inner: Writer::new()}
    }

    #[inline]
    pub fn with_capacity(len: usize) -> Self {
        Self{inner: Writer::with_capacity(len)}
    }

    pub fn into_string(self) -> AsciiString {
        let vec = self.inner.into_vec();
        unsafe {
            AsciiString::from_bytes_unchecked(vec)
        }
    }

    pub fn write_byte(&mut self, value: Ascii) {
        self.write_array([value])
    }

    pub fn write_array<const N: usize>(&mut self, value: [Ascii; N]) {
        self.inner.write_array::<N>(AsciiArray::from(value).into())
    }

    pub fn write_slice(&mut self, value: &[Ascii]) {
        self.inner.write_slice(<&AsciiStr>::from(value).into())
    }

}


