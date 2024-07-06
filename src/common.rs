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

#[inline]
pub(crate) unsafe fn ptr_sub<T>(more: *const T, less: *const T) -> usize {
    //! # Safety
    //! The caller guarantees that
    //! * `less <= more`;
    //! * both pointers are contained within the same allocated object;
    //! * distance is exact multiple of `T`;
    //!
    //! …and other safety requirements of `offset_from` method.
    // SAFETY: ensured by the caller
    let diff = unsafe { more.offset_from(less) };
    if diff < 0 {
        // SAFETY: ensured by the caller
        unsafe { std::hint::unreachable_unchecked() }
    }
    diff as usize
}

/// # Safety
/// The implementer must ensure that
/// `Self` is `repr(transparent)` over `Self::Target` and
/// there are no additional safety-bearing invariants for
/// the contained value.
pub(crate) unsafe trait TransparentRef : AsRef<Self::Target> + Sized {
    type Target : Sized;
    fn from_ref(target: &Self::Target) -> &Self {
        // SAFETY: `Self` is `repr(transparent)` over `Target`
        unsafe { &*((target as *const Self::Target).cast::<Self>()) }
    }
    fn unwrap(self) -> Self::Target {
        let this = std::ptr::addr_of!(self);
        std::mem::forget(self);
        // SAFETY: `Self` is `repr(transparent)` over `Target`
        unsafe { this.cast::<Self::Target>().read() }
    }
}

