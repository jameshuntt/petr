use petr_affinity::{CpuId, CpuSet, Placement};
use petr_core::{Key, PortHint};
use petr_execution::{
    ExecutionError, ExecutionPlan, ExecutionProfile, HandlerReadiness, HandlerSpec, Obligation,
    ResourceNeed,
};
use petr_resource::{
    ChunkClass, ResourceError, ResourceKind, ResourceManager, ResourceOp, ResourcePool,
    ResourceSelector,
};
use petr_temporal::{Tick, Window};
use scope::{Cx, FnTask, QoSClass, TaskPoll};
use std::collections::BTreeSet;
use std::sync::Arc;

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

fn handler(
    id: u32,
    name: &'static str,
    window: Window,
    placement: Placement,
    obligation: Obligation,
    resource: Option<ResourceNeed>,
) -> HandlerSpec {
    HandlerSpec {
        id,
        name,
        window,
        placement,
        obligation,
        resource,
    }
}

#[test]
fn duplicate_insert_is_atomic_and_preserves_original_handler() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    plan.insert(handler(
        7,
        "original",
        always(),
        Placement::Require(CpuId(0)),
        Obligation::Required,
        None,
    ))
    .unwrap();

    let duplicate = plan.insert(handler(
        7,
        "replacement",
        always(),
        Placement::Require(CpuId(1)),
        Obligation::Opportunistic,
        None,
    ));
    assert!(matches!(
        duplicate,
        Err(ExecutionError::DuplicateHandler(7))
    ));

    let only = plan.handlers().next().unwrap();
    assert_eq!(only.name, "original");
    assert_eq!(
        plan.profile_at(7, Tick(0)).unwrap().placement,
        Placement::Require(CpuId(0))
    );
    assert_eq!(
        plan.profile_at(7, Tick(0)).unwrap().obligation,
        Obligation::Required
    );
}

#[test]
fn failed_required_admission_rolls_back_all_prior_resources() {
    let mut manager =
        ResourceManager::<4>::with_key(1, ResourcePool::new(2, 0), Key::<4>::from_seed(100));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    plan.insert(handler(
        1,
        "first",
        always(),
        Placement::Any,
        Obligation::Required,
        Some(ram(1)),
    ))
    .unwrap();
    plan.insert(handler(
        2,
        "too-large",
        always(),
        Placement::Any,
        Obligation::Required,
        Some(ram(2)),
    ))
    .unwrap();

    let err = match plan.admit(&mut manager, Tick(0)) {
        Ok(_) => panic!("required admission unexpectedly succeeded"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        ExecutionError::Resource(ResourceError::NotEnough {
            kind: ResourceKind::Ram,
            requested: 2,
            available: 1,
        })
    ));
    assert_eq!(
        manager.pool().ram_chunks,
        2,
        "rollback leaked a prior allocation"
    );
}

#[test]
fn opportunistic_lower_id_cannot_starve_required_resource_admission() {
    let mut manager =
        ResourceManager::<4>::with_key(2, ResourcePool::new(1, 0), Key::<4>::from_seed(101));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());

    // The opportunistic handler intentionally has the lower id. Resource
    // admission must still prioritize the required handler.
    plan.insert(handler(
        1,
        "spare",
        always(),
        Placement::Any,
        Obligation::Opportunistic,
        Some(ram(1)),
    ))
    .unwrap();
    plan.insert(handler(
        2,
        "required",
        always(),
        Placement::Any,
        Obligation::Required,
        Some(ram(1)),
    ))
    .unwrap();

    {
        let admitted = plan.admit(&mut manager, Tick(0)).unwrap();
        let ids = admitted.handlers().iter().map(|h| h.id).collect::<Vec<_>>();
        // Successful admission of only the required handler proves the lower-id
        // opportunistic handler did not consume the sole resource first.
        assert_eq!(ids, vec![2]);
    }
    assert_eq!(manager.pool().ram_chunks, 1);
}

#[test]
fn resource_lease_is_reclaimed_after_scoped_task_panic() {
    let mut manager =
        ResourceManager::<4>::with_key(3, ResourcePool::new(1, 0), Key::<4>::from_seed(102));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(handler(
        1,
        "panic",
        always(),
        Placement::Any,
        Obligation::Required,
        Some(ram(1)),
    ))
    .unwrap();

    {
        let admitted = plan.admit(&mut manager, Tick(0)).unwrap();
        let mut env = ();
        let report = admitted.lease(1).unwrap().run_scoped(&mut env, |s| {
            s.spawn_named(
                "boom",
                FnTask(|_: &mut Cx<'_, '_, ()>| -> TaskPoll {
                    panic!("intentional hardening panic")
                }),
            );
        });
        assert_eq!(report.panicked, 1);
        assert!(!report.all_completed());
    }
    assert_eq!(manager.pool().ram_chunks, 1);
}

#[test]
fn resource_lease_is_reclaimed_after_scoped_task_cancellation() {
    let mut manager =
        ResourceManager::<4>::with_key(4, ResourcePool::new(1, 0), Key::<4>::from_seed(103));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(handler(
        1,
        "cancel",
        always(),
        Placement::Any,
        Obligation::Required,
        Some(ram(1)),
    ))
    .unwrap();

    {
        let admitted = plan.admit(&mut manager, Tick(0)).unwrap();
        let mut env = ();
        let report = admitted.lease(1).unwrap().run_scoped(&mut env, |s| {
            let id = s.spawn_named(
                "cancel-me",
                FnTask(|cx: &mut Cx<'_, '_, ()>| -> TaskPoll { cx.yield_now() }),
            );
            s.cancel_now(id);
        });
        assert_eq!(report.cancelled, 1);
        assert!(!report.all_completed());
    }
    assert_eq!(manager.pool().ram_chunks, 1);
}

#[test]
fn dependency_fan_in_stays_closed_until_every_prerequisite_completes() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(4).unwrap());
    for id in 1..=4 {
        plan.insert(handler(
            id,
            "prerequisite",
            always(),
            Placement::Any,
            Obligation::Required,
            None,
        ))
        .unwrap();
        plan.set_once(id).unwrap();
    }
    plan.insert(handler(
        10,
        "join",
        always(),
        Placement::Any,
        Obligation::Required,
        None,
    ))
    .unwrap();
    for id in 1..=4 {
        plan.add_dependency(10, id).unwrap();
    }

    let mut state = plan.state();
    for id in 1..=3 {
        state.mark_completed(id);
    }
    let join = plan
        .inspect(Tick(0), &state)
        .into_iter()
        .find(|h| h.id == 10)
        .unwrap();
    assert_eq!(join.readiness, HandlerReadiness::WaitingForDependency);

    state.mark_completed(4);
    let assignments = plan.assignments_with_state(Tick(0), &state).unwrap();
    assert_eq!(
        assignments.iter().map(|a| a.handler_id).collect::<Vec<_>>(),
        vec![10]
    );
}

#[test]
fn completed_one_shot_can_be_explicitly_rearmed_in_state() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(handler(
        1,
        "once",
        always(),
        Placement::Any,
        Obligation::Required,
        None,
    ))
    .unwrap();
    plan.set_once(1).unwrap();

    let mut state = plan.state();
    state.mark_completed(1);
    assert!(
        plan.assignments_with_state(Tick(0), &state)
            .unwrap()
            .is_empty()
    );
    state.clear_completed(1);
    assert_eq!(
        plan.assignments_with_state(Tick(0), &state).unwrap()[0].handler_id,
        1
    );
}

#[test]
fn same_tick_profile_transition_replaces_instead_of_stacking() {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());
    plan.insert(handler(
        1,
        "profiled",
        always(),
        Placement::Any,
        Obligation::Required,
        None,
    ))
    .unwrap();

    plan.transition_profile(
        1,
        Tick(5),
        ExecutionProfile::new(
            Placement::Require(CpuId(0)),
            Obligation::Required,
            None,
            QoSClass::Interactive,
        ),
    )
    .unwrap();
    plan.transition_profile(
        1,
        Tick(5),
        ExecutionProfile::new(
            Placement::Require(CpuId(1)),
            Obligation::Required,
            None,
            QoSClass::Realtime,
        ),
    )
    .unwrap();

    assert_eq!(plan.transitions(1).unwrap().len(), 1);
    let profile = plan.profile_at(1, Tick(5)).unwrap();
    assert_eq!(profile.placement, Placement::Require(CpuId(1)));
    assert_eq!(profile.qos, QoSClass::Realtime);
}

fn stress_plan() -> ExecutionPlan {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(8).unwrap());

    // Three required handlers are active in each of eight phases.
    for phase in 0u32..8 {
        let window = Window::new(8, 1u64 << phase, 0).unwrap();
        let base = phase * 10;
        plan.insert(handler(
            base + 1,
            "fixed",
            window,
            Placement::Require(CpuId(phase as usize)),
            Obligation::Required,
            None,
        ))
        .unwrap();
        plan.insert(handler(
            base + 2,
            "preferred",
            window,
            Placement::Prefer(CpuId(phase as usize)),
            Obligation::Required,
            None,
        ))
        .unwrap();
        plan.insert(handler(
            base + 3,
            "any",
            window,
            Placement::Any,
            Obligation::Required,
            None,
        ))
        .unwrap();
    }

    // Five spare handlers fill only CPUs that remain after required work.
    for id in 100..105 {
        plan.insert(handler(
            id,
            "spare",
            always(),
            Placement::Any,
            Obligation::Opportunistic,
            None,
        ))
        .unwrap();
    }
    plan
}

fn trace_digest(plan: &ExecutionPlan, ticks: u64) -> u64 {
    let state = plan.state();
    let mut hash = 0xcbf29ce484222325u64;
    for tick in 0..ticks {
        let assignments = plan.assignments_with_state(Tick(tick), &state).unwrap();
        for assignment in assignments {
            let word = ((tick & 0xffff) << 48)
                ^ ((assignment.handler_id as u64) << 16)
                ^ assignment.cpu.0 as u64;
            hash ^= word;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

#[test]
fn large_schedule_never_double_books_a_cpu() {
    let plan = stress_plan();
    let state = plan.state();
    for tick in 0..16_384u64 {
        let assignments = plan.assignments_with_state(Tick(tick), &state).unwrap();
        assert_eq!(assignments.len(), 8);
        let mut cpus = BTreeSet::new();
        for assignment in assignments {
            assert!(assignment.cpu.0 < 8);
            assert!(
                cpus.insert(assignment.cpu),
                "CPU double-booked at tick {tick}"
            );
        }
    }
}

#[test]
fn immutable_plan_replays_identically_from_multiple_threads() {
    let plan = Arc::new(stress_plan());
    let expected = trace_digest(&plan, 8_192);
    let workers = (0..8)
        .map(|_| {
            let plan = Arc::clone(&plan);
            std::thread::spawn(move || trace_digest(&plan, 8_192))
        })
        .collect::<Vec<_>>();

    for worker in workers {
        assert_eq!(worker.join().unwrap(), expected);
    }
}
