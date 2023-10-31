use std::io::Read;

use flate2::read::ZlibDecoder as UnZippingReader;

use crate::{
    Exchange,
    ascii::{self, Ascii, AsciiArray, AsciiStr},
    intlim::{Int62, Int31, decode_base62}, table::u32_to_usize,
};

use super::{
    error::Error,
    reader::{Reader, AsciiReader},
};

#[cold]
fn error_eof() -> Error {
    Error::from("unexpected end of data")
}

pub(crate) fn decompress(
    body: &str,
) -> Result<Exchange<Vec<u8>, Vec<u8>>, Error> {
    let mut body = AsciiReader::from_slice(<&AsciiStr>::try_from(body)?);
    let kind = match body.read_slice(3)
        .map(|s| <&AsciiStr>::from(s).into())
        .ok_or_else(error_eof)?
    {
        "DSB" => Exchange::Blueprint(()),
        "DSC" => Exchange::Behavior(()),
        _ => return Err(Error::from("unrecognized blueprint header")),
    };
    let encoded_len = read_len_base31(&mut body)?;
    let encoded_checksum = decode_base62(
        body.read_end_char()
            .ok_or_else(error_eof)?
    )?;
    let (body, checksum) = read_encoded(body)?;
    if checksum != encoded_checksum {
        return Err(Error::from("checksum does not match"));
    }
    let body: Vec<u8> = if encoded_len == 0 { body } else {
        let unzipped = unzip(&body)?;
        if encoded_len != unzipped.len() {
            return Err(Error::from("checksum does not match"));
        }
        unzipped
    };
    Ok(kind.map_mono(|()| body))
}

fn read_len_base31(reader: &mut AsciiReader) -> Result<usize, Error> {
    const MAX_DIGITS: usize = Int31::sufficient_digits();
    let mut digits = [Int31::zero(); MAX_DIGITS];
    let mut digits_mut: &mut [_] = &mut digits;
    loop {
        let Some(c) = reader.read_char() else {
            return Err(error_eof());
        };
        let x = decode_base62(c)?;
        let Some((next, rest)) = digits_mut.split_first_mut() else {
            return Err(Error::from("encoded length is too large"));
        };
        digits_mut = rest;
        match x.try_as_31() {
            Ok(x) => *next = x,
            Err(x) => {
                *next = x;
                break;
            }
        }
    }
    let end = MAX_DIGITS - digits_mut.len();
    Ok(u32_to_usize(
        Int31::be_compose(&digits[..end])
            .map_err(|_err| Error::from("encoded length is too large"))?
    ))
}

fn unzip(data: &[u8]) -> Result<Vec<u8>, Error> {
    use std::io::Write;
    let mut unzipper = UnZippingReader::new(
        data,
    );
    let mut result = Vec::new();
    unzipper.read_to_end(&mut result)?;
    Ok(result)
}

fn read_encoded(reader: AsciiReader<'_>) -> Result<(Vec<u8>, Int62), Error> {
    let mut decoder = Base62Decoder::new(reader);
    let mut result = Vec::new();
    let checksum = decoder.read_all(&mut result)?;
    Ok((result, checksum))
}

const WORD_LEN: usize = (u32::BITS / u8::BITS) as usize;
const ENCODED_WORD_LEN: usize = Int62::sufficient_digits();

struct Base62Decoder<'r> {
    reader: AsciiReader<'r>,
    checksum: std::num::Wrapping<u32>,
}

impl<'r> Base62Decoder<'r> {
    fn new(reader: AsciiReader<'r>) -> Self {
        Self{
            reader,
            checksum: std::num::Wrapping(0),
        }
    }
    fn read_all(mut self, dest: &mut Vec<u8>) -> Result<Int62, Error> {
        while let Some(word) = self.emit_word()? {
            dest.extend(word.to_le_bytes());
        }
        if !self.reader.is_empty() {
            dest.extend(self.emit_final_word()?.to_le_bytes());
        }
        let Self{checksum, ..} = self;
        Ok(Int62::divrem(checksum.0).1)
    }
    fn emit_word(&mut self) -> Result<Option<u32>, Error> {
        let Some(array) = self.reader.read_array::<ENCODED_WORD_LEN>() else {
            return Ok(None)
        };
        let word = Self::decode_word(&array)?;
        self.checksum += word;
        Ok(Some(word))
    }
    fn emit_final_word(&mut self) -> Result<u32, Error> {
        let slice = self.reader.read_rest();
        let word = Self::decode_word(slice)?;
        self.checksum += word;
        Ok(word)
    }
    fn decode_word(value: &[Ascii]) -> Result<u32, Error> {
        assert!(value.len() <= ENCODED_WORD_LEN);
        let start = ENCODED_WORD_LEN - value.len();
        let mut buffer = [Int62::zero(); ENCODED_WORD_LEN];
        for (&c, x_mut) in std::iter::zip(
            value,
            &mut buffer[start ..]
        ) {
            *x_mut = decode_base62(c)?;
        }
        Ok(Int62::be_compose(&buffer[start..])?)
    }
}

