pub trait Read<B: Copy> {
    fn len(&self) -> usize;
    fn read_slice(&mut self, len: usize) -> Option<&[B]>;
    fn read_end_slice(&mut self, len: usize) -> Option<&[B]>;
    fn read_rest(&mut self) -> &[B];

    #[inline]
    fn read_array<const N: usize>(&mut self) -> Option<[B; N]> {
        let Ok(array) = <&[B; N]>::try_from(self.read_slice(N)?) else {
            unreachable!();
        };
        Some(*array)
    }

    #[inline]
    fn read_end_array<const N: usize>(&mut self) -> Option<[B; N]> {
        let Ok(array) = <&[B; N]>::try_from(self.read_end_slice(N)?) else {
            unreachable!();
        };
        Some(*array)
    }

    #[inline]
    fn read_byte(&mut self) -> Option<B> {
        self.read_array().map(|[b]| b)
    }

    #[inline]
    fn read_end_byte(&mut self) -> Option<B> {
        self.read_end_array().map(|[b]| b)
    }

}

impl<B: Copy> Read<B> for &[B] {
    #[inline]
    fn len(&self) -> usize {
        <[B]>::len(self)
    }
    #[inline]
    fn read_slice(&mut self, len: usize) -> Option<&[B]> {
        if self.len() < len { return None; }
        let (slice, rest) = self.split_at(len);
        *self = rest;
        Some(slice)
    }
    #[inline]
    fn read_end_slice(&mut self, len: usize) -> Option<&[B]> {
        if self.len() < len { return None; }
        let (rest, slice) = self.split_at(self.len() - len);
        *self = rest;
        Some(slice)
    }
    #[inline]
    fn read_rest(&mut self) -> &[B] {
        let rest = *self;
        *self = &self[self.len()..];
        rest
    }
}

impl<B: Copy, R> Read<B> for &mut R
where R: Read<B>
{
    #[inline]
    fn len(&self) -> usize
    { R::len(self) }
    #[inline]
    fn read_slice(&mut self, len: usize) -> Option<&[B]>
    { R::read_slice(self, len) }
    #[inline]
    fn read_end_slice(&mut self, len: usize) -> Option<&[B]>
    { R::read_end_slice(self, len) }
    #[inline]
    fn read_rest(&mut self) -> &[B]
    { R::read_rest(self) }
    #[inline]
    fn read_array<const N: usize>(&mut self) -> Option<[B; N]>
    { R::read_array(self) }
    #[inline]
    fn read_end_array<const N: usize>(&mut self) -> Option<[B; N]>
    { R::read_end_array(self) }
    #[inline]
    fn read_byte(&mut self) -> Option<B>
    { R::read_byte(self) }
}

pub trait Write<B: Copy> {
    fn write_slice(&mut self, value: &[B]);

    #[inline]
    fn write_array<const N: usize>(&mut self, value: [B; N]) {
        self.write_slice(&value)
    }

    #[inline]
    fn write_byte(&mut self, value: B) {
        self.write_array([value])
    }

}

impl<B: Copy> Write<B> for Vec<B> {
    #[inline]
    fn write_slice(&mut self, value: &[B]) {
        self.extend(value)
    }
}

impl<B: Copy, W> Write<B> for &mut W
where W: Write<B>
{
    #[inline]
    fn write_slice(&mut self, value: &[B])
    { W::write_slice(self, value) }
    #[inline]
    fn write_array<const N: usize>(&mut self, value: [B; N])
    { W::write_array(self, value) }
    #[inline]
    fn write_byte(&mut self, value: B)
    { W::write_byte(self, value) }
}

