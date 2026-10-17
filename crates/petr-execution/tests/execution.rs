use petr_affinity::{CpuId, CpuSet, Placement};
use petr_core::{Key, PortHint};
use petr_execution::*;
use petr_resource::{
    ChunkClass, ResourceKind, ResourceManager, ResourceOp, ResourcePool, ResourceSelector,
};
use petr_temporal::{Tick, Window};
use scope::{Cx, FnTask, QoSClass, TaskPoll};

fn window(mask: u64) -> Window {
    Window::new(2, mask, 0).unwrap()
}
fn ram() -> ResourceNeed {
    ResourceNeed {
        selector: ResourceSelector {
            kind: ResourceKind::Ram,
            class: ChunkClass::K4,
            op: ResourceOp::Allocate,
            hint: PortHint::Load,
        },
        chunks: 1,
    }
}

#[test]
fn required_work_preempts_opportunistic_cpu_use() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    plan.insert(HandlerSpec {
        id: 2,
        name: "required",
        window: window(1),
        placement: Placement::Require(CpuId(0)),
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.insert(HandlerSpec {
        id: 1,
        name: "spare",
        window: window(1),
        placement: Placement::Prefer(CpuId(0)),
        obligation: Obligation::Opportunistic,
        resource: None,
    })
    .unwrap();
    let a = plan.assignments(Tick(0)).unwrap();
    assert_eq!(a[0].cpu, CpuId(1));
    assert_eq!(a[1].cpu, CpuId(0));
}

#[test]
fn temporal_windows_select_different_handlers() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "even",
        window: window(1),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.insert(HandlerSpec {
        id: 2,
        name: "odd",
        window: window(2),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    assert_eq!(plan.assignments(Tick(0)).unwrap()[0].handler_id, 1);
    assert_eq!(plan.assignments(Tick(1)).unwrap()[0].handler_id, 2);
}

#[test]
fn admitted_resources_live_for_plan_lifetime() {
    let mut manager =
        ResourceManager::<4>::with_key(7, ResourcePool::new(2, 0), Key::<4>::from_seed(5));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "worker",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: Some(ram()),
    })
    .unwrap();
    {
        let admitted = plan.admit(&mut manager, Tick(0)).unwrap();
        assert_eq!(admitted.handlers().len(), 1);
        assert_eq!(admitted.handlers()[0].resource.unwrap().chunks(), 1);
    }
    assert_eq!(manager.pool().ram_chunks, 2);
}

#[test]
fn handler_runs_inside_scope_join_boundary() {
    let mut manager =
        ResourceManager::<4>::with_key(9, ResourcePool::new(1, 0), Key::<4>::from_seed(7));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "scoped",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    let admitted = plan.admit(&mut manager, Tick(0)).unwrap();
    let mut state = Vec::<u32>::new();
    let report = admitted
        .run_scoped(1, &mut state, |s| {
            s.spawn_named(
                "write",
                FnTask(|cx: &mut Cx<'_, '_, Vec<u32>>| {
                    cx.resources.push(42);
                    TaskPoll::Ready
                }),
            );
        })
        .unwrap();
    assert_eq!(state, vec![42]);
    assert_eq!(report.completed, 1);
}

#[test]
fn dependencies_gate_parts_until_completion() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    plan.insert(HandlerSpec {
        id: 10,
        name: "prepare",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.insert(HandlerSpec {
        id: 20,
        name: "publish",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.add_dependency(20, 10).unwrap();
    plan.set_once(10).unwrap();

    let mut state = plan.state();
    let first = plan.assignments_with_state(Tick(0), &state).unwrap();
    assert_eq!(
        first.iter().map(|a| a.handler_id).collect::<Vec<_>>(),
        vec![10]
    );
    assert_eq!(
        plan.inspect(Tick(0), &state)[1].readiness,
        HandlerReadiness::WaitingForDependency
    );

    state.mark_completed(10);
    let second = plan.assignments_with_state(Tick(1), &state).unwrap();
    assert_eq!(
        second.iter().map(|a| a.handler_id).collect::<Vec<_>>(),
        vec![20]
    );
    assert_eq!(
        plan.inspect(Tick(1), &state)[0].readiness,
        HandlerReadiness::Completed
    );
}

#[test]
fn dependency_cycles_are_rejected() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    for id in 1..=3 {
        plan.insert(HandlerSpec {
            id,
            name: "part",
            window: Window::new(1, 1, 0).unwrap(),
            placement: Placement::Any,
            obligation: Obligation::Required,
            resource: None,
        })
        .unwrap();
    }
    plan.add_dependency(2, 1).unwrap();
    plan.add_dependency(3, 2).unwrap();
    assert!(matches!(
        plan.add_dependency(1, 3),
        Err(ExecutionError::DependencyCycle { .. })
    ));
}

#[test]
fn profile_transitions_change_placement_and_qos_at_logical_ticks() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "agent",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Require(CpuId(0)),
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.transition_profile(
        1,
        Tick(4),
        ExecutionProfile::new(
            Placement::Require(CpuId(1)),
            Obligation::Required,
            None,
            QoSClass::Realtime,
        ),
    )
    .unwrap();

    assert_eq!(plan.assignments(Tick(3)).unwrap()[0].cpu, CpuId(0));
    assert_eq!(plan.assignments(Tick(4)).unwrap()[0].cpu, CpuId(1));
    assert_eq!(plan.profile_at(1, Tick(4)).unwrap().qos, QoSClass::Realtime);
}

#[test]
fn handler_lease_carries_profile_into_scope_config() {
    let mut manager =
        ResourceManager::<4>::with_key(11, ResourcePool::new(1, 0), Key::<4>::from_seed(8));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "interactive",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.set_profile(
        1,
        ExecutionProfile::new(
            Placement::Any,
            Obligation::Required,
            None,
            QoSClass::Interactive,
        ),
    )
    .unwrap();

    let admitted = plan.admit(&mut manager, Tick(0)).unwrap();
    let lease = admitted.lease(1).unwrap();
    assert_eq!(lease.scheduler_config().default_qos, QoSClass::Interactive);
}

#[test]
fn same_plan_and_state_produce_same_admission_trace() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(3).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "a",
        window: Window::new(2, 0b01, 0).unwrap(),
        placement: Placement::Require(CpuId(0)),
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.insert(HandlerSpec {
        id: 2,
        name: "b",
        window: Window::new(2, 0b11, 0).unwrap(),
        placement: Placement::Prefer(CpuId(0)),
        obligation: Obligation::Opportunistic,
        resource: None,
    })
    .unwrap();
    plan.insert(HandlerSpec {
        id: 3,
        name: "c",
        window: Window::new(2, 0b10, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();

    let state = plan.state();
    let trace = || {
        (0..32)
            .map(|tick| {
                plan.assignments_with_state(Tick(tick), &state)
                    .unwrap()
                    .into_iter()
                    .map(|a| (a.handler_id, a.cpu.0))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(trace(), trace());
}

#[test]
fn protection_capabilities_gate_required_and_opportunistic_work() {
    use petr_protection::{ProtectionCapabilities, ProtectionRequirement};

    let allowed = CpuSet::new(vec![CpuId(0), CpuId(1)]).unwrap();
    let mut plan = ExecutionPlan::new(allowed);
    plan.insert(HandlerSpec {
        id: 70,
        name: "required-protected",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.insert(HandlerSpec {
        id: 71,
        name: "optional-protected",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Opportunistic,
        resource: None,
    })
    .unwrap();

    plan.set_profile(
        70,
        ExecutionProfile::new(
            Placement::Any,
            Obligation::Required,
            None,
            QoSClass::Standard,
        )
        .with_protection(ProtectionRequirement::Authentication),
    )
    .unwrap();
    plan.set_profile(
        71,
        ExecutionProfile::new(
            Placement::Any,
            Obligation::Opportunistic,
            None,
            QoSClass::Standard,
        )
        .with_protection(ProtectionRequirement::MemoryTagging),
    )
    .unwrap();

    let state = plan.state();
    assert!(matches!(
        plan.assignments_with_capabilities(Tick(0), &state, ProtectionCapabilities::NONE),
        Err(ExecutionError::ProtectionUnavailable {
            handler_id: 70,
            requirement: ProtectionRequirement::Authentication,
        })
    ));

    let only_auth = plan
        .assignments_with_capabilities(Tick(0), &state, ProtectionCapabilities::AUTHENTICATION)
        .unwrap();
    assert_eq!(
        only_auth.iter().map(|a| a.handler_id).collect::<Vec<_>>(),
        vec![70]
    );

    let all = plan
        .assignments_with_capabilities(Tick(0), &state, ProtectionCapabilities::ALL)
        .unwrap();
    assert_eq!(
        all.iter().map(|a| a.handler_id).collect::<Vec<_>>(),
        vec![70, 71]
    );
}

#[test]
fn protection_requirement_can_transition_with_the_execution_profile() {
    use petr_protection::{ProtectionCapabilities, ProtectionRequirement};

    let allowed = CpuSet::new(vec![CpuId(0)]).unwrap();
    let mut plan = ExecutionPlan::new(allowed);
    plan.insert(HandlerSpec {
        id: 72,
        name: "evolving-protection",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.transition_profile(
        72,
        Tick(10),
        ExecutionProfile::new(
            Placement::Any,
            Obligation::Required,
            None,
            QoSClass::Interactive,
        )
        .with_protection(ProtectionRequirement::AuthenticationAndMemoryTagging),
    )
    .unwrap();

    let state = plan.state();
    assert_eq!(
        plan.profile_at(72, Tick(9)).unwrap().protection,
        ProtectionRequirement::None
    );
    assert_eq!(
        plan.profile_at(72, Tick(10)).unwrap().protection,
        ProtectionRequirement::AuthenticationAndMemoryTagging,
    );
    assert!(
        plan.assignments_with_capabilities(Tick(9), &state, ProtectionCapabilities::NONE)
            .is_ok()
    );
    assert!(matches!(
        plan.assignments_with_capabilities(
            Tick(10),
            &state,
            ProtectionCapabilities::AUTHENTICATION
        ),
        Err(ExecutionError::ProtectionUnavailable { handler_id: 72, .. })
    ));
}

#[test]
fn default_admission_uses_the_legacy_zero_epoch() {
    use petr_execution::AdmissionEpoch;

    let mut manager = ResourceManager::<4>::new_randomized(140, ResourcePool::new(0, 0));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 80,
        name: "epoch-default",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();

    let admitted = plan.admit(&mut manager, Tick(9)).unwrap();
    assert_eq!(admitted.epoch(), AdmissionEpoch::DEFAULT);
    let lease = admitted.lease(80).unwrap();
    assert_eq!(lease.epoch(), AdmissionEpoch::DEFAULT);
    assert_eq!(lease.context().epoch, AdmissionEpoch::DEFAULT);
    assert_eq!(lease.context().tick, Tick(9));
    assert_eq!(lease.context().handler_id, 80);
}

#[test]
fn explicit_admission_epochs_isolate_replays_without_changing_assignment() {
    use petr_execution::AdmissionEpoch;

    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 81,
        name: "epoch-replay",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();

    let first_context = {
        let mut manager = ResourceManager::<4>::new_randomized(141, ResourcePool::new(0, 0));
        let admitted = plan
            .admit_in_epoch(&mut manager, Tick(12), AdmissionEpoch(1001))
            .unwrap();
        assert_eq!(admitted.handlers()[0].cpu, CpuId(0));
        admitted.lease(81).unwrap().context()
    };

    let second_context = {
        let mut manager = ResourceManager::<4>::new_randomized(142, ResourcePool::new(0, 0));
        let admitted = plan
            .admit_in_epoch(&mut manager, Tick(12), AdmissionEpoch(1002))
            .unwrap();
        assert_eq!(admitted.handlers()[0].cpu, CpuId(0));
        admitted.lease(81).unwrap().context()
    };

    assert_eq!(first_context.handler_id, second_context.handler_id);
    assert_eq!(first_context.tick, second_context.tick);
    assert_ne!(first_context.epoch, second_context.epoch);
    assert_ne!(first_context, second_context);
}

#[test]
fn execution_authority_is_live_only_for_its_admitted_plan() {
    let mut manager = ResourceManager::<4>::new_randomized(143, ResourcePool::new(0, 0));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 82,
        name: "revocable-authority",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();

    let authority = {
        let admitted = plan
            .admit_in_epoch(&mut manager, Tick(18), AdmissionEpoch(2001))
            .unwrap();
        let lease = admitted.lease(82).unwrap();
        let authority = lease.authority();
        assert!(authority.is_active());
        assert_eq!(authority.context(), lease.context());
        authority
    };

    assert!(!authority.is_active());
    assert_eq!(authority.status(), ExecutionAuthorityStatus::Revoked);
    assert_eq!(authority.context().handler_id, 82);
    assert_eq!(authority.context().tick, Tick(18));
    assert_eq!(authority.context().epoch, AdmissionEpoch(2001));
}

#[test]
fn handler_authorities_are_independent_and_terminal() {
    let mut manager = ResourceManager::<4>::new_randomized(144, ResourcePool::new(0, 0));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    for id in [90, 91] {
        plan.insert(HandlerSpec {
            id,
            name: "independent-authority",
            window: Window::new(1, 1, 0).unwrap(),
            placement: Placement::Any,
            obligation: Obligation::Required,
            resource: None,
        })
        .unwrap();
    }

    let mut admitted = plan
        .admit_in_epoch(&mut manager, Tick(22), AdmissionEpoch(3001))
        .unwrap();
    let first = admitted.lease(90).unwrap().authority();
    let second = admitted.lease(91).unwrap().authority();

    admitted.complete_handler(90).unwrap();
    assert_eq!(first.status(), ExecutionAuthorityStatus::Completed);
    assert!(!first.is_active());
    assert_eq!(second.status(), ExecutionAuthorityStatus::Active);
    assert!(second.is_active());

    assert!(matches!(
        admitted.cancel_handler(90),
        Err(ExecutionError::AuthorityTerminal {
            handler_id: 90,
            status: ExecutionAuthorityStatus::Completed,
        })
    ));

    admitted.cancel_handler(91).unwrap();
    assert_eq!(second.status(), ExecutionAuthorityStatus::Cancelled);
    assert!(!second.is_active());
}

#[test]
fn plan_drop_revokes_only_authorities_that_are_still_active() {
    let mut manager = ResourceManager::<4>::new_randomized(145, ResourcePool::new(0, 0));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    for id in [92, 93] {
        plan.insert(HandlerSpec {
            id,
            name: "drop-authority",
            window: Window::new(1, 1, 0).unwrap(),
            placement: Placement::Any,
            obligation: Obligation::Required,
            resource: None,
        })
        .unwrap();
    }

    let (completed, active) = {
        let mut admitted = plan.admit(&mut manager, Tick(23)).unwrap();
        let completed = admitted.lease(92).unwrap().authority();
        let active = admitted.lease(93).unwrap().authority();
        admitted.complete_handler(92).unwrap();
        (completed, active)
    };

    assert_eq!(completed.status(), ExecutionAuthorityStatus::Completed);
    assert_eq!(active.status(), ExecutionAuthorityStatus::Revoked);
}

#[test]
fn completion_can_atomically_advance_plan_progress_and_release_handler_resource() {
    let mut manager =
        ResourceManager::<4>::with_key(146, ResourcePool::new(2, 0), Key::<4>::from_seed(19));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    plan.insert(HandlerSpec {
        id: 94,
        name: "prepare",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: Some(ram()),
    })
    .unwrap();
    plan.insert(HandlerSpec {
        id: 95,
        name: "publish",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: Some(ram()),
    })
    .unwrap();
    plan.add_dependency(95, 94).unwrap();
    plan.set_once(94).unwrap();

    let mut state = plan.state();
    let mut admitted = plan
        .admit_with_state(&mut manager, Tick(24), &state)
        .unwrap();
    assert_eq!(admitted.handlers().len(), 1);
    assert!(admitted.handler(94).unwrap().resource.is_some());

    admitted.complete_handler_and_mark(94, &mut state).unwrap();
    assert_eq!(
        admitted.authority_status(94).unwrap(),
        ExecutionAuthorityStatus::Completed
    );
    assert!(admitted.handler(94).unwrap().resource.is_none());
    drop(admitted);

    let second = plan.assignments_with_state(Tick(25), &state).unwrap();
    assert_eq!(
        second
            .iter()
            .map(|assignment| assignment.handler_id)
            .collect::<Vec<_>>(),
        vec![95]
    );
}

#[test]
fn admission_epoch_names_one_concrete_attempt_without_affecting_assignment() {
    let first = AdmissionEpoch::new(4101);
    let second = AdmissionEpoch::new(4102);
    assert_eq!(first.value(), 4101);
    assert!(!first.is_default());
    assert!(AdmissionEpoch::DEFAULT.is_default());
    assert_ne!(first, second);

    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 101,
        name: "attempt-identity",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();

    let first_context = {
        let mut manager = ResourceManager::<4>::new_randomized(201, ResourcePool::new(0, 0));
        let admitted = plan.admit_in_epoch(&mut manager, Tick(31), first).unwrap();
        assert_eq!(admitted.handlers()[0].cpu, CpuId(0));
        admitted.lease(101).unwrap().context()
    };
    let second_context = {
        let mut manager = ResourceManager::<4>::new_randomized(202, ResourcePool::new(0, 0));
        let admitted = plan.admit_in_epoch(&mut manager, Tick(31), second).unwrap();
        assert_eq!(admitted.handlers()[0].cpu, CpuId(0));
        admitted.lease(101).unwrap().context()
    };

    assert_eq!(first_context.handler_id, second_context.handler_id);
    assert_eq!(first_context.tick, second_context.tick);
    assert_ne!(first_context.epoch, second_context.epoch);
}

#[test]
fn effect_identity_survives_re_admission_while_execution_identity_changes() {
    let effect = EffectIdentity::from_bytes([0xA5; 32]);
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 102,
        name: "stable-effect",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan.set_effect_identity(102, effect).unwrap();
    assert_eq!(plan.effect_identity(102).unwrap(), Some(effect));

    let first = {
        let mut manager = ResourceManager::<4>::new_randomized(203, ResourcePool::new(0, 0));
        let admitted = plan
            .admit_in_epoch(&mut manager, Tick(32), AdmissionEpoch::new(5001))
            .unwrap();
        let lease = admitted.lease(102).unwrap();
        assert_eq!(lease.effect_identity(), Some(effect));
        (lease.context(), lease.effect_identity())
    };
    let second = {
        let mut manager = ResourceManager::<4>::new_randomized(204, ResourcePool::new(0, 0));
        let admitted = plan
            .admit_in_epoch(&mut manager, Tick(32), AdmissionEpoch::new(5002))
            .unwrap();
        let lease = admitted.lease(102).unwrap();
        (lease.context(), lease.effect_identity())
    };

    assert_ne!(first.0, second.0);
    assert_eq!(first.1, second.1);
    assert_eq!(first.1, Some(effect));

    plan.clear_effect_identity(102).unwrap();
    assert_eq!(plan.effect_identity(102).unwrap(), None);
}

#[test]
fn revoke_ends_only_the_current_admission_and_releases_its_resource() {
    let effect = EffectIdentity::from_bytes([0x5A; 32]);
    let mut manager =
        ResourceManager::<4>::with_key(205, ResourcePool::new(1, 0), Key::<4>::from_seed(33));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 103,
        name: "deferrable",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: Some(ram()),
    })
    .unwrap();
    plan.set_effect_identity(103, effect).unwrap();

    let old_authority = {
        let mut admitted = plan
            .admit_in_epoch(&mut manager, Tick(33), AdmissionEpoch::new(6001))
            .unwrap();
        let authority = admitted.lease(103).unwrap().authority();
        assert!(authority.is_active());
        assert!(admitted.handler(103).unwrap().resource.is_some());

        admitted.revoke_handler(103).unwrap();
        assert_eq!(
            admitted.authority_status(103).unwrap(),
            ExecutionAuthorityStatus::Revoked
        );
        assert!(admitted.handler(103).unwrap().resource.is_none());
        authority
    };

    assert_eq!(manager.pool().ram_chunks, 1);
    assert_eq!(old_authority.status(), ExecutionAuthorityStatus::Revoked);

    let admitted = plan
        .admit_in_epoch(&mut manager, Tick(33), AdmissionEpoch::new(6002))
        .unwrap();
    let new_lease = admitted.lease(103).unwrap();
    assert!(new_lease.authority().is_active());
    assert_eq!(new_lease.effect_identity(), Some(effect));
    assert_ne!(old_authority.context(), new_lease.context());
    assert_eq!(old_authority.status(), ExecutionAuthorityStatus::Revoked);
}

#[test]
fn terminal_authority_statuses_are_explicitly_terminal() {
    assert!(!ExecutionAuthorityStatus::Active.is_terminal());
    assert!(ExecutionAuthorityStatus::Completed.is_terminal());
    assert!(ExecutionAuthorityStatus::Cancelled.is_terminal());
    assert!(ExecutionAuthorityStatus::Revoked.is_terminal());
}
