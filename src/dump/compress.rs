use flate2::write::ZlibEncoder as ZippingWriter;

use crate::ascii::{self, Ascii, AsciiArray};

use super::{
    error::Error,
    writer::{Writer, AsciiWriter},
    ExchangeKind,
};

pub(super) fn compress(
    kind: ExchangeKind,
    body: &[u8],
) -> String {
    let mut writer = AsciiWriter::with_capacity(36);
    writer.write_slice(match kind {
        ExchangeKind::Blueprint(()) => ascii::str!("DSB"),
        ExchangeKind::Behavior(()) => ascii::str!("DSC"),
    });
    let mut zipped = None;
    let (len, body) = if body.len() <= 32 { (0, body) } else {
        let len = body.len();
        let zipped = zipped.insert(zip(body));
        (len, &**zipped)
    };
    writer.into_string().into()
}

pub(super) fn write_len_base31(write: &mut AsciiWriter, len: usize) {
    if len == 0 {
        return write.write_byte(ascii::char!('V'));
    }
    let mut value: u32 = len.try_into()
        .expect("the len should not be that large");
    while value > 0 {
        let (next_value, index) = base62::Int31::divrem(value);
        value = next_value;
        write.write_byte(if value > 0 {
            index.into()
        } else {
            index.add_31().into()
        });
    }
}

mod base62 {
    #![allow(clippy::use_self)]

    use crate::ascii::{self, Ascii};

    // Invariant: the value in the struct cannot exceed 61
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub(super) struct Int62(u8);

    impl Int62 {
        #[inline]
        pub(super) fn divrem(value: u32) -> (u32, Int62) {
            let rem = value % 62;
            let div = value /  62;
            (div, Int62(rem as u8))
        }
    }

    impl From<Int62> for Ascii {
        #[inline]
        fn from(value: Int62) -> Ascii {
            let value = value.0;
            let byte = match value {
                 0 ..=  9 => b'0' +  value      ,
                10 ..= 35 => b'A' + (value - 10),
                36 ..= 61 => b'a' + (value - 36),
                // SAFETY: `Int62` type invariant
                _ => unsafe { std::hint::unreachable_unchecked() },
            };
            // SAFETY: `byte` lies within the ASCII range
            unsafe { Ascii::from_byte_unchecked(byte) }
        }
    }

    // Invariant: the value in the struct cannot exceed 61
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub(super) struct Int31(u8);

    impl Int31 {
        #[inline]
        pub(super) fn divrem(value: u32) -> (u32, Int31) {
            let rem = value % 31;
            let div = value / 31;
            (div, Int31(rem as u8))
        }
        #[inline]
        pub(super) fn add_31(self) -> Int62 {
            Int62(self.0 + 31)
        }
    }

    impl From<Int31> for Int62 {
        #[inline]
        fn from(value: Int31) -> Int62 {
            Self(value.0)
        }
    }

    impl From<Int31> for Ascii {
        #[inline]
        fn from(value: Int31) -> Ascii {
            Int62::from(value).into()
        }
    }

}

fn zip(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut zipper = ZippingWriter::new(
        Vec::<u8>::new(),
        flate2::Compression::best(),
    );
    zipper.write_all(data).unwrap();
    zipper.try_finish().unwrap();
    zipper.finish().unwrap()
}

fn encode(data: &[u8]) -> (AsciiWriter, Ascii) {
    let mut encoder = Base62Encoder::new(AsciiWriter::new());
    encoder.write_all(data);
    encoder.finish()
}

const WORD_LEN: usize = 4;
const ENCODED_WORD_LEN: usize = 6;

// Invariant: `buffer_len` is always less than or equal to the `WORD_LEN`
struct Base62Encoder {
    write: AsciiWriter,
    buffer: [u8; WORD_LEN],
    buffer_len: u8,
    checksum: std::num::Wrapping<u32>,
}

impl Base62Encoder {
    fn new(write: AsciiWriter) -> Self {
        Self{
            write,
            buffer: [0; WORD_LEN],
            buffer_len: 0,
            checksum: std::num::Wrapping(0),
        }
    }
    fn finish(mut self) -> (AsciiWriter, Ascii) {
        if self.buffer_len > 0 {
            self.consume_final_word();
        }
        let Self{write, checksum, buffer_len: 0, ..} = self else {
            unreachable!()
        };
        let checksum = base62::Int62::divrem(checksum.0).1.into();
        (write, checksum)
    }
    #[inline]
    fn take_word(&mut self) -> u32 {
        let word_bytes = std::mem::replace(&mut self.buffer, [0; WORD_LEN]);
        self.buffer_len = 0;
        u32::from_le_bytes(word_bytes)
    }
    #[inline]
    fn consume_word(&mut self) {
        let word = self.take_word();
        self.checksum += word;
        let (encoded, _) = Self::encode_word(word);
        self.write.write_array(encoded);
    }
    #[inline]
    fn consume_final_word(&mut self) {
        let word = self.take_word();
        self.checksum += word;
        let (encoded, start) = Self::encode_word(word);
        let start = start as usize;
        self.write.write_slice(&encoded[start ..]);
    }
    #[inline]
    fn encode_word(mut word: u32) -> ([Ascii; ENCODED_WORD_LEN], u8) {
        assert!({ const OK: bool = {
            let ok = 62_u32.checked_pow(6).is_none();
            assert!(ok); ok
        }; OK});
        let mut result = [ascii::char!('0'); ENCODED_WORD_LEN];
        let mut start = ENCODED_WORD_LEN;
        while word > 0 {
            if start == 0 {
                // SAFETY: `62.pow(start) > word` is an invariant
                unsafe { std::hint::unreachable_unchecked(); }
            }
            let (word_div, rem) = base62::Int62::divrem(word);
            word = word_div;
            start -= 1;
            result[start] = rem.into();
        }
        if start >= ENCODED_WORD_LEN {
            // SAFETY: `start` only decreases
            unsafe { std::hint::unreachable_unchecked(); }
        }
        (result, start as u8)
    }
    fn write_some(&mut self, slice: &[u8]) -> usize {
        assert!(!slice.is_empty(), "slice should not be empty");
        let mut buffer_len = self.buffer_len as usize;
        if buffer_len > WORD_LEN {
            // SAFETY: `Self` struct invariant
            unsafe { std::hint::unreachable_unchecked() }
        }
        if buffer_len == WORD_LEN {
            self.consume_word();
            buffer_len = 0;
        }
        let start = buffer_len;
        let write_size = usize::min(WORD_LEN - start, slice.len());
        // SAFETY: `start` is a correct offset into `buffer`
        let buffer_start = unsafe {
            self.buffer.as_mut_ptr().add(start) };
        // SAFETY: `write_size` is a correct length for both regions
        unsafe {
            std::ptr::copy_nonoverlapping(
                slice.as_ptr(), buffer_start, write_size );
        }
        self.buffer_len += write_size as u8;
        write_size
    }
    fn write_all(&mut self, mut slice: &[u8]) {
        while !slice.is_empty() {
            let written = self.write_some(slice);
            if written > slice.len() {
                // SAFETY: `write_some()` will never write more
                // than `slice.len()`
                unsafe { std::hint::unreachable_unchecked() }
            }
            slice = slice.split_at(written).1;
        }
    }
}

