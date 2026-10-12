use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use petr_collections::{EncodedArenaHandle, PagedKeyedArena};
use petr_core::{Key, PortHint, ProphecyBits, SelectorWord};

const LOW_BITS: usize = 4;
const PAGE_SIZE: usize = 4;
const READY: SelectorWord = SelectorWord::new(1, 2, 3);
const DONE: SelectorWord = SelectorWord::new(1, 2, 4);

#[repr(align(16))]
#[derive(Debug, PartialEq, Eq)]
struct Cell(u64);

fn prophecy(skip_hint: bool) -> ProphecyBits {
    ProphecyBits {
        maybe_zero: false,
        maybe_empty: false,
        hot: !skip_hint,
        skip_hint,
        port_hint: PortHint::Dispatch,
    }
}

#[test]
fn paged_arena_amortizes_allocations_and_keeps_handles_valid() {
    let mut arena = PagedKeyedArena::<Cell, LOW_BITS, PAGE_SIZE>::with_key(
        Key::from_seed(0x67_1A),
    );
    arena.reserve_entries(10).expect("reserve pages");
    assert_eq!(arena.page_count(), 3);
    assert_eq!(arena.allocated_slot_capacity(), 12);

    let handles: Vec<_> = (0..10)
        .map(|value| {
            arena
                .insert(Cell(value), 1, READY, prophecy(false))
                .expect("insert")
        })
        .collect();

    assert_eq!(arena.len(), 10);
    assert_eq!(handles[0].page_index(), 0);
    assert_eq!(handles[4].page_index(), 1);
    assert_eq!(handles[8].page_index(), 2);
    for (expected, handle) in handles.iter().copied().enumerate() {
        assert_eq!(arena.get(handle).expect("validated get").0, expected as u64);
    }
}

#[test]
fn paged_arena_mutates_in_place_and_transitions_metadata_once() {
    let mut arena = PagedKeyedArena::<Cell, LOW_BITS, PAGE_SIZE>::with_key(
        Key::from_seed(0x67_1B),
    );
    let old = arena
        .insert(Cell(7), 1, READY, prophecy(false))
        .expect("insert");
    let address_before = arena.get(old).expect("get") as *const Cell as usize;
    arena.get_mut(old).expect("get mut").0 = 49;
    let updated = arena
        .transition(old, 3, DONE, prophecy(true))
        .expect("transition");
    let address_after = arena.get(updated).expect("get updated") as *const Cell as usize;

    assert_eq!(address_before, address_after);
    assert_eq!(arena.get(updated).expect("get updated").0, 49);
    assert!(!arena.contains(old));
    assert_eq!(updated.tag(), 3);
    assert_eq!(updated.selector(), DONE);
    assert!(updated.prophecy().skip_hint);
}

#[test]
fn paged_arena_reuses_slots_with_new_generations_and_encoded_handles() {
    let mut arena = PagedKeyedArena::<Cell, LOW_BITS, PAGE_SIZE>::with_key(
        Key::from_seed(0x67_1C),
    );
    let first = arena
        .insert(Cell(11), 1, READY, prophecy(false))
        .expect("insert first");
    assert_eq!(arena.remove(first).expect("remove"), Cell(11));
    let second = arena
        .insert(Cell(12), 1, READY, prophecy(false))
        .expect("insert second");

    assert_eq!(first.index(), second.index());
    assert_ne!(first.generation(), second.generation());
    assert!(!arena.contains(first));
    assert!(arena.contains(second));

    let encoded = EncodedArenaHandle::from(second);
    let decoded = encoded
        .to_paged_handle::<Cell, LOW_BITS, PAGE_SIZE>()
        .expect("decode");
    assert_eq!(arena.get(decoded).expect("decoded handle validates"), &Cell(12));
}

#[test]
fn reset_retain_pages_drops_entries_keeps_capacity_and_invalidates_handles() {
    let mut arena = PagedKeyedArena::<Cell, LOW_BITS, PAGE_SIZE>::with_key(
        Key::from_seed(0x67_1D),
    );
    arena.reserve_entries(9).expect("reserve pages");
    let first = arena
        .insert(Cell(21), 1, READY, prophecy(false))
        .expect("insert first");
    let second = arena
        .insert(Cell(22), 1, READY, prophecy(false))
        .expect("insert second");
    let pages_before = arena.page_count();
    let capacity_before = arena.allocated_slot_capacity();

    arena.reset_retain_pages();

    assert!(arena.is_empty());
    assert_eq!(arena.page_count(), pages_before);
    assert_eq!(arena.allocated_slot_capacity(), capacity_before);
    assert!(!arena.contains(first));
    assert!(!arena.contains(second));

    let reused = arena
        .insert(Cell(23), 1, READY, prophecy(false))
        .expect("insert after reset");
    assert_eq!(reused.index(), 0);
    assert_ne!(reused.generation(), first.generation());
    assert_eq!(arena.get(reused).expect("reused handle validates"), &Cell(23));
}


#[repr(align(16))]
struct DropCell(Arc<AtomicUsize>);

impl Drop for DropCell {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn reset_and_clear_drop_each_initialized_value_exactly_once() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut arena = PagedKeyedArena::<DropCell, LOW_BITS, PAGE_SIZE>::with_key(
        Key::from_seed(0x67_1E),
    );
    for _ in 0..7 {
        arena
            .insert(DropCell(Arc::clone(&drops)), 1, READY, prophecy(false))
            .expect("insert before reset");
    }
    arena.reset_retain_pages();
    assert_eq!(drops.load(Ordering::SeqCst), 7);

    for _ in 0..3 {
        arena
            .insert(DropCell(Arc::clone(&drops)), 1, READY, prophecy(false))
            .expect("insert before clear");
    }
    arena.clear();
    assert_eq!(drops.load(Ordering::SeqCst), 10);
}

#[test]
fn keyed_page_capability_amortizes_validation_and_transitions_live_entries() {
    use petr_collections::{KeyedArenaError, PagedEntryAction};

    let mut arena = PagedKeyedArena::<Cell, LOW_BITS, PAGE_SIZE>::with_key(
        Key::from_seed(0x67_1F),
    );
    let old_handles: Vec<_> = (0..PAGE_SIZE)
        .map(|value| {
            arena
                .insert(Cell(value as u64), 1, READY, prophecy(false))
                .expect("insert page cell")
        })
        .collect();
    let page = arena.page_handle_at(0).expect("page capability");
    assert_eq!(arena.validate_page_handle(page).expect("valid page"), 0);

    arena
        .try_visit_page_mut::<KeyedArenaError, _>(
            page,
            |_index, cell, _selector, _prophecy| {
                cell.0 += 100;
                Ok(PagedEntryAction::Transition {
                    tag: 3,
                    selector: DONE,
                    prophecy: prophecy(true),
                })
            },
        )
        .expect("page kernel");

    for (index, old) in old_handles.into_iter().enumerate() {
        assert!(!arena.contains(old));
        let refreshed = arena.handle_at(index).expect("refreshed entry handle");
        assert_eq!(refreshed.tag(), 3);
        assert_eq!(refreshed.selector(), DONE);
        assert_eq!(arena.get(refreshed).expect("refreshed get").0, index as u64 + 100);
    }
}

#[test]
fn retained_page_reset_invalidates_old_page_capabilities() {
    let mut arena = PagedKeyedArena::<Cell, LOW_BITS, PAGE_SIZE>::with_key(
        Key::from_seed(0x67_20),
    );
    arena
        .insert(Cell(1), 1, READY, prophecy(false))
        .expect("insert");
    let old_page = arena.page_handle_at(0).expect("old page capability");
    arena.reset_retain_pages();
    assert!(arena.validate_page_handle(old_page).is_err());

    arena
        .insert(Cell(2), 1, READY, prophecy(false))
        .expect("insert after reset");
    let new_page = arena.page_handle_at(0).expect("new page capability");
    assert_ne!(old_page.generation(), new_page.generation());
    assert!(arena.validate_page_handle(new_page).is_ok());

    let forged = new_page.with_generation(new_page.generation().wrapping_add(1));
    assert!(arena.validate_page_handle(forged).is_err());
}
