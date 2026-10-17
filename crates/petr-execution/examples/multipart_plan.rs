use petr_affinity::{CpuId, CpuSet, Placement};
use petr_core::{Key, PortHint};
use petr_execution::{ExecutionPlan, HandlerSpec, Obligation, ResourceNeed};
use petr_resource::{
    ChunkClass, ResourceKind, ResourceManager, ResourceOp, ResourcePool, ResourceSelector,
};
use petr_temporal::{Tick, Window};

fn main() {
    let mut resources =
        ResourceManager::<4>::with_key(1, ResourcePool::new(8, 0), Key::<4>::from_seed(1));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "mandatory",
        window: Window::new(4, 0b0101, 0).unwrap(),
        placement: Placement::Require(CpuId(0)),
        obligation: Obligation::Required,
        resource: Some(ResourceNeed {
            selector: ResourceSelector {
                kind: ResourceKind::Ram,
                class: ChunkClass::K4,
                op: ResourceOp::Allocate,
                hint: PortHint::Load,
            },
            chunks: 1,
        }),
    })
    .unwrap();
    plan.insert(HandlerSpec {
        id: 2,
        name: "opportunistic",
        window: Window::new(4, 0b1111, 0).unwrap(),
        placement: Placement::Prefer(CpuId(0)),
        obligation: Obligation::Opportunistic,
        resource: None,
    })
    .unwrap();

    let admitted = plan.admit(&mut resources, Tick(0)).unwrap();
    for h in admitted.handlers() {
        println!("{} -> cpu {}", h.name, h.cpu.0);
    }
}
