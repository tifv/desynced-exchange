use std::io::Write;

use flate2::write::ZlibEncoder as ZippingWriter;

use super::{
    Error,
    ExchangeKind,
};

pub(super) struct Compressor<W: std::fmt::Write> {
    write: W,
    body: Vec<u8>,
}

impl<W: std::fmt::Write> Compressor<W> {
    pub(super) fn new(
        mut write: W, kind: ExchangeKind,
    ) -> Result<Self, Error> {
        write.write_str(match kind {
            ExchangeKind::Blueprint(()) => "DSB",
            ExchangeKind::Behavior(()) => "DSC",
        })?;
        Ok(Self{
            write,
            body: Vec::new(),
        })
    }
    pub(super) fn content_writer(&mut self) -> impl std::io::Write + '_ {
        &mut self.body
    }
    pub(super) fn write_len_base31(
        write: &mut W, len: usize,
    ) -> Result<(), Error> {
        if len == 0 {
            return Ok(write.write_char('V')?);
        }
        let mut value: u32 = len.try_into()?;
        while value > 0 {
            let (next_value, index) = base62::divrem31(value);
            value = next_value;
            write.write_char(if value > 0 {
                base62::encode_letter(index.into())
            } else {
                base62::encode_letter(index.add31())
            }.into())?;
        }
        Ok(())
    }
    pub(super) fn finish(self) -> Result<W, Error> {
        let Self{mut write, body} = self;
        let (len, body) = if body.len() <= 32 { (0, body) } else {
            let len = body.len();
            (len, compress(&body)?)
        };
        Self::write_len_base31(&mut write, len)?;
        let (body, checksum) = Base62Encoder::encode(&body)?;
        write.write_str(&body)?;
        write.write_char(checksum)?;
        Ok(write)
    }
}

mod base62 {

    // Invariant: the value in the struct cannot exceed 61
    #[repr(transparent)]
    pub(super) struct Int62(u8);

    pub(super) fn divrem(value: u32) -> (u32, Int62) {
        let rem = value % 62;
        let div = value /  62;
        (div, Int62(rem as u8))
    }

    #[inline]
    pub(super) fn encode_letter(value: Int62) -> u8 {
        let value = value.0;
        match value {
             0 ..=  9 => b'0' +  value      ,
            10 ..= 35 => b'A' + (value - 10),
            36 ..= 61 => b'a' + (value - 36),
            // SAFETY: `Int62` type invariant
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
    }

    // Invariant: the value in the struct cannot exceed 61
    #[repr(transparent)]
    pub(super) struct Int31(u8);

    pub(super) fn divrem31(value: u32) -> (u32, Int31) {
        let rem = value % 31;
        let div = value / 31;
        (div, Int31(rem as u8))
    }

    impl Int31 {
        pub(super) fn add31(self) -> Int62 {
            Int62(self.0 + 31)
        }
    }

    impl From<Int31> for Int62 {
        fn from(value: Int31) -> Self {
            Self(value.0)
        }
    }

}

fn compress(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut zipper = ZippingWriter::new(
        Vec::<u8>::new(),
        flate2::Compression::best(),
    );
    zipper.write_all(data)?;
    zipper.try_finish()?;
    Ok(zipper.finish()?)
}

fn fmt_error_to_io(error: std::fmt::Error) -> std::io::Error {
    std::io::Error::from(std::io::ErrorKind::Other)
}

const WORD_LEN: usize = 4;
const ENCODED_WORD_LEN: usize = 6;

struct Base62Encoder<W: std::fmt::Write> {
    write: W,
    buffer: [u8; WORD_LEN],
    buffer_len: u8,
    checksum: std::num::Wrapping<u32>,
}
// struct invariant: `buffer_len` is always less than `WORD_LEN`

impl<W: std::fmt::Write> Base62Encoder<W> {
    fn new(write: W) -> Self {
        Self{
            write,
            buffer: [0; WORD_LEN],
            buffer_len: 0,
            checksum: std::num::Wrapping(0),
        }
    }
    fn finish(mut self) -> Result<(W, char), std::io::Error> {
        if self.buffer_len > 0 {
            self.write_final_word()?;
        }
        let Self{write, checksum, ..} = self;
        let checksum = base62::encode_letter(
            base62::divrem(checksum.0).1 ).into();
        Ok((write, checksum))
    }
    #[inline]
    fn encode_word(mut word: u32) -> ([u8; ENCODED_WORD_LEN], u8) {
        assert!({ const OK: bool = {
            let ok = 62_u32.checked_pow(6).is_none();
            assert!(ok); ok
        }; OK});
        let mut result = [b'0'; ENCODED_WORD_LEN];
        let mut start = ENCODED_WORD_LEN;
        while word > 0 {
            if start == 0 {
                // SAFETY: `62.pow(start) > word` is an invariant
                unsafe { std::hint::unreachable_unchecked(); }
            }
            let (word_div, rem) = base62::divrem(word);
            word = word_div;
            start -= 1;
            result[start] = base62::encode_letter(rem);
        }
        if start >= ENCODED_WORD_LEN {
            // SAFETY: `start` only decreases
            unsafe { std::hint::unreachable_unchecked(); }
        }
        (result, start as u8)
    }
    #[inline]
    fn take_word(&mut self) -> u32 {
        let word_bytes = std::mem::replace(&mut self.buffer, [0; WORD_LEN]);
        self.buffer_len = 0;
        u32::from_le_bytes(word_bytes)
    }
    #[inline]
    fn write_word(&mut self) -> Result<(), std::io::Error> {
        let word = self.take_word();
        self.checksum += word;
        let (encoded, _) = Self::encode_word(word);
        // SAFETY: `Self::encode_word` produces only ASCII
        self.write.write_str( unsafe {
            std::str::from_utf8_unchecked(&encoded)
        } ).map_err(fmt_error_to_io)?;
        Ok(())
    }
    #[inline]
    fn write_final_word(&mut self) -> Result<(), std::io::Error> {
        let word = self.take_word();
        self.checksum += word;
        let (encoded, start) = Self::encode_word(word);
        let start = start as usize;
        // SAFETY: `Self::encode_word` produces only ASCII
        self.write.write_str( unsafe {
            std::str::from_utf8_unchecked(&encoded[start ..])
        } ).map_err(fmt_error_to_io)?;
        Ok(())
    }
}

impl Base62Encoder<String> {
    fn encode(data: &[u8]) -> Result<(String, char), std::io::Error> {
        let mut encoder = Self::new(String::new());
        encoder.write_all(data)?;
        encoder.finish()
    }
}

impl<W: std::fmt::Write> std::io::Write for Base62Encoder<W> {

    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = self.buffer_len as usize;
        if len >= WORD_LEN {
            // SAFETY: `Self` struct invariant
            unsafe { std::hint::unreachable_unchecked() }
        }
        let mut write_buffer = &mut self.buffer[len .. WORD_LEN];
        let result = std::io::Write::write(&mut write_buffer, buf);
        let remaining_len = write_buffer.len();
        match remaining_len {
            0 => self.write_word()?,
            1 ..= WORD_LEN =>
                self.buffer_len = (WORD_LEN - remaining_len) as u8,
            // SAFETY: `remaining_len` can only decrease from `WORD_LEN`
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
        result
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }

}

