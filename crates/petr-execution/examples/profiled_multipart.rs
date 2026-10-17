use petr_affinity::{CpuId, CpuSet, Placement};
use petr_core::Key;
use petr_execution::{ExecutionPlan, ExecutionProfile, HandlerSpec, Obligation};
use petr_resource::{ResourceManager, ResourcePool};
use petr_temporal::{Tick, Window};
use scope::QoSClass;

fn main() {
    let mut resources =
        ResourceManager::<4>::with_key(1, ResourcePool::new(2, 0), Key::<4>::from_seed(9));
    let mut plan = ExecutionPlan::new(CpuSet::from_count(2).unwrap());

    plan.insert(HandlerSpec {
        id: 1,
        name: "prepare",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Require(CpuId(0)),
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();

    plan.insert(HandlerSpec {
        id: 2,
        name: "publish",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Prefer(CpuId(0)),
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();

    plan.add_dependency(2, 1).unwrap();
    plan.set_once(1).unwrap();
    plan.transition_profile(
        2,
        Tick(4),
        ExecutionProfile::new(
            Placement::Prefer(CpuId(1)),
            Obligation::Required,
            None,
            QoSClass::Realtime,
        ),
    )
    .unwrap();

    let mut state = plan.state();
    let prepare = plan
        .admit_with_state(&mut resources, Tick(0), &state)
        .unwrap();
    assert!(prepare.handler(1).is_some());
    assert!(prepare.handler(2).is_none());
    drop(prepare);

    state.mark_completed(1);
    let publish = plan
        .admit_with_state(&mut resources, Tick(4), &state)
        .unwrap();
    let handler = publish.lease(2).unwrap();
    assert_eq!(handler.cpu(), CpuId(1));
    assert_eq!(handler.scheduler_config().default_qos, QoSClass::Realtime);
}
