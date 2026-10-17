use core::ptr::NonNull;
use petr_core::{
    Key, KeyedTaggedNonNull, LowTagLayout, PortHint, ProphecyBits, ResidualClass, ResidualRouter,
    SelectorWord, SplitTaggedNonNull, TagError, TaggedNonNull, ValueWord,
};
use proptest::prelude::any;
use proptest::*;

#[repr(align(64))]
#[derive(Debug, PartialEq, Eq)]
struct Aligned64(u64);

#[test]
fn low_tag_layout_masks_and_states_are_exact_for_small_widths() {
    assert_eq!(LowTagLayout::<0>::ALIGN, 1);
    assert_eq!(LowTagLayout::<0>::LOW_MASK, 0b0);
    assert_eq!(LowTagLayout::<0>::CLEAN_MASK, !0usize);
    assert_eq!(LowTagLayout::<0>::TAG_STATES, 1);

    assert_eq!(LowTagLayout::<1>::ALIGN, 2);
    assert_eq!(LowTagLayout::<1>::LOW_MASK, 0b1);
    assert_eq!(LowTagLayout::<1>::TAG_STATES, 2);

    assert_eq!(LowTagLayout::<3>::ALIGN, 8);
    assert_eq!(LowTagLayout::<3>::LOW_MASK, 0b111);
    assert_eq!(LowTagLayout::<3>::TAG_STATES, 8);

    assert_eq!(LowTagLayout::<6>::ALIGN, 64);
    assert_eq!(LowTagLayout::<6>::LOW_MASK, 0b11_1111);
    assert_eq!(LowTagLayout::<6>::TAG_STATES, 64);
}

#[test]
fn low_tag_validation_accepts_exact_range_and_rejects_overflow() {
    for tag in 0..=LowTagLayout::<4>::LOW_MASK {
        LowTagLayout::<4>::validate_tag(tag).unwrap();
    }

    assert_eq!(
        LowTagLayout::<4>::validate_tag(0b1_0000).unwrap_err(),
        TagError::TagTooLarge {
            tag: 0b1_0000,
            mask: 0b1111
        }
    );

    assert_eq!(
        LowTagLayout::<13>::validate_layout().unwrap_err(),
        TagError::LowBitsTooLarge { bits: 13, max: 12 }
    );
}

#[test]
fn clean_word_and_low_tag_partition_the_word_without_overlap() {
    const WORDS: &[usize] = &[0x0, 0x1, 0x7, 0x8, 0x1234_5678, 0xFFFF_FFFF_FFFF_FFFFusize];

    for &word in WORDS {
        let clean = LowTagLayout::<3>::clean_word(word);
        let tag = LowTagLayout::<3>::low_tag(word);
        assert_eq!(clean & tag, 0);
        assert_eq!(clean | tag, word);
        assert_eq!(clean & LowTagLayout::<3>::LOW_MASK, 0);
        assert!(tag <= LowTagLayout::<3>::LOW_MASK);
    }
}

#[test]
fn tagged_non_null_roundtrips_all_low_tags_and_never_derefs_dirty_word() {
    let mut value = Aligned64(99);
    let ptr = NonNull::from(&mut value);
    let clean_addr = ptr.as_ptr() as usize;
    LowTagLayout::<6>::validate_addr(clean_addr).unwrap();

    for tag in 0..=LowTagLayout::<6>::LOW_MASK {
        let tagged = TaggedNonNull::<Aligned64, 6>::new(ptr, tag).unwrap();
        assert_eq!(tagged.tag(), tag);
        assert_eq!(tagged.clean_addr(), clean_addr);
        assert_eq!(tagged.word(), clean_addr | tag);
        assert_eq!(tagged.clean().as_ptr(), ptr.as_ptr());
        unsafe {
            assert_eq!(tagged.as_ref().0, 99);
        }
    }
}

#[test]
fn tagged_non_null_from_word_roundtrips_nonzero_clean_words_and_rejects_zero() {
    let mut value = Aligned64(7);
    let ptr = NonNull::from(&mut value);
    let addr = ptr.as_ptr() as usize;

    let ok = TaggedNonNull::<Aligned64, 3>::from_word(addr | 0b111).unwrap();
    assert_eq!(ok.tag(), 0b111);
    assert_eq!(ok.clean_addr(), addr);

    assert_eq!(
        TaggedNonNull::<Aligned64, 3>::from_word(0b111).unwrap_err(),
        TagError::NullAddress
    );
    assert_eq!(
        LowTagLayout::<3>::validate_addr(addr + 4).unwrap_err(),
        TagError::Misaligned {
            addr: addr + 4,
            align: 8
        }
    );
}

#[test]
fn with_tag_preserves_address_and_only_changes_low_bits() {
    let mut value = Aligned64(123);
    let ptr = TaggedNonNull::<Aligned64, 4>::from_mut(&mut value, 0b0011).unwrap();
    let changed = ptr.with_tag(0b1010).unwrap();

    assert_eq!(changed.clean_addr(), ptr.clean_addr());
    assert_eq!(changed.tag(), 0b1010);
    assert_eq!(changed.word(), ptr.clean_addr() | 0b1010);
    assert!(ptr.with_tag(0b1_0000).is_err());
}

#[test]
fn split_tagged_pointer_roundtrips_head_tail_and_clean_body() {
    let mut value = Aligned64(5);
    let ptr = NonNull::from(&mut value);
    let clean_addr = ptr.as_ptr() as usize;

    for top in [0x00, 0x01, 0x7F, 0x80, 0xFF] {
        for low in [0, 1, 7, 15] {
            let split = SplitTaggedNonNull::<Aligned64, 4>::new(ptr, top, low).unwrap();
            assert_eq!(split.top(), top);
            assert_eq!(split.low(), low);
            assert_eq!(split.clean_addr(), clean_addr);
            assert_eq!(split.clean().as_ptr(), ptr.as_ptr());
            assert_eq!(
                SplitTaggedNonNull::<Aligned64, 4>::from_word(split.word())
                    .unwrap()
                    .word(),
                split.word()
            );
        }
    }

    assert_eq!(
        SplitTaggedNonNull::<Aligned64, 4>::from_word(
            0xFFusize << SplitTaggedNonNull::<Aligned64, 4>::TOP_SHIFT
        )
        .unwrap_err(),
        TagError::NullAddress
    );
}

#[test]
fn split_tag_mutators_change_only_the_requested_end() {
    let mut value = Aligned64(5);
    let ptr = NonNull::from(&mut value);
    let split = SplitTaggedNonNull::<Aligned64, 4>::new(ptr, 0xAA, 0b0011).unwrap();

    let top_changed = split.with_top(0x55).unwrap();
    assert_eq!(top_changed.top(), 0x55);
    assert_eq!(top_changed.low(), 0b0011);
    assert_eq!(top_changed.clean_addr(), split.clean_addr());

    let low_changed = split.with_low(0b1111).unwrap();
    assert_eq!(low_changed.top(), 0xAA);
    assert_eq!(low_changed.low(), 0b1111);
    assert_eq!(low_changed.clean_addr(), split.clean_addr());
    assert!(split.with_low(0b1_0000).is_err());
}

#[test]
fn keyed_pointers_keep_low_tags_visible_and_mask_clean_addresses() {
    let mut value = Aligned64(777);
    let ptr = NonNull::from(&mut value);
    let addr = ptr.as_ptr() as usize;
    let key = Key::<4>::from_seed(0xA5A5_1234);

    assert_eq!(key.mask() & LowTagLayout::<4>::LOW_MASK, 0);

    let tagged = KeyedTaggedNonNull::<Aligned64, 4>::new(ptr, 0b1010, key).unwrap();
    assert_eq!(tagged.tag(), 0b1010);
    assert_eq!(tagged.stored_word() & LowTagLayout::<4>::LOW_MASK, 0b1010);
    assert_ne!(tagged.stored_word() & LowTagLayout::<4>::CLEAN_MASK, addr);
    assert_eq!(tagged.reveal_addr(key).unwrap(), addr);
    assert_eq!(tagged.reveal(key).unwrap().as_ptr(), ptr.as_ptr());
}

#[test]
fn wrong_key_does_not_reveal_the_original_address_for_different_masks() {
    let mut value = Aligned64(888);
    let ptr = NonNull::from(&mut value);
    let addr = ptr.as_ptr() as usize;
    let key = Key::<4>::from_seed(0x1111_2222);
    let wrong = Key::<4>::from_seed(0x3333_4444);
    assert_ne!(key.mask(), wrong.mask());

    let tagged = KeyedTaggedNonNull::<Aligned64, 4>::new(ptr, 0b0011, key).unwrap();
    if let Ok(revealed) = tagged.reveal_addr(wrong) {
        assert_ne!(revealed, addr);
    }
}

#[test]
fn residual_classes_validate_moduli_and_route_by_remainder() {
    assert_eq!(ResidualClass::new(0, 0).unwrap_err(), TagError::ZeroModulus);
    assert_eq!(
        ResidualClass::new(7, 7).unwrap_err(),
        TagError::BadResidue {
            modulus: 7,
            residue: 7
        }
    );

    let class = ResidualClass::new(7, 3).unwrap();
    assert!(class.check(10));
    assert!(!class.check(11));
    assert_eq!(class.residue_of(17), 3);

    let router = ResidualRouter::new(5)
        .unwrap()
        .set(0, 0x10)
        .unwrap()
        .set(4, 0x44)
        .unwrap();
    assert_eq!(router.modulus(), 5);
    assert_eq!(router.selector_for(25), 0x10);
    assert_eq!(router.selector_for(29), 0x44);
    assert_eq!(
        ResidualRouter::new(3).unwrap().set(3, 0xFF).unwrap_err(),
        TagError::BadResidue {
            modulus: 3,
            residue: 3
        }
    );
}

#[test]
fn selector_word_masks_fields_into_4_4_8_layout() {
    let s = SelectorWord::new(0xAB, 0xCD, 0xEF);
    assert_eq!(s.trait_id(), 0x0B);
    assert_eq!(s.impl_id(), 0x0D);
    assert_eq!(s.function_id(), 0xEF);
    assert_eq!(s.table_index(), 0xEF);
    assert_eq!(s.raw(), 0xBDEF);
}

#[test]
fn prophecy_bits_pack_unpack_all_lane_values_and_flags() {
    let lanes = [
        PortHint::Integer,
        PortHint::Vector,
        PortHint::Load,
        PortHint::Store,
        PortHint::Dispatch,
    ];

    for &lane in &lanes {
        for flags in 0u8..16 {
            let bits = ProphecyBits {
                maybe_zero: flags & 1 != 0,
                maybe_empty: flags & 2 != 0,
                hot: flags & 4 != 0,
                skip_hint: flags & 8 != 0,
                port_hint: lane,
            };
            assert_eq!(ProphecyBits::unpack(bits.pack()), bits);
        }
    }

    assert_eq!(ProphecyBits::unpack(0xF0).port_hint, PortHint::Integer);
}

#[test]
fn value_word_immediate_and_handle_encodings_are_disjoint() {
    for value in [-1, 0, 1, 42, i32::MAX, i32::MIN + 1] {
        let word = ValueWord::immediate_i31(value);
        assert!(word.is_immediate());
        assert_eq!(word.as_i31(), Some(value));
        assert_eq!(word.raw() & 1, 1);
    }

    let handle = ValueWord::handle(0xDEAD_BEEF);
    assert!(!handle.is_immediate());
    assert_eq!(handle.as_i31(), None);
    assert_eq!(handle.raw() & 1, 0);
}

#[test]
fn atomic_tagged_word_load_store_and_compare_exchange_are_exact() {
    use petr_core::AtomicTaggedWord;
    use std::sync::atomic::Ordering;

    let word = AtomicTaggedWord::new(0x10);
    assert_eq!(word.load(Ordering::SeqCst), 0x10);

    word.store(0x20, Ordering::SeqCst);
    assert_eq!(word.load(Ordering::SeqCst), 0x20);

    assert_eq!(
        word.compare_exchange(0x20, 0x30, Ordering::SeqCst, Ordering::SeqCst),
        Ok(0x20)
    );
    assert_eq!(word.load(Ordering::SeqCst), 0x30);
    assert_eq!(
        word.compare_exchange(0x20, 0x40, Ordering::SeqCst, Ordering::SeqCst),
        Err(0x30)
    );
    assert_eq!(word.load(Ordering::SeqCst), 0x30);
}

use core::num::NonZeroUsize;
use petr_core::KeyMaterial;

#[test]
fn random_key_masks_have_clean_low_bits_and_are_nonzero() {
    for _ in 0..32 {
        let key = Key::<5>::random();
        assert_ne!(key.mask(), 0);
        assert_eq!(key.mask() & LowTagLayout::<5>::LOW_MASK, 0);
    }
}

#[test]
fn key_material_exposes_copyable_key_view() {
    let material = KeyMaterial::<5>::from_seed(0xABCD);
    let key = material.key();
    assert_eq!(key.mask() & LowTagLayout::<5>::LOW_MASK, 0);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        failure_persistence: None,
        .. proptest::test_runner::Config::default()
    })]
    #[test]
    fn keyed_mask_reveal_roundtrip_for_aligned_addresses(seed in any::<u64>(), addr_base in 1usize..1_000_000, tag in 0usize..32) {
        let key = Key::<5>::from_seed(seed);
        let addr = (addr_base << 5) & LowTagLayout::<5>::CLEAN_MASK;
        proptest::prop_assume!(addr != 0);
        let word = key.mask_addr(addr, tag).unwrap();
        prop_assert_eq!(word.get() & LowTagLayout::<5>::LOW_MASK, tag);
        prop_assert_eq!(key.reveal_addr(word).unwrap(), addr);
    }

    #[test]
    fn keyed_mask_rejects_tags_outside_layout(seed in any::<u64>(), addr_base in 1usize..1_000_000, tag in 32usize..512) {
        let key = Key::<5>::from_seed(seed);
        let addr = (addr_base << 5) & LowTagLayout::<5>::CLEAN_MASK;
        proptest::prop_assume!(addr != 0);
        prop_assert!(key.mask_addr(addr, tag).is_err());
    }

    #[test]
    fn nonzero_words_never_reveal_null_with_valid_key(seed in any::<u64>(), raw in 1usize..usize::MAX) {
        let key = Key::<4>::from_seed(seed);
        let Some(word) = NonZeroUsize::new(raw) else { return Ok(()); };
        if let Ok(addr) = key.reveal_addr(word) {
            prop_assert_ne!(addr, 0);
            prop_assert_eq!(addr & LowTagLayout::<4>::LOW_MASK, 0);
        }
    }
}
