use petr_resource::{ChunkClass, ResourceKind, ResourceOp, ResourcePool, ResourceScheduler, ResourceSelector};
use petr_core::PortHint;

fn main() {
    let mut pool = ResourcePool::new(128, 512);
    let allocate_ram = ResourceSelector { kind: ResourceKind::Ram, class: ChunkClass::K64, op: ResourceOp::Allocate, hint: PortHint::Store };
    let commit_file = ResourceSelector { kind: ResourceKind::FilePatch, class: ChunkClass::K4, op: ResourceOp::Commit, hint: PortHint::Store };
    let a = ResourceScheduler::dispatch(&mut pool, allocate_ram, 4).unwrap();
    let b = ResourceScheduler::dispatch(&mut pool, commit_file, 2).unwrap();
    println!("{:?}", a);
    println!("{:?}", b);
    println!("pool={:?}", pool);
}
