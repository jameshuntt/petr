use petr_affinity::Placement;
use petr_dispatch::{OpArg, OpFrame, OperationTable};
use petr_execution::dispatch_bridge::{DispatchOutcome, execute_if_eligible};
use petr_execution::observable_bridge::{ExecutionWakeKind, execution_wake_key};
use petr_execution::protocol_bridge::task_policy;
use petr_execution::{ExecutionProfile, Obligation};
use petr_temporal::{Tick, Window};
use scope::QoSClass;

fn add(mut frame: OpFrame, arg: &OpArg) -> petr_dispatch::DispatchResult<OpFrame> {
    frame.acc += arg.n;
    Ok(frame)
}

fn main() {
    let even_ticks = Window::new(2, 0b01, 0).unwrap();
    let mut table = OperationTable::new();
    table.register(3, "add", add, OpArg::n(7));

    let closed = execute_if_eligible(even_ticks, Tick(1), &table, 3, OpFrame::acc(10)).unwrap();
    assert_eq!(closed, DispatchOutcome::Deferred(OpFrame::acc(10)));

    let open = execute_if_eligible(even_ticks, Tick(2), &table, 3, OpFrame::acc(10)).unwrap();
    assert_eq!(open, DispatchOutcome::Executed(OpFrame::acc(17)));

    let profile = ExecutionProfile::new(
        Placement::Any,
        Obligation::Required,
        None,
        QoSClass::Interactive,
    );

    let policy = task_policy(profile);
    assert_eq!(policy.qos, waker_protocol::QoSClass::Interactive);

    let key = execution_wake_key(ExecutionWakeKind::TemporalEligible, profile, Tick(258)).unwrap();
    assert_eq!(
        key.reason_code().unwrap(),
        ob_serve_a_bowl::WakeReasonCode::Timer
    );
    assert_eq!(key.qos(), u64::from(QoSClass::Interactive.priority()));
    assert_eq!(key.phase(), 2);
}
