use petr_temporal::*;
use std::num::NonZeroUsize;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn w(period: u8, mask: u64) -> Window {
    Window::new(period, mask, 0).unwrap()
}

#[test]
fn windows_repeat() {
    let x = w(4, 0b0101);
    for t in 0..100 {
        assert_eq!(x.eligible(Tick(t)), x.eligible(Tick(t + 4)));
    }
}
#[test]
fn windows_validate() {
    assert!(matches!(
        Window::new(0, 1, 0),
        Err(TemporalError::InvalidPeriod)
    ));
    assert!(matches!(
        Window::new(4, 0, 0),
        Err(TemporalError::EmptyWindow)
    ));
    assert!(matches!(
        Window::new(4, 0x10, 0),
        Err(TemporalError::BitsOutsidePeriod)
    ));
}
#[test]
fn phase_clock_maps_slots_cycles_and_phases() {
    let c = PhaseClock::started_at(
        Instant::now(),
        Duration::from_millis(10),
        NonZeroUsize::new(4).unwrap(),
    )
    .unwrap();
    let t = c.tick_at_elapsed(Duration::from_millis(95));
    assert_eq!(t.slot_index, 9);
    assert_eq!(t.cycle_index, 2);
    assert_eq!(t.phase, PhaseId(1));
}
#[test]
fn cyclic_executor_fires_once_per_slot() {
    let clock = PhaseClock::new(Duration::from_millis(10), NonZeroUsize::new(2).unwrap()).unwrap();
    let hits = Arc::new(Mutex::new(Vec::new()));
    let h = hits.clone();
    let mut exec = CyclicExecutive::with_clock(clock).unwrap();
    exec.add_task(PhaseTask::every_cycle("a", PhaseId(0), move |t| {
        h.lock().unwrap().push(t.slot_index)
    }))
    .unwrap();
    exec.step_at_elapsed(Duration::ZERO).unwrap();
    exec.step_at_elapsed(Duration::ZERO).unwrap();
    exec.step_at_elapsed(Duration::from_millis(20)).unwrap();
    assert_eq!(*hits.lock().unwrap(), vec![0, 2]);
}
#[test]
fn replay_policy_replays_skipped_slots() {
    let clock = PhaseClock::new(Duration::from_millis(1), NonZeroUsize::new(2).unwrap()).unwrap();
    let hits = Arc::new(Mutex::new(Vec::new()));
    let h = hits.clone();
    let mut exec = CyclicExecutive::with_clock(clock).unwrap();
    exec.set_missed_window_policy(MissedWindowPolicy::Replay { max_slots: 8 });
    exec.add_task(PhaseTask::every_cycle("all", PhaseId(0), move |t| {
        h.lock().unwrap().push(t.slot_index)
    }))
    .unwrap();
    exec.step_at_elapsed(Duration::ZERO).unwrap();
    let report = exec.step_at_elapsed(Duration::from_millis(4)).unwrap();
    assert_eq!(report.missed_slots, 3);
    assert_eq!(*hits.lock().unwrap(), vec![0, 2, 4]);
}
#[test]
fn error_policy_detects_skipped_slots() {
    let clock = PhaseClock::new(Duration::from_millis(1), NonZeroUsize::new(2).unwrap()).unwrap();
    let mut exec = CyclicExecutive::with_clock(clock).unwrap();
    exec.set_missed_window_policy(MissedWindowPolicy::Error);
    exec.step_at_elapsed(Duration::ZERO).unwrap();
    assert!(matches!(
        exec.step_at_elapsed(Duration::from_millis(2)),
        Err(TemporalError::MissedSlots { .. })
    ));
}
#[test]
fn temporal_pointer_preserves_provenance_and_checks_phase() {
    #[repr(align(16))]
    struct A(u64);
    let mut value = A(7);
    let ptr = NonNull::from(&mut value);
    let clock = PhaseClock::new(Duration::from_millis(10), NonZeroUsize::new(4).unwrap()).unwrap();
    let authority = TemporalPointerAuthority::<4>::new(clock, 3).unwrap();
    let h = authority.issue_at_elapsed(ptr, Duration::ZERO).unwrap();
    assert_eq!(h.domain(), 3);
    assert_eq!(
        authority.validate_at_elapsed(h, Duration::ZERO).unwrap(),
        ptr
    );
    assert!(matches!(
        authority.validate_at_elapsed(h, Duration::from_millis(10)),
        Err(TemporalError::PhaseMismatch { .. })
    ));
    assert_eq!(value.0, 7);
}
#[test]
fn generation_prevents_phase_revival() {
    #[repr(align(16))]
    struct A(u64);
    let mut value = A(9);
    let clock = PhaseClock::new(Duration::from_millis(10), NonZeroUsize::new(2).unwrap()).unwrap();
    let mut authority = TemporalPointerAuthority::<4>::new(clock, 1).unwrap();
    let h = authority
        .issue_at_elapsed(NonNull::from(&mut value), Duration::ZERO)
        .unwrap();
    authority.transition().unwrap();
    assert!(matches!(
        authority.validate_at_elapsed(h, Duration::from_millis(20)),
        Err(TemporalError::ExpiredAuthority)
    ));
    assert_eq!(value.0, 9);
}

#[test]
fn generation_capability_does_not_revive() {
    let mut a = Authority::new(77);
    let h = a.issue(Window::new(2, 1, 0).unwrap());
    assert_eq!(a.validate(h, Tick(0)), Ok(()));
    assert!(matches!(
        a.validate(h, Tick(1)),
        Err(TemporalError::NotEligible { .. })
    ));
    a.transition().unwrap();
    assert!(matches!(
        a.validate(h, Tick(2)),
        Err(TemporalError::ExpiredAuthority)
    ));
}
