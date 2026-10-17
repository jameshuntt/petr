use petr_core::PortHint;
use petr_resource::{
    ChunkClass, ResourceError, ResourceKind, ResourceOp, ResourcePool, ResourceScheduler,
    ResourceSelector,
};

fn selector(kind: ResourceKind, class: ChunkClass, op: ResourceOp) -> ResourceSelector {
    ResourceSelector {
        kind,
        class,
        op,
        hint: PortHint::Dispatch,
    }
}

#[test]
fn chunk_classes_report_exact_byte_sizes() {
    assert_eq!(ChunkClass::K4.bytes(), 4 * 1024);
    assert_eq!(ChunkClass::K64.bytes(), 64 * 1024);
    assert_eq!(ChunkClass::M2.bytes(), 2 * 1024 * 1024);
}

#[test]
fn resource_selector_packs_kind_class_and_op_into_4_4_8_word() {
    let s = selector(ResourceKind::Storage, ChunkClass::M2, ResourceOp::Commit);
    let word = s.selector_word();
    assert_eq!(word.trait_id(), ResourceKind::Storage as u8);
    assert_eq!(word.impl_id(), ChunkClass::M2 as u8);
    assert_eq!(word.function_id(), ResourceOp::Commit as u8);
}

#[test]
fn ram_allocate_grow_release_and_shrink_update_pool_exactly() {
    let mut pool = ResourcePool::new(10, 20);

    let ev = ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::Ram, ChunkClass::K64, ResourceOp::Allocate),
        3,
    )
    .unwrap();
    assert_eq!(pool.ram_chunks, 7);
    assert!(ev.detail.contains("allocated 3 RAM chunks"));
    assert!(ev.detail.contains("65536"));

    ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::Ram, ChunkClass::K4, ResourceOp::Grow),
        2,
    )
    .unwrap();
    assert_eq!(pool.ram_chunks, 5);

    ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::Ram, ChunkClass::K4, ResourceOp::Release),
        4,
    )
    .unwrap();
    assert_eq!(pool.ram_chunks, 9);

    ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::Ram, ChunkClass::K4, ResourceOp::Shrink),
        1,
    )
    .unwrap();
    assert_eq!(pool.ram_chunks, 10);
}

#[test]
fn storage_allocate_and_release_are_independent_of_ram() {
    let mut pool = ResourcePool::new(5, 8);

    ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::Storage, ChunkClass::M2, ResourceOp::Allocate),
        6,
    )
    .unwrap();
    assert_eq!(pool.storage_chunks, 2);
    assert_eq!(pool.ram_chunks, 5);

    ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::Storage, ChunkClass::K4, ResourceOp::Release),
        3,
    )
    .unwrap();
    assert_eq!(pool.storage_chunks, 5);
    assert_eq!(pool.ram_chunks, 5);
}

#[test]
fn commit_operations_accumulate_file_patch_and_network_chunks() {
    let mut pool = ResourcePool::new(1, 1);

    ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::FilePatch, ChunkClass::K4, ResourceOp::Commit),
        2,
    )
    .unwrap();
    ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::FilePatch, ChunkClass::K4, ResourceOp::Commit),
        3,
    )
    .unwrap();
    ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::Network, ChunkClass::K64, ResourceOp::Commit),
        4,
    )
    .unwrap();

    assert_eq!(pool.file_patch_chunks, 5);
    assert_eq!(pool.network_chunks, 4);
    assert_eq!(pool.ram_chunks, 1);
    assert_eq!(pool.storage_chunks, 1);
}

#[test]
fn insufficient_chunks_return_precise_error_without_mutating_pool() {
    let mut pool = ResourcePool::new(2, 3);
    let before = pool.clone();

    let err = ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::Ram, ChunkClass::K4, ResourceOp::Allocate),
        4,
    )
    .unwrap_err();
    assert_eq!(
        err,
        ResourceError::NotEnough {
            kind: ResourceKind::Ram,
            requested: 4,
            available: 2
        }
    );
    assert_eq!(pool, before);

    let err = ResourceScheduler::dispatch(
        &mut pool,
        selector(ResourceKind::Storage, ChunkClass::K4, ResourceOp::Grow),
        9,
    )
    .unwrap_err();
    assert_eq!(
        err,
        ResourceError::NotEnough {
            kind: ResourceKind::Storage,
            requested: 9,
            available: 3
        }
    );
    assert_eq!(pool, before);
}

#[test]
fn unsupported_kind_op_combinations_are_noop_events() {
    let mut pool = ResourcePool::new(2, 3);
    let before = pool.clone();
    let ev = ResourceScheduler::dispatch(
        &mut pool,
        selector(
            ResourceKind::FilePatch,
            ChunkClass::K4,
            ResourceOp::Allocate,
        ),
        99,
    )
    .unwrap();

    assert_eq!(pool, before);
    assert!(ev.detail.contains("no-op"));
}

use petr_core::Key;
use petr_resource::ResourceManager;

#[test]
fn resource_manager_issues_opaque_generation_checked_handles() {
    let key = Key::<4>::from_seed(0xABCD);
    let mut manager = ResourceManager::<4>::with_key(7, ResourcePool::new(16, 32), key);
    let selector = ResourceSelector {
        kind: ResourceKind::Ram,
        class: ChunkClass::K64,
        op: ResourceOp::Allocate,
        hint: PortHint::Load,
    };

    let handle = manager.allocate(selector, 4).unwrap();
    assert_eq!(handle.pool_id(), 7);
    assert_eq!(handle.chunks(), 4);
    assert_eq!(handle.selector(), selector.selector_word());
    assert_eq!(manager.pool().ram_chunks, 12);
    assert!(manager.validate(handle).is_ok());

    let stale = handle.with_generation(handle.generation().wrapping_add(1));
    assert!(manager.validate(stale).is_err());

    let event = manager.commit(handle).unwrap();
    assert_eq!(event.selector, selector.selector_word());
    assert!(event.detail.contains("committed"));

    manager.release(handle).unwrap();
    assert_eq!(manager.pool().ram_chunks, 16);
    assert!(manager.validate(handle).is_err());
}

#[test]
fn resource_manager_reuses_slots_with_new_generations() {
    let key = Key::<4>::from_seed(0x11);
    let mut manager = ResourceManager::<4>::with_key(1, ResourcePool::new(4, 4), key);
    let selector = ResourceSelector {
        kind: ResourceKind::Storage,
        class: ChunkClass::K4,
        op: ResourceOp::Allocate,
        hint: PortHint::Store,
    };

    let first = manager.allocate(selector, 1).unwrap();
    manager.release(first).unwrap();
    let second = manager.allocate(selector, 1).unwrap();

    assert_eq!(first.index(), second.index());
    assert_ne!(first.generation(), second.generation());
    assert!(manager.validate(first).is_err());
    assert!(manager.validate(second).is_ok());
}
