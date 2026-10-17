#![no_main]
use libfuzzer_sys::fuzz_target;
use petr_affinity::{CpuSet, Placement};
use petr_core::{Key, PortHint};
use petr_execution::{
    AdmissionEpoch, EffectIdentity, ExecutionAuthorityStatus, ExecutionError, ExecutionPlan,
    HandlerSpec, Obligation, ResourceNeed,
};
use petr_resource::{
    ChunkClass, ResourceKind, ResourceManager, ResourceOp, ResourcePool, ResourceSelector,
};
use petr_temporal::{Tick, Window};

fuzz_target!(|data: &[u8]| {
    if data.len() < 20 { return; }

    let mut epoch_bytes = [0u8; 8];
    epoch_bytes.copy_from_slice(&data[0..8]);
    let epoch = u64::from_le_bytes(epoch_bytes);

    let mut effect = [0u8; 32];
    for (index, byte) in effect.iter_mut().enumerate() {
        *byte = data[index % data.len()];
    }

    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "fuzz",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
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
    }).unwrap();
    plan.set_effect_identity(1, EffectIdentity::from_bytes(effect)).unwrap();

    let mut resources = ResourceManager::<4>::with_key(
        201,
        ResourcePool::new(1, 0),
        Key::<4>::from_seed(0xF022),
    );

    {
        let mut admitted = plan
            .admit_in_epoch(&mut resources, Tick((data[8] as u64) & 0x0f), AdmissionEpoch::new(epoch))
            .unwrap();
        let authority = admitted.lease(1).unwrap().authority();

        let expected = match data[9] % 3 {
            0 => {
                admitted.complete_handler(1).unwrap();
                ExecutionAuthorityStatus::Completed
            }
            1 => {
                admitted.cancel_handler(1).unwrap();
                ExecutionAuthorityStatus::Cancelled
            }
            _ => {
                admitted.revoke_handler(1).unwrap();
                ExecutionAuthorityStatus::Revoked
            }
        };

        assert_eq!(authority.status(), expected);
        assert!(authority.status().is_terminal());
        assert!(matches!(
            admitted.complete_handler(1),
            Err(ExecutionError::AuthorityTerminal { handler_id: 1, status }) if status == expected
        ));
    }

    assert_eq!(resources.pool().ram_chunks, 1);
});
