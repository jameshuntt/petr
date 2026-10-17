#[cfg(feature = "dispatch")]
#[test]
fn temporal_dispatch_is_optional_and_does_not_mutate_when_closed() {
    use petr_dispatch::{OpArg, OpFrame, OperationTable};
    use petr_execution::dispatch_bridge::{DispatchOutcome, execute_if_eligible};
    use petr_temporal::{Tick, Window};

    fn add(mut frame: OpFrame, arg: &OpArg) -> petr_dispatch::DispatchResult<OpFrame> {
        frame.acc += arg.n;
        Ok(frame)
    }

    let mut table = OperationTable::new();
    table.register(7, "add", add, OpArg::n(5));
    let even = Window::new(2, 0b01, 0).unwrap();

    let closed = execute_if_eligible(even, Tick(1), &table, 7, OpFrame::acc(10)).unwrap();
    assert_eq!(closed, DispatchOutcome::Deferred(OpFrame::acc(10)));

    let open = execute_if_eligible(even, Tick(2), &table, 7, OpFrame::acc(10)).unwrap();
    assert_eq!(open, DispatchOutcome::Executed(OpFrame::acc(15)));
}

#[cfg(feature = "waker-protocol")]
#[test]
fn protocol_bridge_maps_qos_without_owning_the_scheduler() {
    use petr_affinity::Placement;
    use petr_execution::protocol_bridge::task_policy;
    use petr_execution::{ExecutionProfile, Obligation};

    let profile = ExecutionProfile::new(
        Placement::Any,
        Obligation::Required,
        None,
        scope::QoSClass::Realtime,
    );
    let policy = task_policy(profile);
    assert_eq!(policy.qos, waker_protocol::QoSClass::Realtime);
}

#[cfg(feature = "observable")]
#[test]
fn observable_bridge_encodes_execution_semantics_in_wake_key() {
    use petr_affinity::Placement;
    use petr_execution::observable_bridge::{ExecutionWakeKind, execution_wake_key};
    use petr_execution::{ExecutionProfile, Obligation};
    use petr_temporal::Tick;

    let profile = ExecutionProfile::new(
        Placement::Any,
        Obligation::Required,
        None,
        scope::QoSClass::Interactive,
    );
    let key = execution_wake_key(ExecutionWakeKind::ProfileTransition, profile, Tick(259)).unwrap();
    assert_eq!(
        key.reason_code().unwrap(),
        ob_serve_a_bowl::WakeReasonCode::Qos
    );
    assert_eq!(key.qos(), 4);
    assert_eq!(key.phase(), 3);
    assert!(key.has_any_field(ExecutionWakeKind::ProfileTransition.field_mask()));
}
