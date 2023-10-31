use flate2::write::ZlibEncoder as ZippingWriter;

use crate::{
    ascii::{self, Ascii, AsciiArray},
    intlim::{Int62, Int31, encode_base62},
};

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
    write_len_base31(&mut writer, len);
    let checksum = write_encoded(&mut writer, body);
    writer.write_byte(checksum);
    writer.into_string().into()
}

pub(super) fn write_len_base31(write: &mut AsciiWriter, len: usize) {
    const MAX_DIGITS: usize = Int31::sufficient_digits();
    if len == 0 {
        return write.write_byte(ascii::char!('V'));
    }
    let value: u32 = len.try_into()
        .expect("the len should not be that large");
    let (mut index, digits) = Int31::be_decompose::<MAX_DIGITS>(value);
    while index < MAX_DIGITS - 1 {
        write.write_byte(encode_base62(digits[index].into()));
        index += 1;
    }
    write.write_byte(encode_base62(digits[MAX_DIGITS-1].add_31()));
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

fn write_encoded(writer: &mut AsciiWriter, data: &[u8]) -> Ascii {
    let mut encoder = Base62Encoder::new(writer);
    encoder.write_all(data);
    encoder.finish()
}

const WORD_LEN: usize = (u32::BITS / u8::BITS) as usize;
const ENCODED_WORD_LEN: usize = Int62::sufficient_digits();

// Invariant: `buffer_len` is always less than or equal to the `WORD_LEN`
struct Base62Encoder<'w> {
    writer: &'w mut AsciiWriter,
    buffer: [u8; WORD_LEN],
    buffer_len: u8,
    checksum: std::num::Wrapping<u32>,
}

impl<'w> Base62Encoder<'w> {
    fn new(writer: &'w mut AsciiWriter) -> Self {
        Self{
            writer,
            buffer: [0; WORD_LEN],
            buffer_len: 0,
            checksum: std::num::Wrapping(0),
        }
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
    fn finish(mut self) -> Ascii {
        if self.buffer_len > 0 {
            self.consume_final_word();
        }
        let Self{writer, checksum, buffer_len: 0, ..} = self else {
            unreachable!()
        };
        encode_base62(Int62::divrem(checksum.0).1)
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
    #[inline]
    fn consume_word(&mut self) {
        let word = self.take_word();
        self.checksum += word;
        let (_, encoded) = Self::encode_word(word);
        self.writer.write_array(encoded);
    }
    #[inline]
    fn consume_final_word(&mut self) {
        let word = self.take_word();
        self.checksum += word;
        let (start, encoded) = Self::encode_word(word);
        self.writer.write_slice(&encoded[start..]);
    }
    #[inline]
    fn take_word(&mut self) -> u32 {
        let word_bytes = std::mem::replace(&mut self.buffer, [0; WORD_LEN]);
        self.buffer_len = 0;
        u32::from_le_bytes(word_bytes)
    }
    #[inline]
    fn encode_word(mut word: u32) -> (usize, [Ascii; ENCODED_WORD_LEN]) {
        let (start, decomposed) =
            Int62::be_decompose::<ENCODED_WORD_LEN>(word);
        let mut result = [ascii::char!('0'); ENCODED_WORD_LEN];
        for (&x, c_mut) in std::iter::zip(
            &decomposed[start..],
            &mut result[start..],
        ) {
            *c_mut = encode_base62(x);
        }
        (start, result)
    }
}

