#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::undocumented_unsafe_blocks)]
#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::marker::PhantomData;
use std::hint::unreachable_unchecked;

use crate::ascii::{Ascii, AsciiArray, AsciiStr};

pub(super) struct Reader<'data> {
    cursor: *const u8,
    end: *const u8,
    lifetime: PhantomData<&'data [u8]>
}

#[inline]
unsafe fn ptr_sub<T>(more: *const T, less: *const T) -> usize {
    let diff = more.offset_from(less);
    if diff < 0 {
        unreachable_unchecked()
    }
    diff as usize
}

impl<'data> Reader<'data> {

    #[inline]
    pub(super) fn from_slice(slice: &'data [u8]) -> Self {
        let std::ops::Range{start, end} = slice.as_ptr_range();
        Self{cursor: start, end, lifetime: PhantomData}
    }

    #[inline]
    pub(super) fn into_slice(self) -> &'data [u8] {
        unsafe {
            let Self{cursor: start, end, ..} = self;
            let len = ptr_sub(end, start);
            std::slice::from_raw_parts(start, len)
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
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
        unsafe {
            if self.len() < N {
                return None;
            }
            let value = self.cursor.cast::<[u8; N]>().read();
            self.cursor = self.cursor.add(N);
            Some(value)
        }
    }

    #[inline]
    pub(super) fn read_end_array<const N: usize>(&mut self) -> Option<[u8; N]> {
        unsafe {
            if self.len() < N {
                return None;
            }
            self.end = self.end.sub(N);
            let value = self.end.cast::<[u8; N]>().read();
            Some(value)
        }
    }

    #[inline]
    pub(super) fn read_slice(&mut self, len: usize) -> Option<&'data [u8]> {
        unsafe {
            if self.len() < len {
                return None;
            }
            let slice = std::slice::from_raw_parts(self.cursor, len);
            self.cursor = self.cursor.add(len);
            Some(slice)
        }
    }

    #[inline]
    pub(super) fn read_rest(&mut self) -> &'data [u8] {
        unsafe {
            let len = self.len();
            let slice = std::slice::from_raw_parts(self.cursor, len);
            self.cursor = self.end;
            slice
        }
    }

}

pub(super) struct AsciiReader<'data> {
    reader: Reader<'data>,
}

impl<'data> AsciiReader<'data> {

    #[inline]
    pub fn from_slice(slice: &'data [Ascii]) -> Self {
        Self{reader: Reader::from_slice(
            <&AsciiStr>::from(slice).into() )}
    }

    #[inline]
    pub fn into_slice(self) -> &'data [Ascii] {
        let slice = self.reader.into_slice();
        unsafe {
            AsciiStr::from_bytes_unchecked(slice)
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.reader.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.reader.is_empty()
    }

    #[inline]
    pub fn read_char(&mut self) -> Option<Ascii> {
        self.read_array().map(|[x]| x)
    }

    #[inline]
    pub fn read_end_char(&mut self) -> Option<Ascii> {
        self.read_end_array().map(|[x]| x)
    }

    #[inline]
    pub fn read_array<const N: usize>(&mut self) -> Option<[Ascii; N]> {
        Some(unsafe { *AsciiArray::from_bytes_unchecked(
            self.reader.read_array()?
        ) })
    }

    #[inline]
    pub fn read_end_array<const N: usize>(&mut self) -> Option<[Ascii; N]> {
        Some(unsafe { *AsciiArray::from_bytes_unchecked(
            self.reader.read_end_array()?
        ) })
    }

    #[inline]
    pub fn read_slice(&mut self, len: usize) -> Option<&'data [Ascii]> {
        Some(unsafe { AsciiStr::from_bytes_unchecked(
            self.reader.read_slice(len)?
        ) })
    }

    #[inline]
    pub fn read_rest(&mut self) -> &'data [Ascii] {
        unsafe { AsciiStr::from_bytes_unchecked(
            self.reader.read_rest()
        ) }
    }

}

