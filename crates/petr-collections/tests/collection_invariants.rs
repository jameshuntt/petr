use petr_collections::{DispatchVec, KeyedTaggedVec, TaggedSlab, TaggedVec};
use petr_core::{Key, LowTagLayout, PortHint, ProphecyBits, SelectorWord};
use petr_dispatch::{OpFrame, default_table};
use proptest::*;

#[repr(align(32))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    id: usize,
    text: &'static str,
}

fn active() -> ProphecyBits {
    ProphecyBits {
        maybe_zero: false,
        maybe_empty: false,
        hot: true,
        skip_hint: false,
        port_hint: PortHint::Dispatch,
    }
}

fn skip() -> ProphecyBits {
    ProphecyBits {
        skip_hint: true,
        ..active()
    }
}

#[test]
fn tagged_vec_handles_remain_stable_after_growth_and_tag_mutation() {
    let mut vec = TaggedVec::<Item, 5>::new();
    let first = vec
        .push(
            Item {
                id: 0,
                text: "first",
            },
            0b10101,
        )
        .unwrap();
    let first_handle = vec.handle(first).unwrap();
    let first_addr = first_handle.clean_addr();

    for i in 1..512 {
        vec.push(
            Item {
                id: i,
                text: "more",
            },
            i & LowTagLayout::<5>::LOW_MASK,
        )
        .unwrap();
    }

    assert_eq!(vec.len(), 512);
    assert!(!vec.is_empty());
    assert_eq!(vec.get(first).unwrap().text, "first");
    assert_eq!(vec.handle(first).unwrap().clean_addr(), first_addr);
    assert_eq!(vec.handle(first).unwrap().tag(), 0b10101);

    vec.set_tag(first, 0b00011).unwrap();
    assert_eq!(vec.handle(first).unwrap().clean_addr(), first_addr);
    assert_eq!(vec.handle(first).unwrap().tag(), 0b00011);
    assert!(vec.set_tag(first, 0b1_00000).is_err());
}

#[test]
fn tagged_vec_get_mut_updates_the_value_without_changing_handle_address() {
    let mut vec = TaggedVec::<Item, 5>::new();
    let idx = vec.push(Item { id: 1, text: "old" }, 7).unwrap();
    let addr = vec.handle(idx).unwrap().clean_addr();

    vec.get_mut(idx).unwrap().text = "new";

    assert_eq!(vec.get(idx).unwrap().text, "new");
    assert_eq!(vec.handle(idx).unwrap().clean_addr(), addr);
}

#[test]
fn keyed_vec_masks_addresses_and_reveals_only_inside_collection_with_matching_key() {
    let key = Key::<5>::from_seed(0xABCD_1234);
    let mut vec = KeyedTaggedVec::<Item, 5>::with_key(key);
    let idx = vec
        .push(
            Item {
                id: 7,
                text: "secret",
            },
            0b10101,
        )
        .unwrap();
    let handle = vec.handle(idx).unwrap();

    assert_eq!(vec.key(), key);
    assert_eq!(handle.tag(), 0b10101);
    assert_eq!(vec.get_by_handle(handle).unwrap().unwrap().text, "secret");
    assert_ne!(
        handle.stored_word() & LowTagLayout::<5>::CLEAN_MASK,
        handle.reveal_addr(key).unwrap()
    );

    let wrong_key = Key::<5>::from_seed(0xDEAD_BEEF);
    let wrong_addr = handle.reveal_addr(wrong_key).ok();
    let actual_addr = handle.reveal_addr(key).unwrap();
    assert_ne!(wrong_addr, Some(actual_addr));
}

#[test]
fn keyed_vec_preserves_distinct_tags_and_indexes_across_many_pushes() {
    let key = Key::<4>::from_seed(0x44);
    let mut vec = KeyedTaggedVec::<Item, 4>::with_key(key);

    for i in 0..128 {
        let idx = vec
            .push(Item { id: i, text: "x" }, i & LowTagLayout::<4>::LOW_MASK)
            .unwrap();
        assert_eq!(idx, i);
    }

    assert_eq!(vec.len(), 128);
    for i in 0..128 {
        let handle = vec.handle(i).unwrap();
        assert_eq!(handle.tag(), i & LowTagLayout::<4>::LOW_MASK);
        assert_eq!(vec.get_by_handle(handle).unwrap().unwrap().id, i);
    }
}

#[test]
fn keyed_vec_rejects_tag_that_does_not_fit_layout() {
    let key = Key::<3>::from_seed(1);
    let mut vec = KeyedTaggedVec::<Item, 3>::with_key(key);
    assert!(vec.push(Item { id: 1, text: "bad" }, 0b1_000).is_err());
    assert_eq!(vec.len(), 0);
}

#[test]
fn slab_detects_stale_handles_and_reuses_slots_with_new_generation() {
    let mut slab = TaggedSlab::<Item, 5>::new();
    let a = slab.insert(Item { id: 1, text: "a" }, 0b00001).unwrap();
    let b = slab.insert(Item { id: 2, text: "b" }, 0b00010).unwrap();

    assert_eq!(slab.occupied(), 2);
    assert_eq!(slab.get(a).unwrap().text, "a");
    assert_eq!(slab.get(b).unwrap().text, "b");
    assert_eq!(a.tagged.tag(), 0b00001);

    assert_eq!(slab.remove(a).unwrap().text, "a");
    assert_eq!(slab.occupied(), 1);
    assert!(slab.get(a).is_none());
    assert!(slab.remove(a).is_none());

    let c = slab.insert(Item { id: 3, text: "c" }, 0b00100).unwrap();
    assert_eq!(c.index, a.index);
    assert_ne!(c.generation, a.generation);
    assert_eq!(slab.get(c).unwrap().text, "c");
    assert!(slab.get(a).is_none());
}

#[test]
fn dispatch_vec_stores_selector_prophecy_and_low_tag_per_item() {
    let mut vec = DispatchVec::<Item, 5>::new();
    let encode = SelectorWord::new(1, 2, 0x20);
    let prefix = SelectorWord::new(1, 2, 0x01);

    vec.push(Item { id: 1, text: "hex" }, 0b00101, encode, active())
        .unwrap();
    vec.push(
        Item {
            id: 2,
            text: "skip",
        },
        0b00110,
        prefix,
        skip(),
    )
    .unwrap();

    let handles: Vec<_> = vec.handles().collect();
    assert_eq!(handles.len(), 2);
    assert_eq!(handles[0].ptr.tag(), 0b00101);
    assert_eq!(handles[0].selector, encode);
    assert!(handles[0].prophecy.hot);
    assert_eq!(handles[1].ptr.tag(), 0b00110);
    assert!(handles[1].prophecy.skip_hint);
}

#[test]
fn dispatch_vec_uses_selector_when_active_and_preserves_seed_frame_when_skipped() {
    let table = default_table();
    let mut vec = DispatchVec::<Item, 5>::new();
    vec.push(
        Item { id: 1, text: "run" },
        1,
        SelectorWord::new(1, 1, 0x01),
        active(),
    )
    .unwrap();
    vec.push(
        Item {
            id: 2,
            text: "skip",
        },
        2,
        SelectorWord::new(1, 1, 0x02),
        skip(),
    )
    .unwrap();
    vec.push(
        Item {
            id: 3,
            text: "run2",
        },
        3,
        SelectorWord::new(1, 1, 0x20),
        active(),
    )
    .unwrap();

    let out = vec.dispatch_all(&table, OpFrame::text("ff")).unwrap();
    assert_eq!(out[0], OpFrame::text("Prefix_ff"));
    assert_eq!(out[1], OpFrame::text("ff"));
    assert_eq!(out[2], OpFrame::text("FF"));
}

#[test]
fn dispatch_vec_propagates_operation_errors_for_active_bad_inputs() {
    let table = default_table();
    let mut vec = DispatchVec::<Item, 5>::new();
    vec.push(
        Item { id: 1, text: "bad" },
        1,
        SelectorWord::new(1, 1, 0x20),
        active(),
    )
    .unwrap();

    assert!(vec.dispatch_all(&table, OpFrame::text("not hex")).is_err());
}

use core::num::NonZeroUsize;
use petr_collections::KeyedDispatchVec;

#[test]
fn keyed_dispatch_vec_combines_iv_selector_prophecy_and_generation_checks() {
    let key = Key::<5>::from_seed(0x55AA);
    let table = default_table();
    let mut vec = KeyedDispatchVec::<Item, 5>::with_key(key);
    let selector = SelectorWord::new(3, 4, 0x20);

    let h = vec
        .push(Item { id: 99, text: "kd" }, 0b10101, selector, active())
        .unwrap();
    assert_eq!(h.tag(), 0b10101);
    assert_eq!(h.selector(), selector);
    assert_eq!(h.selector().trait_id(), 3);
    assert_eq!(h.selector().impl_id(), 4);
    assert_eq!(h.selector().function_id(), 0x20);
    assert_eq!(vec.get(h).unwrap().text, "kd");

    let masked_clean = h.check_word().get() & LowTagLayout::<5>::CLEAN_MASK;
    let revealed = key.reveal_addr(h.check_word()).unwrap();
    assert_ne!(masked_clean, revealed);

    let out = vec
        .dispatch_handle(&table, h, OpFrame::text("ff"))
        .unwrap()
        .unwrap();
    assert_eq!(out, OpFrame::text("FF"));
}

#[test]
fn keyed_dispatch_vec_rejects_forged_check_words_and_stale_handles() {
    let key = Key::<5>::from_seed(0xBEEF);
    let mut vec = KeyedDispatchVec::<Item, 5>::with_key(key);
    let h = vec
        .push(
            Item {
                id: 1,
                text: "live",
            },
            3,
            SelectorWord::new(1, 1, 0x01),
            active(),
        )
        .unwrap();

    let forged_word = NonZeroUsize::new(h.check_word().get() ^ (1 << 8)).unwrap();
    let forged = h.with_check_word(forged_word);
    assert!(vec.get(forged).is_err());

    let stale = h.with_generation(h.generation().wrapping_add(1));
    assert!(vec.get(stale).is_err());

    let removed = vec.remove(h).unwrap();
    assert_eq!(removed.text, "live");
    assert!(vec.get(h).is_err());

    let h2 = vec
        .push(
            Item { id: 2, text: "new" },
            4,
            SelectorWord::new(1, 1, 0x02),
            active(),
        )
        .unwrap();
    assert_eq!(h2.index(), h.index());
    assert_ne!(h2.generation(), h.generation());
    assert!(vec.get(h).is_err());
    assert_eq!(vec.get(h2).unwrap().text, "new");
}

#[test]
fn keyed_dispatch_vec_set_tag_validates_and_refreshes_handle() {
    let key = Key::<5>::from_seed(0xCAFE);
    let mut vec = KeyedDispatchVec::<Item, 5>::with_key(key);
    let h = vec
        .push(
            Item { id: 5, text: "tag" },
            1,
            SelectorWord::new(1, 1, 0x01),
            active(),
        )
        .unwrap();

    let h2 = vec.set_tag(h, 0b11111).unwrap();
    assert_eq!(h2.tag(), 0b11111);
    assert!(vec.get(h).is_err());
    assert_eq!(vec.get(h2).unwrap().text, "tag");
    assert!(vec.set_tag(h2, 0b1_00000).is_err());
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        failure_persistence: None,
        .. proptest::test_runner::Config::default()
    })]
    #[test]
    fn keyed_dispatch_vec_accepts_only_valid_low_tags(tag in 0usize..128) {
        let key = Key::<5>::from_seed(0x1234);
        let mut vec = KeyedDispatchVec::<Item, 5>::with_key(key);
        let result = vec.push(Item { id: tag, text: "p" }, tag, SelectorWord::new(1, 1, 0x01), active());
        if tag <= LowTagLayout::<5>::LOW_MASK {
            let h = result.unwrap();
            prop_assert_eq!(h.tag(), tag);
            prop_assert_eq!(vec.get(h).unwrap().id, tag);
        } else {
            prop_assert!(result.is_err());
            prop_assert!(vec.is_empty());
        }
    }
}

use petr_collections::KeyedArena;

#[test]
fn keyed_arena_insert_get_mut_remove_and_reuse_slots_with_generations() {
    let key = Key::<5>::from_seed(0xA11E);
    let mut arena = KeyedArena::<Item, 5>::with_key(key);
    let selector = SelectorWord::new(2, 3, 0x01);

    let a = arena
        .insert(Item { id: 1, text: "a" }, 0b10101, selector, active())
        .unwrap();
    let b = arena
        .insert(Item { id: 2, text: "b" }, 0b00111, selector, active())
        .unwrap();

    assert_eq!(arena.len(), 2);
    assert_eq!(arena.slot_capacity(), 2);
    assert_eq!(arena.free_len(), 0);
    assert!(arena.contains(a));
    assert_eq!(arena.get(a).unwrap().text, "a");
    assert_eq!(arena.get(b).unwrap().id, 2);

    arena.get_mut(a).unwrap().text = "a2";
    assert_eq!(arena.get(a).unwrap().text, "a2");

    let removed = arena.remove(a).unwrap();
    assert_eq!(removed.text, "a2");
    assert_eq!(arena.len(), 1);
    assert_eq!(arena.free_len(), 1);
    assert!(arena.get(a).is_err());

    let c = arena
        .insert(Item { id: 3, text: "c" }, 0b00011, selector, active())
        .unwrap();
    assert_eq!(c.index(), a.index());
    assert_ne!(c.generation(), a.generation());
    assert!(arena.get(a).is_err());
    assert_eq!(arena.get(c).unwrap().text, "c");
}

#[test]
fn keyed_arena_rejects_forged_generation_check_selector_and_prophecy() {
    let key = Key::<5>::from_seed(0x5150);
    let mut arena = KeyedArena::<Item, 5>::with_key(key);
    let h = arena
        .insert(
            Item {
                id: 10,
                text: "secure",
            },
            0b01010,
            SelectorWord::new(1, 2, 0x20),
            active(),
        )
        .unwrap();

    let forged_word = NonZeroUsize::new(h.check_word().get() ^ (1 << 8)).unwrap();
    assert!(arena.get(h.with_check_word(forged_word)).is_err());
    assert!(
        arena
            .get(h.with_generation(h.generation().wrapping_add(1)))
            .is_err()
    );
    assert!(
        arena
            .get(h.with_selector(SelectorWord::new(1, 2, 0x21)))
            .is_err()
    );
    assert!(arena.get(h.with_prophecy(skip())).is_err());

    assert_eq!(arena.get(h).unwrap().text, "secure");
}

#[test]
fn keyed_arena_metadata_updates_invalidate_old_handles() {
    let key = Key::<5>::from_seed(0x7777);
    let mut arena = KeyedArena::<Item, 5>::with_key(key);
    let h = arena
        .insert(
            Item {
                id: 1,
                text: "meta",
            },
            1,
            SelectorWord::new(1, 1, 0x01),
            active(),
        )
        .unwrap();

    let h2 = arena.set_tag(h, 0b11111).unwrap();
    assert!(arena.get(h).is_err());
    assert_eq!(h2.tag(), 0b11111);
    assert_eq!(arena.get(h2).unwrap().text, "meta");
    assert!(arena.set_tag(h2, 0b1_00000).is_err());

    let h3 = arena
        .set_selector(h2, SelectorWord::new(1, 1, 0x02))
        .unwrap();
    assert!(arena.get(h2).is_err());
    assert_eq!(arena.selector(h3).unwrap().function_id(), 0x02);

    let h4 = arena.set_prophecy(h3, skip()).unwrap();
    assert!(arena.get(h3).is_err());
    assert!(arena.prophecy(h4).unwrap().skip_hint);

    let h5 = arena.refresh_generation(h4).unwrap();
    assert!(arena.get(h4).is_err());
    assert_ne!(h5.generation(), h4.generation());
    assert_eq!(arena.get(h5).unwrap().text, "meta");
}

#[test]
fn keyed_arena_dispatches_active_entries_and_skips_skip_hint_entries() {
    let key = Key::<5>::from_seed(0xDA7A);
    let table = default_table();
    let mut arena = KeyedArena::<Item, 5>::with_key(key);

    let encode = arena
        .insert(
            Item {
                id: 1,
                text: "encode",
            },
            1,
            SelectorWord::new(1, 1, 0x20),
            active(),
        )
        .unwrap();
    let skipped = arena
        .insert(
            Item {
                id: 2,
                text: "skip",
            },
            2,
            SelectorWord::new(1, 1, 0x01),
            skip(),
        )
        .unwrap();

    let encoded = arena
        .dispatch_handle(&table, encode, OpFrame::text("ff"))
        .unwrap()
        .unwrap();
    assert_eq!(encoded, OpFrame::text("FF"));

    let skipped_out = arena
        .dispatch_handle(&table, skipped, OpFrame::text("ff"))
        .unwrap()
        .unwrap();
    assert_eq!(skipped_out, OpFrame::text("ff"));

    let all = arena.dispatch_all(&table, OpFrame::text("ff")).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].as_ref().unwrap().clone(), OpFrame::text("FF"));
    assert_eq!(all[1].as_ref().unwrap().clone(), OpFrame::text("ff"));
}

#[test]
fn keyed_arena_live_handles_and_clear_behave_like_arena_policy() {
    let key = Key::<4>::from_seed(0x4444);
    let mut arena = KeyedArena::<Item, 4>::with_key(key);

    for i in 0..16 {
        arena
            .insert(
                Item {
                    id: i,
                    text: "live",
                },
                i & LowTagLayout::<4>::LOW_MASK,
                SelectorWord::new(1, (i & 0x0F) as u8, 0x01),
                active(),
            )
            .unwrap();
    }

    let handles = arena.live_handles();
    assert_eq!(handles.len(), 16);
    for (i, handle) in handles.iter().copied().enumerate() {
        assert_eq!(handle.index() as usize, i);
        assert_eq!(handle.tag(), i & LowTagLayout::<4>::LOW_MASK);
        assert_eq!(arena.get(handle).unwrap().id, i);
    }

    arena.clear();
    assert!(arena.is_empty());
    assert_eq!(arena.slot_capacity(), 0);
    for handle in handles {
        assert!(arena.get(handle).is_err());
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        failure_persistence: None,
        .. proptest::test_runner::Config::default()
    })]
    #[test]
    fn keyed_arena_accepts_only_valid_low_tags(tag in 0usize..128) {
        let key = Key::<5>::from_seed(0x4242);
        let mut arena = KeyedArena::<Item, 5>::with_key(key);
        let result = arena.insert(Item { id: tag, text: "arena" }, tag, SelectorWord::new(1, 1, 0x01), active());
        if tag <= LowTagLayout::<5>::LOW_MASK {
            let h = result.unwrap();
            prop_assert_eq!(h.tag(), tag);
            prop_assert_eq!(arena.get(h).unwrap().id, tag);
        } else {
            prop_assert!(result.is_err());
            prop_assert!(arena.is_empty());
        }
    }
}

use petr_collections::ArenaEngine;

#[test]
fn encoded_arena_handles_roundtrip_and_validate_only_in_owner() {
    let table = default_table();
    let key = Key::<5>::from_seed(0xEC0D_EC0D);
    let mut engine = ArenaEngine::<Item, 5>::with_key_and_table(key, table);
    let handle = engine
        .insert(
            Item {
                id: 44,
                text: "encoded",
            },
            0b10101,
            SelectorWord::new(1, 1, 0x20),
            active(),
        )
        .unwrap();

    let encoded = engine.encode_handle(handle).unwrap();
    assert_eq!(encoded.index, handle.index());
    assert_eq!(encoded.generation, handle.generation());
    assert_eq!(encoded.selector, handle.selector().raw());
    assert_eq!(encoded.prophecy, handle.prophecy().pack());
    assert_eq!(encoded.tag, handle.tag() as u16);

    let decoded = engine.decode_handle(encoded).unwrap();
    assert_eq!(engine.get(decoded).unwrap().text, "encoded");

    let mut forged = encoded;
    forged.selector ^= 1;
    assert!(engine.decode_handle(forged).is_err());

    let mut zero_check = encoded;
    zero_check.check = 0;
    assert!(zero_check.to_handle::<Item, 5>().is_none());
}

#[test]
fn arena_engine_dispatches_and_delegates_metadata_updates() {
    let table = default_table();
    let mut engine = ArenaEngine::<Item, 5>::with_key_and_table(Key::from_seed(0xE001), table);
    let handle = engine
        .insert(
            Item {
                id: 7,
                text: "engine",
            },
            1,
            SelectorWord::new(1, 1, 0x20),
            active(),
        )
        .unwrap();

    let out = engine
        .dispatch(handle, OpFrame::text("ff"))
        .unwrap()
        .unwrap();
    assert_eq!(out, OpFrame::text("FF"));

    let skipped = engine.set_prophecy(handle, skip()).unwrap();
    assert!(engine.get(handle).is_err());
    let out = engine
        .dispatch(skipped, OpFrame::text("ff"))
        .unwrap()
        .unwrap();
    assert_eq!(out, OpFrame::text("ff"));

    let removed = engine.remove(skipped).unwrap();
    assert_eq!(removed.text, "engine");
    assert!(engine.get(skipped).is_err());
}
