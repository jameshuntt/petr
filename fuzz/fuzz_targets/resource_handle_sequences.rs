#![no_main]
use libfuzzer_sys::fuzz_target;
use petr_core::{Key, PortHint};
use petr_resource::{
    ChunkClass, ResourceKind, ResourceManager, ResourceOp, ResourcePool, ResourceSelector,
};

fn selector(byte: u8) -> ResourceSelector {
    ResourceSelector {
        kind: if byte & 1 == 0 { ResourceKind::Ram } else { ResourceKind::Storage },
        class: ChunkClass::K4,
        op: ResourceOp::Allocate,
        hint: PortHint::Load,
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 { return; }

    let ram = 1 + (data[0] as u32 % 8);
    let storage = 1 + (data[1] as u32 % 8);
    let mut manager = ResourceManager::<4>::with_key(
        202,
        ResourcePool::new(ram, storage),
        Key::<4>::from_seed(0xF033),
    );

    let sel = selector(data[2]);
    let available = if matches!(sel.kind, ResourceKind::Ram) { ram } else { storage };
    let chunks = 1 + (data[3] as u32 % available);
    let handle = manager.allocate(sel, chunks).unwrap();
    assert!(manager.validate(handle).is_ok());

    let forged = handle.with_generation(handle.generation().wrapping_add(1));
    assert!(manager.validate(forged).is_err());

    manager.release(handle).unwrap();
    assert!(manager.validate(handle).is_err());
});
