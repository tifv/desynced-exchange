use flate2::write::ZlibEncoder as ZippingWriter;

use crate::{
    common::ConstSlice,
    byteseq::Write as _,
    ascii::{self, Ascii},
    intlim::{Int62, Int31, encode_base62, Base62Encode},
    Exchange,
};

pub(crate) fn compress(
    body: Exchange<&[u8]>,
) -> String {
    let (prefix, body) = match body {
        Exchange::Blueprint(body) => (ascii::str!("DSB"), body),
        Exchange::Behavior (body) => (ascii::str!("DSC"), body),
    };
    let mut writer = Vec::<Ascii>::with_capacity(128);
    writer.write_slice(prefix);
    let mut zipped = None;
    let (len, body) = {
        let zipped: &_ = zipped.insert(zip(body));
        if body.len() <= zipped.len() {
            (0, body)
        } else {
            (body.len(), zipped.as_ref())
        }
    };
    writer.write_slice(&encode_base31(len));
    let mut encoder = Base62Encode::new(writer, std::num::Wrapping(0));
    encoder.write_slice(body);
    #[allow(clippy::shadow_unrelated)]
    let (mut writer, checksum) = encoder.end();
    writer.write_byte(encode_base62(Int62::divrem(checksum.0).1));
    ascii::AsciiString(writer).into()
}

pub(super) fn encode_base31(len: usize) -> impl std::ops::Deref<Target=[Ascii]> {
    const MAX_DIGITS: usize = Int31::u32_sufficient_digits();
    if len == 0 {
        return ConstSlice::from_slice(&[ascii::char!('V')]);
    }
    let value: u32 = len.try_into()
        .expect("the len should not be that large");
    let mut result = ConstSlice::<MAX_DIGITS, Ascii>::new();
    let (mut index, digits) = Int31::u32_be_decompose::<MAX_DIGITS>(value);
    assert!(index < MAX_DIGITS);
    while index < MAX_DIGITS - 1 {
        result.push(encode_base62(Int62::from(digits[index])));
        index += 1;
    }
    result.push(encode_base62(digits[index].add_31()));
    result
}

fn zip(data: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    let mut zipper = ZippingWriter::new(
        Vec::<u8>::new(),
        flate2::Compression::best(),
    );
    zipper.write_all(data).unwrap();
    zipper.try_finish().unwrap();
    zipper.finish().unwrap()
}

