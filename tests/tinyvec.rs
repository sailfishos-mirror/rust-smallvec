use {
    smallvec::SmallVec,
    std::{
        cell::Cell,
        cmp::Ordering,
        rc::Rc
    },
    tinyvec::{
        ArrayVec as TinyArrayVec,
        TinyVec
    }
};

/// Counts how many times it has been dropped, to catch leaks and double drops.
#[derive(Default)]
struct Tracked(Rc<Cell<usize>>);

impl Drop for Tracked {
    fn drop(&mut self) { self.0.set(self.0.get() + 1); }
}

fn tracked<const N: usize>(count: usize, drops: &Rc<Cell<usize>>) -> TinyVec<[Tracked; N]> {
    (0..count).map(|_| Tracked(drops.clone())).collect()
}

#[test]
fn from_tinyvec_stays_inline_or_spills() {
    let tiny: TinyVec<[i32; 8]> = [1, 2, 3].into_iter().collect();
    let small: SmallVec<i32, 4> = tiny.into();
    assert!(!small.spilled());
    assert_eq!(&small[..], &[1, 2, 3]);

    let tiny: TinyVec<[i32; 8]> = [1, 2, 3, 4, 5].into_iter().collect();
    let small: SmallVec<i32, 2> = tiny.into();
    assert!(small.spilled());
    assert_eq!(&small[..], &[1, 2, 3, 4, 5]);
}

#[test]
fn from_heap_tinyvec_adopts_or_repacks() {
    // A heap source that cannot fit inline adopts its buffer without copying.
    let tiny: TinyVec<[i32; 2]> = [1, 2, 3, 4, 5].into_iter().collect();
    assert!(tiny.is_heap());
    let small: SmallVec<i32, 4> = tiny.into();
    assert!(small.spilled());
    assert_eq!(&small[..], &[1, 2, 3, 4, 5]);

    // A heap source that fits inline is repacked into the stack buffer.
    let mut tiny: TinyVec<[i32; 2]> = [1, 2, 3].into_iter().collect();
    assert!(tiny.is_heap());
    tiny.truncate(2);
    let small: SmallVec<i32, 8> = tiny.into();
    assert!(!small.spilled());
    assert_eq!(&small[..], &[1, 2]);
}

#[test]
fn from_tinyarrayvec_stays_inline_or_spills() {
    let array: TinyArrayVec<[i32; 8]> = [1, 2, 3].into_iter().collect();
    let small: SmallVec<i32, 4> = array.into();
    assert!(!small.spilled());
    assert_eq!(&small[..], &[1, 2, 3]);

    let array: TinyArrayVec<[i32; 8]> = [1, 2, 3, 4, 5].into_iter().collect();
    let small: SmallVec<i32, 2> = array.into();
    assert!(small.spilled());
    assert_eq!(&small[..], &[1, 2, 3, 4, 5]);
}

#[test]
fn try_from_smallvec_fits_or_returns_original() {
    // Inline source, exactly filling the array.
    let tiny = TinyVec::<[i32; 4]>::try_from(SmallVec::<i32, 4>::from([1, 2, 3, 4])).unwrap();
    assert_eq!(&tiny[..], &[1, 2, 3, 4]);
    // Spilled source.
    let tiny = TinyVec::<[i32; 8]>::try_from(SmallVec::<i32, 2>::from([1, 2, 3, 4, 5])).unwrap();
    assert_eq!(&tiny[..], &[1, 2, 3, 4, 5]);

    // One element too many: the original comes back untouched.
    let error = TinyVec::<[i32; 3]>::try_from(SmallVec::<i32, 4>::from([1, 2, 3, 4])).unwrap_err();
    assert!(!error.spilled());
    assert_eq!(&error[..], &[1, 2, 3, 4]);
    let error =
        TinyVec::<[i32; 4]>::try_from(SmallVec::<i32, 2>::from([1, 2, 3, 4, 5])).unwrap_err();
    assert!(error.spilled());
    assert_eq!(&error[..], &[1, 2, 3, 4, 5]);
}

#[test]
fn try_from_smallvec_to_tinyarrayvec_fits_or_returns_original() {
    let array = TinyArrayVec::<[i32; 4]>::try_from(SmallVec::<i32, 4>::from([1, 2, 3, 4])).unwrap();
    assert_eq!(&array[..], &[1, 2, 3, 4]);

    let error =
        TinyArrayVec::<[i32; 3]>::try_from(SmallVec::<i32, 4>::from([1, 2, 3, 4])).unwrap_err();
    assert!(!error.spilled());
    assert_eq!(&error[..], &[1, 2, 3, 4]);
}

#[test]
fn conversions_drop_each_element_exactly_once() {
    // `From`, both inline (3 <= 4) and spilled (6 > 4).
    for count in [3, 6] {
        let drops = Rc::new(Cell::new(0));
        let small: SmallVec<Tracked, 4> = tracked::<8>(count, &drops).into();
        assert_eq!(small.spilled(), count > 4);
        assert_eq!(drops.get(), 0, "conversion must not drop elements");
        drop(small);
        assert_eq!(drops.get(), count);
    }

    // `TryFrom` success, both inline and spilled.
    for count in [3, 6] {
        let drops = Rc::new(Cell::new(0));
        let small: SmallVec<Tracked, 4> = tracked::<8>(count, &drops).into();
        let tiny = TinyVec::<[Tracked; 8]>::try_from(small).ok().unwrap();
        assert_eq!(drops.get(), 0, "conversion must not drop elements");
        drop(tiny);
        assert_eq!(drops.get(), count);
    }
    for count in [3, 6] {
        let drops = Rc::new(Cell::new(0));
        let small: SmallVec<Tracked, 4> = tracked::<8>(count, &drops).into();
        let array = TinyArrayVec::<[Tracked; 8]>::try_from(small).ok().unwrap();
        assert_eq!(drops.get(), 0, "conversion must not drop elements");
        drop(array);
        assert_eq!(drops.get(), count);
    }

    // `TryFrom` failure hands every element back.
    let drops = Rc::new(Cell::new(0));
    let small: SmallVec<Tracked, 2> = tracked::<8>(5, &drops).into();
    let error = TinyVec::<[Tracked; 3]>::try_from(small).err().unwrap();
    assert_eq!(drops.get(), 0, "failed conversion must not drop elements");
    drop(error);
    assert_eq!(drops.get(), 5);
}

#[test]
fn zero_sized_items() {
    let tiny: TinyVec<[(); 8]> = std::iter::repeat_n((), 5).collect();
    let small: SmallVec<(), 2> = tiny.into();
    assert_eq!(small.len(), 5);

    assert!(TinyArrayVec::<[(); 4]>::try_from(small.clone()).is_err());
    assert_eq!(
        TinyArrayVec::<[(); 5]>::try_from(small.clone())
            .unwrap()
            .len(),
        5
    );
    assert_eq!(TinyVec::<[(); 5]>::try_from(small).unwrap().len(), 5);
}

#[test]
fn eq_across_capacities_and_item_types() {
    let small = SmallVec::<i32, 2>::from([1, 2, 3]);
    let tiny: TinyVec<[i32; 8]> = [1, 2, 3].into_iter().collect();
    let array: TinyArrayVec<[i32; 8]> = [1, 2, 3].into_iter().collect();
    assert!(small == tiny);
    assert!(tiny == small);
    assert!(small == array);
    assert!(array == small);
    assert!(small != TinyVec::<[i32; 8]>::from_iter([1, 2, 4]));
    assert!(small != TinyArrayVec::<[i32; 8]>::from_iter([1, 2, 4]));
    assert!(tiny != SmallVec::<i32, 2>::from([1, 2]));

    // `Item: PartialEq<Other>` with different types.
    let small: SmallVec<&str, 2> = SmallVec::from(["a", "b"]);
    let tiny: TinyVec<[String; 4]> = ["a".to_string(), "b".to_string()].into_iter().collect();
    let array: TinyArrayVec<[String; 4]> = ["a".to_string(), "b".to_string()].into_iter().collect();
    assert!(small == tiny);
    assert!(tiny == small);
    assert!(small == array);
    assert!(array == small);
}

#[test]
fn ordering_in_both_directions() {
    let small = SmallVec::<i32, 2>::from([1, 2, 3]);
    let tiny: TinyVec<[i32; 8]> = [1, 2, 4].into_iter().collect();
    let array: TinyArrayVec<[i32; 8]> = [1, 2, 4].into_iter().collect();
    assert!(small < tiny);
    assert!(tiny > small);
    assert_eq!(small.partial_cmp(&tiny), Some(Ordering::Less));
    assert_eq!(tiny.partial_cmp(&small), Some(Ordering::Greater));
    assert!(small < array);
    assert!(array > small);
    assert_eq!(small.partial_cmp(&array), Some(Ordering::Less));
    assert_eq!(array.partial_cmp(&small), Some(Ordering::Greater));
}
