//! Temporal execution primitives for PETR.
//!
//! This crate owns time/phase semantics only: logical windows, monotonic phase
//! clocks, cyclic execution policy, missed-window handling, and temporal pointer
//! authority built on PETR's provenance-preserving `SplitTaggedNonNull`.

#![deny(unsafe_code)]

use petr_core::{LowTagLayout, SplitTaggedNonNull, TagError};
use std::num::{NonZeroU8, NonZeroU64, NonZeroUsize};
use std::ptr::NonNull;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PhaseId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    period: NonZeroU8,
    mask: u64,
    phase_offset: u8,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TemporalError {
    #[error("period must be in 1..=64")]
    InvalidPeriod,
    #[error("temporal window cannot be empty")]
    EmptyWindow,
    #[error("window contains bits outside its period")]
    BitsOutsidePeriod,
    #[error("phase count must fit in one byte")]
    PhaseCountTooLarge,
    #[error("temporal authority belongs to another domain")]
    WrongDomain,
    #[error("temporal authority generation has expired")]
    ExpiredAuthority,
    #[error("handle phase {found} does not match current phase {expected}")]
    PhaseMismatch { expected: usize, found: usize },
    #[error("capability is not eligible at logical tick {tick}")]
    NotEligible { tick: u64 },
    #[error("missed cyclic slots: expected next slot {expected}, observed {observed}")]
    MissedSlots { expected: u128, observed: u128 },
    #[error("pointer tag error: {0}")]
    Tag(#[from] TagError),
}

impl Window {
    pub fn new(period: u8, mask: u64, phase_offset: u8) -> Result<Self, TemporalError> {
        if period == 0 || period > 64 {
            return Err(TemporalError::InvalidPeriod);
        }
        if mask == 0 {
            return Err(TemporalError::EmptyWindow);
        }
        if period < 64 && (mask >> period) != 0 {
            return Err(TemporalError::BitsOutsidePeriod);
        }
        Ok(Self {
            period: NonZeroU8::new(period).expect("checked"),
            mask,
            phase_offset: phase_offset % period,
        })
    }
    pub const fn period(self) -> u8 {
        self.period.get()
    }
    pub const fn mask(self) -> u64 {
        self.mask
    }
    pub const fn phase_offset(self) -> u8 {
        self.phase_offset
    }
    #[inline]
    pub fn eligible(self, tick: Tick) -> bool {
        let period = u64::from(self.period.get());
        let phase = ((tick.0 % period + u64::from(self.phase_offset)) % period) as u32;
        ((self.mask >> phase) & 1) != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemporalCapability {
    domain: u64,
    generation: u64,
    window: Window,
}
impl TemporalCapability {
    pub const fn domain(self) -> u64 {
        self.domain
    }
    pub const fn generation(self) -> u64 {
        self.generation
    }
    pub const fn window(self) -> Window {
        self.window
    }
}

#[derive(Debug, Clone)]
pub struct Authority {
    domain: u64,
    generation: u64,
}
impl Authority {
    pub const fn new(domain: u64) -> Self {
        Self {
            domain,
            generation: 1,
        }
    }
    pub const fn domain(&self) -> u64 {
        self.domain
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    pub fn issue(&self, window: Window) -> TemporalCapability {
        TemporalCapability {
            domain: self.domain,
            generation: self.generation,
            window,
        }
    }
    pub fn validate(
        &self,
        capability: TemporalCapability,
        tick: Tick,
    ) -> Result<(), TemporalError> {
        if capability.domain != self.domain {
            return Err(TemporalError::WrongDomain);
        }
        if capability.generation != self.generation {
            return Err(TemporalError::ExpiredAuthority);
        }
        if !capability.window.eligible(tick) {
            return Err(TemporalError::NotEligible { tick: tick.0 });
        }
        Ok(())
    }
    pub fn transition(&mut self) -> Result<(), TemporalError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(TemporalError::ExpiredAuthority)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleTick {
    pub elapsed: Duration,
    pub slot_index: u128,
    pub cycle_index: u128,
    pub phase: PhaseId,
    pub phase_count: usize,
    pub slot_duration: Duration,
}

impl ScheduleTick {
    /// Convert the slot index into the logical tick used by PETR plans when it fits.
    pub fn logical_tick(self) -> Option<Tick> {
        u64::try_from(self.slot_index).ok().map(Tick)
    }
}

#[derive(Debug, Clone)]
pub struct PhaseClock {
    start: Instant,
    slot_duration: Duration,
    phase_count: NonZeroUsize,
}

impl PhaseClock {
    pub fn new(slot_duration: Duration, phase_count: NonZeroUsize) -> Result<Self, TemporalError> {
        Self::started_at(Instant::now(), slot_duration, phase_count)
    }
    pub fn started_at(
        start: Instant,
        slot_duration: Duration,
        phase_count: NonZeroUsize,
    ) -> Result<Self, TemporalError> {
        if slot_duration.is_zero() {
            return Err(TemporalError::InvalidPeriod);
        }
        if phase_count.get() > u8::MAX as usize + 1 {
            return Err(TemporalError::PhaseCountTooLarge);
        }
        Ok(Self {
            start,
            slot_duration,
            phase_count,
        })
    }
    pub fn phase_count(&self) -> usize {
        self.phase_count.get()
    }
    pub fn slot_duration(&self) -> Duration {
        self.slot_duration
    }
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
    pub fn tick(&self) -> ScheduleTick {
        self.tick_at_elapsed(self.elapsed())
    }
    pub fn tick_at_elapsed(&self, elapsed: Duration) -> ScheduleTick {
        let slot_index = elapsed.as_nanos() / self.slot_duration.as_nanos();
        let phase_count = self.phase_count.get();
        let phase = PhaseId((slot_index % phase_count as u128) as usize);
        ScheduleTick {
            elapsed,
            slot_index,
            cycle_index: slot_index / phase_count as u128,
            phase,
            phase_count,
            slot_duration: self.slot_duration,
        }
    }
    pub fn phase_at_elapsed(&self, elapsed: Duration) -> PhaseId {
        self.tick_at_elapsed(elapsed).phase
    }
}

pub type TaskBody = Box<dyn FnMut(ScheduleTick) + Send + 'static>;

pub struct PhaseTask {
    pub name: &'static str,
    pub phase: PhaseId,
    pub every_cycles: NonZeroU64,
    pub cycle_offset: u64,
    last_fired_slot: Option<u128>,
    body: TaskBody,
}
impl PhaseTask {
    pub fn every_cycle(
        name: &'static str,
        phase: PhaseId,
        body: impl FnMut(ScheduleTick) + Send + 'static,
    ) -> Self {
        Self::new(name, phase, NonZeroU64::new(1).expect("one"), 0, body)
    }
    pub fn new(
        name: &'static str,
        phase: PhaseId,
        every_cycles: NonZeroU64,
        cycle_offset: u64,
        body: impl FnMut(ScheduleTick) + Send + 'static,
    ) -> Self {
        Self {
            name,
            phase,
            every_cycles,
            cycle_offset,
            last_fired_slot: None,
            body: Box::new(body),
        }
    }
    pub fn is_due(&self, tick: ScheduleTick) -> bool {
        if tick.phase != self.phase || self.last_fired_slot == Some(tick.slot_index) {
            return false;
        }
        let period = self.every_cycles.get() as u128;
        tick.cycle_index % period == (self.cycle_offset as u128) % period
    }
    fn fire(&mut self, tick: ScheduleTick) {
        self.last_fired_slot = Some(tick.slot_index);
        (self.body)(tick);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleStrategy {
    TightSpin,
    YieldNow,
    Sleep(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedWindowPolicy {
    /// Only evaluate the newest observed slot.
    Skip,
    /// Replay every missed slot, up to `max_slots`; larger gaps fail closed.
    Replay { max_slots: usize },
    /// Any skipped slot is an error.
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StepReport {
    pub evaluated_slots: usize,
    pub fired_tasks: usize,
    pub missed_slots: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventKind {
    PhaseRollover,
    TaskFire,
    MissedWindow,
}

pub struct CyclicExecutive {
    clock: PhaseClock,
    tasks: Vec<PhaseTask>,
    idle_strategy: IdleStrategy,
    missed_policy: MissedWindowPolicy,
    last_processed_slot: Option<u128>,
    last_phase: Option<PhaseId>,
    on_trace: Option<Box<dyn FnMut(TraceEventKind, ScheduleTick, &'static str) + Send + 'static>>,
}

impl CyclicExecutive {
    pub fn new(slot_duration: Duration, phase_count: NonZeroUsize) -> Result<Self, TemporalError> {
        Self::with_clock(PhaseClock::new(slot_duration, phase_count)?)
    }
    pub fn with_clock(clock: PhaseClock) -> Result<Self, TemporalError> {
        Ok(Self {
            clock,
            tasks: Vec::new(),
            idle_strategy: IdleStrategy::YieldNow,
            missed_policy: MissedWindowPolicy::Skip,
            last_processed_slot: None,
            last_phase: None,
            on_trace: None,
        })
    }
    pub fn clock(&self) -> &PhaseClock {
        &self.clock
    }
    pub fn set_idle_strategy(&mut self, strategy: IdleStrategy) {
        self.idle_strategy = strategy;
    }
    pub fn set_missed_window_policy(&mut self, policy: MissedWindowPolicy) {
        self.missed_policy = policy;
    }
    pub fn set_trace_hook(
        &mut self,
        hook: impl FnMut(TraceEventKind, ScheduleTick, &'static str) + Send + 'static,
    ) {
        self.on_trace = Some(Box::new(hook));
    }
    pub fn add_task(&mut self, task: PhaseTask) -> Result<(), TemporalError> {
        if task.phase.0 >= self.clock.phase_count() {
            return Err(TemporalError::PhaseCountTooLarge);
        }
        self.tasks.push(task);
        Ok(())
    }
    pub fn step(&mut self) -> Result<StepReport, TemporalError> {
        let tick = self.clock.tick();
        self.step_tick(tick)
    }
    pub fn step_at_elapsed(&mut self, elapsed: Duration) -> Result<StepReport, TemporalError> {
        let tick = self.clock.tick_at_elapsed(elapsed);
        self.step_tick(tick)
    }
    pub fn step_tick(&mut self, tick: ScheduleTick) -> Result<StepReport, TemporalError> {
        let missed = self
            .last_processed_slot
            .map(|last| tick.slot_index.saturating_sub(last.saturating_add(1)))
            .unwrap_or(0);
        if missed == 0 {
            return Ok(self.eval_one(tick, 0));
        }
        if let Some(hook) = &mut self.on_trace {
            hook(TraceEventKind::MissedWindow, tick, "missed_window");
        }
        match self.missed_policy {
            MissedWindowPolicy::Skip => Ok(self.eval_one(tick, missed as usize)),
            MissedWindowPolicy::Error => Err(TemporalError::MissedSlots {
                expected: self.last_processed_slot.unwrap_or(0).saturating_add(1),
                observed: tick.slot_index,
            }),
            MissedWindowPolicy::Replay { max_slots } => {
                if missed as usize > max_slots {
                    return Err(TemporalError::MissedSlots {
                        expected: self.last_processed_slot.unwrap_or(0).saturating_add(1),
                        observed: tick.slot_index,
                    });
                }
                let first = self
                    .last_processed_slot
                    .unwrap_or(tick.slot_index)
                    .saturating_add(1);
                let mut report = StepReport {
                    evaluated_slots: 0,
                    fired_tasks: 0,
                    missed_slots: missed as usize,
                };
                for slot in first..=tick.slot_index {
                    let elapsed = duration_mul(self.clock.slot_duration(), slot);
                    let r = self.eval_one(self.clock.tick_at_elapsed(elapsed), 0);
                    report.evaluated_slots += r.evaluated_slots;
                    report.fired_tasks += r.fired_tasks;
                }
                report.missed_slots = missed as usize;
                Ok(report)
            }
        }
    }
    fn eval_one(&mut self, tick: ScheduleTick, missed_slots: usize) -> StepReport {
        if self.last_phase != Some(tick.phase) {
            self.last_phase = Some(tick.phase);
            if let Some(hook) = &mut self.on_trace {
                hook(TraceEventKind::PhaseRollover, tick, "phase_rollover");
            }
        }
        let mut fired = 0;
        for task in &mut self.tasks {
            if task.is_due(tick) {
                if let Some(hook) = &mut self.on_trace {
                    hook(TraceEventKind::TaskFire, tick, task.name);
                }
                task.fire(tick);
                fired += 1;
            }
        }
        self.last_processed_slot = Some(tick.slot_index);
        StepReport {
            evaluated_slots: 1,
            fired_tasks: fired,
            missed_slots,
        }
    }
    pub fn run_for(&mut self, duration: Duration) -> Result<(), TemporalError> {
        let start = Instant::now();
        while start.elapsed() < duration {
            self.step()?;
            match self.idle_strategy {
                IdleStrategy::TightSpin => std::hint::spin_loop(),
                IdleStrategy::YieldNow => std::thread::yield_now(),
                IdleStrategy::Sleep(d) => std::thread::sleep(d),
            }
        }
        Ok(())
    }
}

fn duration_mul(slot: Duration, n: u128) -> Duration {
    let nanos = slot.as_nanos().saturating_mul(n).min(u64::MAX as u128);
    Duration::from_nanos(nanos as u64)
}

#[derive(Debug)]
pub struct TemporalPointerHandle<T, const LOW_BITS: usize> {
    tagged: SplitTaggedNonNull<T, LOW_BITS>,
    generation: u64,
}
impl<T, const LOW_BITS: usize> Copy for TemporalPointerHandle<T, LOW_BITS> {}
impl<T, const LOW_BITS: usize> Clone for TemporalPointerHandle<T, LOW_BITS> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, const LOW_BITS: usize> TemporalPointerHandle<T, LOW_BITS> {
    pub fn word(self) -> usize {
        self.tagged.word()
    }
    pub fn domain(self) -> usize {
        self.tagged.low()
    }
    pub fn phase(self) -> u8 {
        self.tagged.top()
    }
    pub fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Clone)]
pub struct TemporalPointerAuthority<const LOW_BITS: usize> {
    clock: PhaseClock,
    domain: usize,
    generation: u64,
}
impl<const LOW_BITS: usize> TemporalPointerAuthority<LOW_BITS> {
    pub fn new(clock: PhaseClock, domain: usize) -> Result<Self, TemporalError> {
        LowTagLayout::<LOW_BITS>::validate_tag(domain)?;
        if clock.phase_count() > 256 {
            return Err(TemporalError::PhaseCountTooLarge);
        }
        Ok(Self {
            clock,
            domain,
            generation: 1,
        })
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn transition(&mut self) -> Result<(), TemporalError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(TemporalError::ExpiredAuthority)?;
        Ok(())
    }
    pub fn issue<T>(
        &self,
        ptr: NonNull<T>,
    ) -> Result<TemporalPointerHandle<T, LOW_BITS>, TemporalError> {
        self.issue_at_elapsed(ptr, self.clock.elapsed())
    }
    pub fn issue_at_elapsed<T>(
        &self,
        ptr: NonNull<T>,
        elapsed: Duration,
    ) -> Result<TemporalPointerHandle<T, LOW_BITS>, TemporalError> {
        let phase = self.clock.phase_at_elapsed(elapsed).0 as u8;
        Ok(TemporalPointerHandle {
            tagged: SplitTaggedNonNull::new(ptr, phase, self.domain)?,
            generation: self.generation,
        })
    }
    pub fn validate<T>(
        &self,
        handle: TemporalPointerHandle<T, LOW_BITS>,
    ) -> Result<NonNull<T>, TemporalError> {
        self.validate_at_elapsed(handle, self.clock.elapsed())
    }
    pub fn validate_at_elapsed<T>(
        &self,
        handle: TemporalPointerHandle<T, LOW_BITS>,
        elapsed: Duration,
    ) -> Result<NonNull<T>, TemporalError> {
        if handle.domain() != self.domain {
            return Err(TemporalError::WrongDomain);
        }
        if handle.generation != self.generation {
            return Err(TemporalError::ExpiredAuthority);
        }
        let expected = self.clock.phase_at_elapsed(elapsed).0;
        let found = handle.phase() as usize;
        if expected != found {
            return Err(TemporalError::PhaseMismatch { expected, found });
        }
        Ok(handle.tagged.clean())
    }
}
