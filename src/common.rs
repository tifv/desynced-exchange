use std::mem::MaybeUninit;


#[must_use]
#[inline]
pub const fn u32_to_usize(len: u32) -> usize {
    assert!({ const OK: bool = {
        let ok = u32::BITS <= usize::BITS;
        assert!(ok); ok
    }; OK});
    len as usize
}

pub type LogSize = u8;

#[must_use]
#[inline]
pub const fn iexp2(loglen: Option<LogSize>) -> u32 {
    let Some(loglen) = loglen else { return 0 };
    match 1_u32.checked_shl(loglen as u32) {
        Some(exp) if exp - 1 <= (i32::MAX as u32) => exp,
        _ => panic!( "differences between indices \
            should be expressable by i32" ),
    }
}

#[must_use]
#[inline]
pub const fn ilog2_ceil(len: usize) -> Option<LogSize> {
    //! Upper-rounded base 2 logarithm.
    //! Returns `None` if `len` is zero.
    let Some(mut ilog2) = len.checked_ilog2() else {
        return None;
    };
    if ilog2 > len.trailing_zeros() {
        ilog2 += 1;
    }
    Some(ilog2 as u8)
}

#[must_use]
#[inline]
pub const fn ilog2_exact(len: usize) -> Option<LogSize> {
    //! Base 2 logarithm.
    //! Returns `None` if `len` is not a power of two.
    let Some(ilog2) = len.checked_ilog2() else {
        return None;
    };
    if ilog2 > len.trailing_zeros() {
        return None;
    }
    Some(ilog2 as u8)
}


/// # Safety
/// The implementer must ensure that
/// `Self` is `repr(transparent)` over `Self::Target` and
/// there are no additional safety-bearing invariants for
/// the contained value.
pub(crate) unsafe trait TransparentRef : AsRef<Self::Target> + Sized {
    type Target : Sized;
    #[must_use]
    fn from_ref(target: &Self::Target) -> &Self {
        // SAFETY: `Self` is `repr(transparent)` over `Target`
        unsafe { &*((target as *const Self::Target).cast::<Self>()) }
    }
    #[must_use]
    fn into_inner(self) -> Self::Target {
        let this = std::ptr::addr_of!(self);
        std::mem::forget(self);
        // SAFETY: `Self` is `repr(transparent)` over `Target`
        unsafe { this.cast::<Self::Target>().read() }
    }
}


pub(crate) fn map_result<const N: usize, T, V, F, E>(array: [T; N], mut f: F)
-> Result<[V; N], E>
where
    V: Default,
    F: FnMut(T) -> Result<V, E>
{
    let mut err = None;
    let array = array.map(|x| match f(x) {
        Ok(y) => y,
        Err(e) => { err = Some(e); V::default() },
    });
    if let Some(err) = err {
        return Err(err);
    }
    Ok(array)
}


pub(crate) struct ConstSlice<const N: usize, T> {
    // SAFETY-BEARING invariant:
    // `array[start .. end]` is initialized.
    start: usize, end: usize,
    array: [MaybeUninit<T>; N],
}

impl<const N: usize, T: Clone> Clone for ConstSlice<N, T> {
    fn clone(&self) -> Self {
        let &Self { start, end, ref array } = self;
        if !(start <= end && end <= N) {
            // SAFETY: struct invariant
            unsafe { std::hint::unreachable_unchecked() }
        }
        let mut cloned_array = Self::new_array();
        for i in start .. end {
            // SAFETY: struct invariant
            cloned_array[i].write(unsafe {
                array[i].assume_init_ref().clone()
            });
        }
        Self { start, end, array: cloned_array }
    }
}

impl<const N: usize, T> std::ops::Deref for ConstSlice<N, T> {
    type Target = [T];
    #[inline]
    fn deref(&self) -> &Self::Target {
        let &Self { start, end, ref array } = self;
        // SAFETY: struct invariant
        unsafe { &*(
            std::ptr::addr_of!(array[start .. end])
            as *const [T]
        ) }
    }
}

impl<const N: usize, T> Drop for ConstSlice<N, T> {
    fn drop(&mut self) {
        let &mut Self { start, end, ref mut array } = self;
        for value in &mut array[start .. end] {
            // SAFETY: struct invariant
            unsafe { value.assume_init_drop() }
        }
    }
}

impl<const N: usize, T> ConstSlice<N, T> {
    #[inline]
    fn new_array() -> [MaybeUninit<T>; N] {
        [(); N].map(|()| MaybeUninit::uninit())
    }
    #[inline]
    pub(crate) fn new() -> Self {
        Self { start: 0, end: 0, array: Self::new_array() }
    }
    pub(crate) fn from_slice(slice: &[T]) -> Self
    where T: Clone
    {
        let len = slice.len();
        assert!(len <= N);
        let mut this = Self::new();
        for (i, v) in slice.iter().enumerate() {
            this.array[i].write(v.clone());
        }
        this.end = len;
        this
    }
    #[inline]
    pub(crate) fn push(&mut self, value: T) {
        let end = self.end;
        assert!(end < N);
        self.array[end].write(value);
        self.end = end + 1;
    }
}

