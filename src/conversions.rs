#[cfg(feature = "specialization")]
use super::specialization::SpecFromSlice;
#[cfg(feature = "arrayvec")]
use arrayvec::ArrayVec;
#[cfg(feature = "tinyvec")]
use tinyvec::{
    Array,
    ArrayVec as TinyArrayVec,
    TinyVec
};
use {
    super::{
        SmallVec,
        locatedlength::LocatedLength,
        rawsmallvec::RawSmallVec
    },
    alloc::{
        alloc::Global,
        borrow::Cow,
        boxed::Box,
        vec::Vec
    },
    core::{
        alloc::Allocator,
        mem::ManuallyDrop,
        ptr::{
            NonNull,
            copy_nonoverlapping
        }
    }
};

impl<Item: Clone, const INLINE: usize> From<&[Item]> for SmallVec<Item, INLINE, Global> {
    #[inline]
    fn from(slice: &[Item]) -> Self {
        if slice.len() <= Self::inline_size() {
            // SAFETY: The precondition is checked in the initial comparison
            // above.
            unsafe {
                #[cfg(feature = "specialization")]
                {
                    <Self as SpecFromSlice<Item>>::spec_from(slice)
                }

                #[cfg(not(feature = "specialization"))]
                {
                    Self::from_slice_fallback(slice)
                }
            }
        } else {
            Vec::from(slice).into()
        }
    }
}

impl<Item: Clone, const INLINE: usize> From<&mut [Item]> for SmallVec<Item, INLINE, Global> {
    #[inline]
    fn from(slice: &mut [Item]) -> Self { From::<&[Item]>::from(slice) }
}

impl<Item: Clone, const INLINE: usize, const LENGTH: usize> From<&[Item; LENGTH]>
    for SmallVec<Item, INLINE, Global>
{
    #[inline]
    fn from(slice: &[Item; LENGTH]) -> Self { From::<&[Item]>::from(slice) }
}

impl<Item: Clone, const INLINE: usize, const LENGTH: usize> From<&mut [Item; LENGTH]>
    for SmallVec<Item, INLINE, Global>
{
    #[inline]
    fn from(slice: &mut [Item; LENGTH]) -> Self { From::<&[Item]>::from(slice) }
}

impl<Item, const INLINE: usize, const LENGTH: usize> From<[Item; LENGTH]>
    for SmallVec<Item, INLINE, Global>
{
    fn from(array: [Item; LENGTH]) -> Self {
        if const { LENGTH <= INLINE } {
            let mut smallvec = Self::new();
            // SAFETY: INLINE <= smallvec.capacity()
            unsafe {
                copy_nonoverlapping(
                    ManuallyDrop::new(array).as_ptr(),
                    smallvec.raw.as_mut_ptr_inline(),
                    LENGTH
                );
                smallvec.set_len(LENGTH);
            }
            smallvec
        } else {
            Vec::from(array).into()
        }
    }
}

impl<Item, const INLINE: usize, const LENGTH: usize, Heap: Allocator>
    TryFrom<SmallVec<Item, INLINE, Heap>> for [Item; LENGTH]
{
    type Error = SmallVec<Item, INLINE, Heap>;

    #[inline]
    fn try_from(
        mut smallvec: SmallVec<Item, INLINE, Heap>
    ) -> Result<[Item; LENGTH], SmallVec<Item, INLINE, Heap>> {
        if smallvec.len() == LENGTH {
            // SAFETY: we move the elements out of the smallvec, none are left
            // in it
            unsafe {
                smallvec.set_len(0);
            }
            Ok(unsafe { smallvec.as_ptr().cast::<[Item; LENGTH]>().read() })
        } else {
            Err(smallvec)
        }
    }
}

#[cfg(feature = "arrayvec")]
impl<Item, const INLINE: usize, const LENGTH: usize> From<ArrayVec<Item, LENGTH>>
    for SmallVec<Item, INLINE, Global>
{
    fn from(mut array: ArrayVec<Item, LENGTH>) -> Self {
        let length = array.len();
        let mut smallvec = Self::with_capacity(length);
        unsafe {
            copy_nonoverlapping(array.as_ptr(), smallvec.as_mut_ptr(), length);
            array.set_len(0);
            smallvec.set_len(length);
        }
        smallvec
    }
}
#[cfg(feature = "arrayvec")]
impl<Item, const INLINE: usize, const LENGTH: usize, Heap: Allocator>
    TryFrom<SmallVec<Item, INLINE, Heap>> for ArrayVec<Item, LENGTH>
{
    type Error = SmallVec<Item, INLINE, Heap>;

    fn try_from(
        mut smallvec: SmallVec<Item, INLINE, Heap>
    ) -> Result<ArrayVec<Item, LENGTH>, SmallVec<Item, INLINE, Heap>> {
        let length = smallvec.len();
        if length > LENGTH {
            return Err(smallvec);
        }
        let mut array = ArrayVec::new();
        unsafe {
            copy_nonoverlapping(smallvec.as_ptr(), array.as_mut_ptr(), length);
            smallvec.set_len(0);
            array.set_len(length);
        }
        Ok(array)
    }
}

#[cfg(feature = "tinyvec")]
impl<A: Array, const INLINE: usize> From<TinyVec<A>> for SmallVec<A::Item, INLINE, Global> {
    fn from(tiny: TinyVec<A>) -> Self {
        if tiny.is_heap() && tiny.len() > Self::inline_size() {
            // The source already owns a heap buffer big enough for a spilled
            // `SmallVec`, so it can be adopted without copying. A heap buffer
            // that still fits inline is repacked into the stack buffer below.
            tiny.into_vec().into()
        } else {
            let mut smallvec = Self::with_capacity(tiny.len());
            smallvec.extend(tiny);
            smallvec
        }
    }
}
#[cfg(feature = "tinyvec")]
impl<A: Array, const INLINE: usize, Heap: Allocator> TryFrom<SmallVec<A::Item, INLINE, Heap>>
    for TinyVec<A>
{
    type Error = SmallVec<A::Item, INLINE, Heap>;

    fn try_from(
        smallvec: SmallVec<A::Item, INLINE, Heap>
    ) -> Result<TinyVec<A>, SmallVec<A::Item, INLINE, Heap>> {
        if smallvec.len() > A::CAPACITY {
            return Err(smallvec);
        }
        let mut tiny = TinyVec::new();
        tiny.extend(smallvec);
        Ok(tiny)
    }
}
#[cfg(feature = "tinyvec")]
impl<A: Array, const INLINE: usize> From<TinyArrayVec<A>> for SmallVec<A::Item, INLINE, Global> {
    fn from(array: TinyArrayVec<A>) -> Self {
        let mut smallvec = Self::with_capacity(array.len());
        smallvec.extend(array);
        smallvec
    }
}
#[cfg(feature = "tinyvec")]
impl<A: Array, const INLINE: usize, Heap: Allocator> TryFrom<SmallVec<A::Item, INLINE, Heap>>
    for TinyArrayVec<A>
{
    type Error = SmallVec<A::Item, INLINE, Heap>;

    fn try_from(
        smallvec: SmallVec<A::Item, INLINE, Heap>
    ) -> Result<TinyArrayVec<A>, SmallVec<A::Item, INLINE, Heap>> {
        if smallvec.len() > A::CAPACITY {
            return Err(smallvec);
        }
        let mut array = TinyArrayVec::new();
        array.extend(smallvec);
        Ok(array)
    }
}

impl<Item, const INLINE: usize> From<Vec<Item>> for SmallVec<Item, INLINE> {
    fn from(array: Vec<Item>) -> Self {
        if array.capacity() == 0 {
            return Self::new();
        }

        if Self::IS_ZST {
            // "Move" elements to stack buffer. They're ZST so we don't actually
            // have to do anything. Just make sure they're not
            // dropped. We don't wrap the vector in ManuallyDrop so
            // that when it's dropped, the memory is deallocated, if
            // it needs to be.
            let mut vec = array;
            let length = vec.len();

            // SAFETY: `0` is less than the vector's capacity.
            // old_len..new_len is an empty range. So there are no uninitialized
            // elements
            unsafe { vec.set_len(0) };
            Self {
                length: LocatedLength::new(length, false),
                raw: RawSmallVec::new(),
                allocator: Global
            }
        } else {
            let mut vec = ManuallyDrop::new(array);
            let length = vec.len();

            // A heap-allocated `SmallVec` must always observe the invariant
            // that `capacity > INLINE`.
            if vec.capacity() <= INLINE {
                vec.reserve(INLINE + 1 - length);
            }
            // SAFETY: vec.capacity is not `0` (checked above), so the pointer
            // can not dangle and thus specifically cannot be null.
            Self {
                length: LocatedLength::new(length, true),
                raw: RawSmallVec {
                    heap: NonNull::slice_from_raw_parts(
                        unsafe { NonNull::new_unchecked(vec.as_mut_ptr()).cast() },
                        vec.capacity()
                    )
                },
                allocator: Global
            }
        }
    }
}

impl<Item, const INLINE: usize, Heap: Allocator> From<SmallVec<Item, INLINE, Heap>> for Vec<Item> {
    fn from(this: SmallVec<Item, INLINE, Heap>) -> Self {
        let (length, spilled) = this.length.parts();
        if !spilled {
            let mut vec = Vec::with_capacity(length);
            let this = ManuallyDrop::new(this);
            // SAFETY: we create a new vector with sufficient capacity, copy our
            // elements into it to transfer ownership and then set
            // the length we don't drop the elements we previously
            // held
            unsafe {
                copy_nonoverlapping(this.raw.as_ptr_inline(), vec.as_mut_ptr(), length);
                vec.set_len(length);
            }
            vec
        } else {
            let this = ManuallyDrop::new(this);
            // SAFETY:
            // - `pointer` was created with the SmallVec's allocator
            // - `pointer` was created with the appropriate alignment for `Item`
            // - the allocation pointed to by pointer is exactly capacity *
            //   sizeof(Item)
            // - `length` is less than or equal to `capacity`
            // - the first `length` entries are proper `Item`-values
            // - the allocation is not larger than `isize::MAX`
            unsafe {
                let pointer = this.raw.heap;
                Vec::from_raw_parts(pointer.cast().as_ptr(), length, pointer.len())
            }
        }
    }
}

impl<Item, const INLINE: usize, Heap: Allocator> From<SmallVec<Item, INLINE, Heap>>
    for Box<[Item]>
{
    fn from(this: SmallVec<Item, INLINE, Heap>) -> Self { Vec::from(this).into_boxed_slice() }
}

impl<T, const N: usize> From<Box<[T]>> for SmallVec<T, N> {
    fn from(boxed: Box<[T]>) -> Self { boxed.into_vec().into() }
}

impl<Item, const INLINE: usize> From<SmallVec<Item, INLINE, Global>>
    for (LocatedLength<Item>, RawSmallVec<Item, INLINE>)
{
    #[inline]
    fn from(value: SmallVec<Item, INLINE, Global>) -> Self {
        let (length, raw, Global) = value.into();
        (length, raw)
    }
}

impl<Item, const INLINE: usize, Heap: Allocator> From<SmallVec<Item, INLINE, Heap>>
    for (LocatedLength<Item>, RawSmallVec<Item, INLINE>, Heap)
{
    #[inline]
    fn from(value: SmallVec<Item, INLINE, Heap>) -> Self {
        let this = ManuallyDrop::new(value);
        // SAFETY: `this` is never dropped or used again, so each field is moved
        // out exactly once and `Drop for SmallVec` never runs.
        (
            this.length,
            unsafe { (&raw const this.raw).read() },
            unsafe { (&raw const this.allocator).read() }
        )
    }
}

impl<Item: Clone, const INLINE: usize> From<Cow<'_, [Item]>> for SmallVec<Item, INLINE, Global> {
    #[inline]
    fn from(value: Cow<'_, [Item]>) -> Self {
        match value {
            Cow::Owned(mut vector) => unsafe {
                match NonNull::new(vector.as_mut_ptr()) {
                    Some(pointer) => Self::parts(
                        LocatedLength::new(vector.len(), true),
                        RawSmallVec {
                            heap: NonNull::slice_from_raw_parts(pointer.cast(), vector.capacity())
                        }
                    ),
                    None => Self::parts(LocatedLength::new(vector.len(), false), RawSmallVec::new())
                }
            },
            Cow::Borrowed(slice) => slice.into()
        }
    }
}
impl<Item: Clone, const INLINE: usize> From<SmallVec<Item, INLINE, Global>> for Cow<'_, [Item]> {
    #[inline]
    fn from(value: SmallVec<Item, INLINE, Global>) -> Self { Cow::Owned(value.into()) }
}
