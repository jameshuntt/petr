use petr_affinity::{CpuId, CpuSet, Placement};
use petr_core::{Key, PortHint};
use petr_execution::*;
use petr_resource::{
    ChunkClass, ResourceKind, ResourceManager, ResourceOp, ResourcePool, ResourceSelector,
};
use petr_temporal::{Tick, Window};
use std::sync::{Arc, Barrier};

fn plan_with_resource() -> ExecutionPlan {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "hostile",
        window: Window::new(1, 1, 0).unwrap(),
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
    plan.set_effect_identity(1, EffectIdentity::from_bytes([0xA5; 32]))
        .unwrap();
    plan
}

#[test]
fn ten_thousand_re_admissions_never_revive_old_authority_or_leak_resource() {
    let plan = plan_with_resource();
    let mut resources = ResourceManager::<4>::with_key(
        81,
        ResourcePool::new(1, 0),
        Key::<4>::from_seed(0xCAFE_BABE),
    );

    let mut prior = Vec::new();
    for epoch in 1u64..=10_000 {
        {
            let mut admitted = plan
                .admit_in_epoch(&mut resources, Tick(4), AdmissionEpoch::new(epoch))
                .unwrap();
            let lease = admitted.lease(1).unwrap();
            assert_eq!(lease.context().epoch, AdmissionEpoch::new(epoch));
            assert_eq!(
                lease.effect_identity(),
                Some(EffectIdentity::from_bytes([0xA5; 32]))
            );
            let authority = lease.authority();

            admitted.revoke_handler(1).unwrap();
            assert_eq!(authority.status(), ExecutionAuthorityStatus::Revoked);

            if epoch % 1000 == 0 {
                prior.push(authority);
            }
        }
        assert_eq!(resources.pool().ram_chunks, 1);
    }

    assert!(
        prior
            .iter()
            .all(|authority| authority.status() == ExecutionAuthorityStatus::Revoked)
    );
}

#[test]
fn terminal_publication_is_visible_to_concurrent_authority_observers() {
    for terminal in [
        ExecutionAuthorityStatus::Completed,
        ExecutionAuthorityStatus::Cancelled,
        ExecutionAuthorityStatus::Revoked,
    ] {
        let plan = plan_with_resource();
        let mut resources = ResourceManager::<4>::with_key(
            82,
            ResourcePool::new(1, 0),
            Key::<4>::from_seed(0xBADC_0FFE),
        );
        let mut admitted = plan
            .admit_in_epoch(&mut resources, Tick(1), AdmissionEpoch::new(77))
            .unwrap();
        let authority = admitted.lease(1).unwrap().authority();

        let observers = 8usize;
        let gate = Arc::new(Barrier::new(observers + 1));
        let workers = (0..observers)
            .map(|_| {
                let gate = Arc::clone(&gate);
                let authority = authority.clone();
                std::thread::spawn(move || {
                    gate.wait();
                    authority.status()
                })
            })
            .collect::<Vec<_>>();

        match terminal {
            ExecutionAuthorityStatus::Completed => admitted.complete_handler(1).unwrap(),
            ExecutionAuthorityStatus::Cancelled => admitted.cancel_handler(1).unwrap(),
            ExecutionAuthorityStatus::Revoked => admitted.revoke_handler(1).unwrap(),
            ExecutionAuthorityStatus::Active => unreachable!(),
        }
        gate.wait();

        for worker in workers {
            assert_eq!(worker.join().unwrap(), terminal);
        }
        assert_eq!(authority.status(), terminal);
        drop(admitted);
        assert_eq!(
            authority.status(),
            terminal,
            "plan drop must not overwrite a terminal state"
        );
        assert_eq!(resources.pool().ram_chunks, 1);
    }
}

fn complete<'a>(plan: &mut AdmittedPlan<'a, 4>, id: u32) -> Result<(), ExecutionError> {
    plan.complete_handler(id)
}
fn cancel<'a>(plan: &mut AdmittedPlan<'a, 4>, id: u32) -> Result<(), ExecutionError> {
    plan.cancel_handler(id)
}
fn revoke<'a>(plan: &mut AdmittedPlan<'a, 4>, id: u32) -> Result<(), ExecutionError> {
    plan.revoke_handler(id)
}

#[test]
fn terminal_permutations_never_change_the_first_terminal_reason() {
    type TerminalFn = for<'a> fn(&mut AdmittedPlan<'a, 4>, u32) -> Result<(), ExecutionError>;
    let operations: &[(ExecutionAuthorityStatus, TerminalFn)] = &[
        (ExecutionAuthorityStatus::Completed, complete),
        (ExecutionAuthorityStatus::Cancelled, cancel),
        (ExecutionAuthorityStatus::Revoked, revoke),
    ];

    for &(first_status, first) in operations {
        for &(_, second) in operations {
            let plan = plan_with_resource();
            let mut resources = ResourceManager::<4>::with_key(
                83,
                ResourcePool::new(1, 0),
                Key::<4>::from_seed(0x1234_5678),
            );
            {
                let mut admitted = plan
                    .admit_in_epoch(&mut resources, Tick(0), AdmissionEpoch::new(1))
                    .unwrap();
                let authority = admitted.lease(1).unwrap().authority();
                first(&mut admitted, 1).unwrap();
                assert_eq!(authority.status(), first_status);
                assert!(matches!(
                    second(&mut admitted, 1),
                    Err(ExecutionError::AuthorityTerminal { handler_id: 1, status }) if status == first_status
                ));
                assert_eq!(authority.status(), first_status);
            }
            assert_eq!(resources.pool().ram_chunks, 1);
        }
    }
}
