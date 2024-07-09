use flate2::read::ZlibDecoder as UnZippingReader;

use crate::{
    error::LoadError as Error,
    common::{
        ascii::{Ascii, AsciiStr},
        byteseq::Read,
        intlim::{Int62, Int31, decode_base62, Base62Decode},
    },
    Exchange,
};

#[cold]
fn error_eof() -> Error {
    Error::from("unexpected end of data")
}

pub(crate) fn decompress(
    body: &str,
) -> Result<Exchange<Vec<u8>>, Error> {
    let mut body: &[Ascii] = <&AsciiStr>::try_from(body)?;
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
        body.read_end_byte()
            .ok_or_else(error_eof)?
    )?;
    let mut decoder = Base62Decode::new(Vec::new(), std::num::Wrapping(0));
    decoder.write_slice(body)?;
    #[allow(clippy::shadow_unrelated)]
    let (body, checksum) = decoder.end()?;
    if Int62::divrem(checksum.0).1 != encoded_checksum {
        return Err(Error::from("checksum does not match"));
    }
    let body: Vec<u8> = if encoded_len == 0 { body } else {
        let unzipped = unzip(&body)?;
        if encoded_len != unzipped.len() {
            return Err(Error::from("length does not match"));
        }
        unzipped
    };
    Ok(kind.map_mono(|()| body))
}

fn read_len_base31(mut reader: impl Read<Ascii>) -> Result<usize, Error> {
    const MAX_DIGITS: usize = Int31::u32_sufficient_digits();
    let mut digits = [Int31::zero(); MAX_DIGITS];
    let mut digits_mut: &mut [_] = &mut digits;
    loop {
        let Some(c) = reader.read_byte() else {
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
    Ok( Int31::u32_be_compose(&digits[..end])
            .map_err(|_err| Error::from("encoded length is too large"))?
        as usize )
}

fn unzip(data: &[u8]) -> Result<Vec<u8>, Error> {
    use std::io::Read as _;
    let mut unzipper = UnZippingReader::new(
        data,
    );
    let mut result = Vec::new();
    unzipper.read_to_end(&mut result)?;
    Ok(result)
}

