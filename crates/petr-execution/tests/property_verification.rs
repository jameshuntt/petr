use petr_affinity::{CpuId, CpuSet, Placement};
use petr_core::{Key, PortHint};
use petr_execution::*;
use petr_resource::{
    ChunkClass, ResourceKind, ResourceManager, ResourceOp, ResourcePool, ResourceSelector,
};
use petr_temporal::{Tick, Window};
use proptest::prelude::*;

fn always() -> Window {
    Window::new(1, 1, 0).unwrap()
}

fn ram(chunks: u32) -> ResourceNeed {
    ResourceNeed {
        selector: ResourceSelector {
            kind: ResourceKind::Ram,
            class: ChunkClass::K4,
            op: ResourceOp::Allocate,
            hint: PortHint::Load,
        },
        chunks,
    }
}

fn single_handler_plan(resource: Option<ResourceNeed>) -> ExecutionPlan {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 7,
        name: "property",
        window: always(),
        placement: Placement::Require(CpuId(0)),
        obligation: Obligation::Required,
        resource,
    })
    .unwrap();
    plan
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    #[test]
    fn fresh_epochs_change_execution_identity_without_changing_assignment(
        left in any::<u64>(),
        right in any::<u64>(),
        tick in 0u64..4096,
    ) {
        prop_assume!(left != right);
        let plan = single_handler_plan(None);
        let mut resources = ResourceManager::<4>::with_key(
            40,
            ResourcePool::new(0, 0),
            Key::<4>::from_seed(0xA11CE),
        );

        let first = {
            let admitted = plan.admit_in_epoch(&mut resources, Tick(tick), AdmissionEpoch::new(left)).unwrap();
            let lease = admitted.lease(7).unwrap();
            (lease.context(), lease.cpu())
        };
        let second = {
            let admitted = plan.admit_in_epoch(&mut resources, Tick(tick), AdmissionEpoch::new(right)).unwrap();
            let lease = admitted.lease(7).unwrap();
            (lease.context(), lease.cpu())
        };

        prop_assert_eq!(first.0.handler_id, second.0.handler_id);
        prop_assert_eq!(first.0.tick, second.0.tick);
        prop_assert_ne!(first.0.epoch, second.0.epoch);
        prop_assert_eq!(first.1, second.1);
    }

    #[test]
    fn effect_identity_is_stable_across_arbitrary_re_admissions(
        effect in any::<[u8; 32]>(),
        left in any::<u64>(),
        right in any::<u64>(),
    ) {
        prop_assume!(left != right);
        let mut plan = single_handler_plan(None);
        let expected = EffectIdentity::from_bytes(effect);
        plan.set_effect_identity(7, expected).unwrap();

        let mut resources = ResourceManager::<4>::with_key(
            41,
            ResourcePool::new(0, 0),
            Key::<4>::from_seed(0xEFFE_C7),
        );

        let first = {
            let admitted = plan.admit_in_epoch(&mut resources, Tick(3), AdmissionEpoch::new(left)).unwrap();
            admitted.lease(7).unwrap().effect_identity()
        };
        let second = {
            let admitted = plan.admit_in_epoch(&mut resources, Tick(3), AdmissionEpoch::new(right)).unwrap();
            admitted.lease(7).unwrap().effect_identity()
        };

        prop_assert_eq!(first, Some(expected));
        prop_assert_eq!(second, Some(expected));
    }

    #[test]
    fn terminal_transition_is_one_way_and_resource_is_reclaimed_exactly_once(
        epoch in any::<u64>(),
        terminal in 0u8..3,
    ) {
        let plan = single_handler_plan(Some(ram(1)));
        let mut resources = ResourceManager::<4>::with_key(
            42,
            ResourcePool::new(1, 0),
            Key::<4>::from_seed(0x7E12_11A1),
        );

        let expected = match terminal {
            0 => ExecutionAuthorityStatus::Completed,
            1 => ExecutionAuthorityStatus::Cancelled,
            _ => ExecutionAuthorityStatus::Revoked,
        };

        {
            let mut admitted = plan.admit_in_epoch(&mut resources, Tick(9), AdmissionEpoch::new(epoch)).unwrap();
            let authority = admitted.lease(7).unwrap().authority();

            match terminal {
                0 => admitted.complete_handler(7).unwrap(),
                1 => admitted.cancel_handler(7).unwrap(),
                _ => admitted.revoke_handler(7).unwrap(),
            }

            prop_assert_eq!(authority.status(), expected);
            prop_assert!(authority.status().is_terminal());
            prop_assert!(admitted.handler(7).unwrap().resource.is_none());

            let second = admitted.complete_handler(7).unwrap_err();
            let is_expected_terminal = matches!(
                second,
                ExecutionError::AuthorityTerminal { handler_id: 7, status } if status == expected
            );
            prop_assert!(is_expected_terminal);
        }

        prop_assert_eq!(resources.pool().ram_chunks, 1);
    }

    #[test]
    fn one_handler_terminal_transition_never_changes_a_sibling(
        terminal in 0u8..3,
        epoch in any::<u64>(),
    ) {
        let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
        for id in [1, 2] {
            plan.insert(HandlerSpec {
                id,
                name: "sibling",
                window: always(),
                placement: Placement::Any,
                obligation: Obligation::Required,
                resource: None,
            }).unwrap();
        }

        let mut resources = ResourceManager::<4>::with_key(
            43,
            ResourcePool::new(0, 0),
            Key::<4>::from_seed(0x5151),
        );
        let mut admitted = plan.admit_in_epoch(&mut resources, Tick(0), AdmissionEpoch::new(epoch)).unwrap();
        let left = admitted.lease(1).unwrap().authority();
        let right = admitted.lease(2).unwrap().authority();

        match terminal {
            0 => admitted.complete_handler(1).unwrap(),
            1 => admitted.cancel_handler(1).unwrap(),
            _ => admitted.revoke_handler(1).unwrap(),
        }

        prop_assert!(left.status().is_terminal());
        prop_assert_eq!(right.status(), ExecutionAuthorityStatus::Active);
    }
}
