//! Optional composition layer for PETR execution policy.
//!
//! `petr-execution` does not replace `scope`, selector dispatch, temporal
//! eligibility, resource management, affinity, wake protocols, or observables.
//! It combines those independent capabilities into lifetime-bound handler plans.

#![deny(unsafe_code)]

#[cfg(feature = "dispatch")]
pub mod dispatch_bridge;
#[cfg(feature = "observable")]
pub mod observable_bridge;
#[cfg(feature = "waker-protocol")]
pub mod protocol_bridge;

#[cfg(loom)]
use loom::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use petr_affinity::{AffinityError, CpuId, CpuSet, Placement, pin_current};
pub use petr_protection::{ProtectionCapabilities, ProtectionRequirement};
use petr_resource::{ResourceError, ResourceHandle, ResourceManager, ResourceSelector};
use petr_temporal::{Tick, Window};
use scope::{QoSClass, SchedulerConfig};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(loom))]
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Obligation {
    Required,
    Opportunistic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceNeed {
    pub selector: ResourceSelector,
    pub chunks: u32,
}

/// Stable, opaque identity for the external effect a logical handler represents.
///
/// PETR does not interpret this value and does not own receipts, retries, or
/// idempotency policy. Higher layers may bind their own stable digest/identifier
/// here so repeated admissions of the same logical effect remain recognizable
/// even though every concrete admission uses a distinct [`AdmissionEpoch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectIdentity([u8; 32]);

impl EffectIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Backward-compatible simple handler declaration.
///
/// More advanced policy is layered on with [`ExecutionPlan::set_profile`],
/// [`ExecutionPlan::transition_profile`] and dependencies. A plain `HandlerSpec`
/// behaves exactly as before: standard QoS, no dependencies, no transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandlerSpec {
    pub id: u32,
    pub name: &'static str,
    pub window: Window,
    pub placement: Placement,
    pub obligation: Obligation,
    pub resource: Option<ResourceNeed>,
}

impl HandlerSpec {
    pub const fn profile(self) -> ExecutionProfile {
        ExecutionProfile {
            placement: self.placement,
            obligation: self.obligation,
            resource: self.resource,
            qos: QoSClass::Standard,
            protection: ProtectionRequirement::None,
        }
    }
}

/// Policy that may change while the logical handler identity remains stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionProfile {
    pub placement: Placement,
    pub obligation: Obligation,
    pub resource: Option<ResourceNeed>,
    pub qos: QoSClass,
    /// Protection semantics required by this profile. Logical admission APIs
    /// remain capability-agnostic; use the capability-aware admission methods
    /// when this field is not `None`.
    pub protection: ProtectionRequirement,
}

impl ExecutionProfile {
    pub const fn new(
        placement: Placement,
        obligation: Obligation,
        resource: Option<ResourceNeed>,
        qos: QoSClass,
    ) -> Self {
        Self {
            placement,
            obligation,
            resource,
            qos,
            protection: ProtectionRequirement::None,
        }
    }

    pub const fn with_protection(mut self, protection: ProtectionRequirement) -> Self {
        self.protection = protection;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileTransition {
    pub at: Tick,
    pub profile: ExecutionProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assignment {
    pub handler_id: u32,
    pub cpu: CpuId,
    pub obligation: Obligation,
}

/// Caller-supplied identity for one concrete admission attempt.
///
/// Logical ticks and handler ids are intentionally replayable, so they are not
/// sufficient to distinguish a retry, resume, or later run of the same logical
/// handler. Callers that need replay isolation must supply a fresh epoch for
/// every concrete re-admission. The value is opaque to PETR and never changes
/// scheduling decisions.
///
/// [`AdmissionEpoch::DEFAULT`] exists only for backward-compatible callers that
/// do not require replay isolation; it must not be treated as a unique attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AdmissionEpoch(pub u64);

impl AdmissionEpoch {
    pub const DEFAULT: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn value(self) -> u64 {
        self.0
    }
    pub const fn is_default(self) -> bool {
        self.0 == 0
    }
}

/// Stable authority identity for one admitted handler invocation.
///
/// `handler_id + tick` identifies where the handler sits in the logical plan;
/// `epoch` distinguishes separate runs/replays of that same logical point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionContext {
    pub handler_id: u32,
    pub tick: Tick,
    pub epoch: AdmissionEpoch,
}

impl ExecutionContext {
    /// Backward-compatible context for callers that do not distinguish runs.
    pub const fn new(handler_id: u32, tick: Tick) -> Self {
        Self {
            handler_id,
            tick,
            epoch: AdmissionEpoch::DEFAULT,
        }
    }

    pub const fn in_epoch(handler_id: u32, tick: Tick, epoch: AdmissionEpoch) -> Self {
        Self {
            handler_id,
            tick,
            epoch,
        }
    }
}

/// Terminal/runtime state of one admitted handler authority.
///
/// The state is deliberately separate from [`ExecutionContext`]. Context is a
/// deterministic replay identity; authority state is runtime liveness and may
/// transition exactly once away from `Active`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionAuthorityStatus {
    Active = 0,
    Completed = 1,
    Cancelled = 2,
    Revoked = 3,
}

impl ExecutionAuthorityStatus {
    const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Active,
            1 => Self::Completed,
            2 => Self::Cancelled,
            _ => Self::Revoked,
        }
    }

    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Active)
    }
}

/// Runtime liveness token for one admitted handler invocation.
///
/// `ExecutionContext` is deterministic and may intentionally be reproduced
/// during replay. `ExecutionAuthority` instead identifies the live authority of
/// one concrete admitted handler. Each handler receives an independent token.
/// Completion or cancellation revokes that handler immediately; any authority
/// still active is revoked when the owning [`AdmittedPlan`] drops.
#[derive(Clone)]
pub struct ExecutionAuthority {
    context: ExecutionContext,
    state: Arc<AtomicU8>,
}

impl core::fmt::Debug for ExecutionAuthority {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExecutionAuthority")
            .field("context", &self.context)
            .field("status", &self.status())
            .finish()
    }
}

impl ExecutionAuthority {
    fn new(context: ExecutionContext, state: Arc<AtomicU8>) -> Self {
        Self { context, state }
    }

    pub const fn context(&self) -> ExecutionContext {
        self.context
    }

    pub fn status(&self) -> ExecutionAuthorityStatus {
        ExecutionAuthorityStatus::from_u8(self.state.load(Ordering::Acquire))
    }

    pub fn is_active(&self) -> bool {
        self.status() == ExecutionAuthorityStatus::Active
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerReadiness {
    Completed,
    TemporalClosed,
    WaitingForDependency,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandlerInspection {
    pub id: u32,
    pub tick: Tick,
    pub readiness: HandlerReadiness,
    pub profile: ExecutionProfile,
}

/// Mutable progress state is deliberately separate from the immutable plan.
/// This keeps the same plan reusable for simulation, replay and multiple runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanState {
    completed: BTreeSet<u32>,
}

impl PlanState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn mark_completed(&mut self, handler_id: u32) {
        self.completed.insert(handler_id);
    }
    pub fn clear_completed(&mut self, handler_id: u32) {
        self.completed.remove(&handler_id);
    }
    pub fn is_completed(&self, handler_id: u32) -> bool {
        self.completed.contains(&handler_id)
    }
    pub fn completed(&self) -> impl Iterator<Item = u32> + '_ {
        self.completed.iter().copied()
    }
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("handler id {0} is duplicated")]
    DuplicateHandler(u32),
    #[error("handler {0} is not part of the execution plan")]
    UnknownHandler(u32),
    #[error("handler {handler_id} depends on unknown handler {prerequisite}")]
    UnknownDependency { handler_id: u32, prerequisite: u32 },
    #[error("handler {0} cannot depend on itself")]
    SelfDependency(u32),
    #[error("dependency would create a cycle: handler {handler_id} -> {prerequisite}")]
    DependencyCycle { handler_id: u32, prerequisite: u32 },
    #[error("required handler {handler_id} could not obtain a CPU")]
    NoCpu { handler_id: u32 },
    #[error("affinity error: {0}")]
    Affinity(#[from] AffinityError),
    #[error("resource error: {0}")]
    Resource(#[from] ResourceError),
    #[error("required handler {handler_id} needs unavailable protection {requirement:?}")]
    ProtectionUnavailable {
        handler_id: u32,
        requirement: ProtectionRequirement,
    },
    #[error("handler {handler_id} authority is already terminal: {status:?}")]
    AuthorityTerminal {
        handler_id: u32,
        status: ExecutionAuthorityStatus,
    },
}

#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    allowed: CpuSet,
    handlers: BTreeMap<u32, HandlerSpec>,
    effects: BTreeMap<u32, EffectIdentity>,
    dependencies: BTreeMap<u32, BTreeSet<u32>>,
    transitions: BTreeMap<u32, Vec<ProfileTransition>>,
    once: BTreeSet<u32>,
}

impl ExecutionPlan {
    pub fn new(allowed: CpuSet) -> Self {
        Self {
            allowed,
            handlers: BTreeMap::new(),
            effects: BTreeMap::new(),
            dependencies: BTreeMap::new(),
            transitions: BTreeMap::new(),
            once: BTreeSet::new(),
        }
    }

    pub fn insert(&mut self, spec: HandlerSpec) -> Result<(), ExecutionError> {
        if self.handlers.contains_key(&spec.id) {
            return Err(ExecutionError::DuplicateHandler(spec.id));
        }
        self.handlers.insert(spec.id, spec);
        Ok(())
    }

    pub fn handlers(&self) -> impl Iterator<Item = &HandlerSpec> {
        self.handlers.values()
    }
    pub fn allowed_cpus(&self) -> &CpuSet {
        &self.allowed
    }
    pub fn state(&self) -> PlanState {
        PlanState::new()
    }

    /// Bind a stable external-effect identity to a logical handler.
    ///
    /// The identity is immutable across admissions unless the caller explicitly
    /// replaces it. PETR stores and carries the value but never interprets it.
    pub fn set_effect_identity(
        &mut self,
        handler_id: u32,
        effect: EffectIdentity,
    ) -> Result<(), ExecutionError> {
        if !self.handlers.contains_key(&handler_id) {
            return Err(ExecutionError::UnknownHandler(handler_id));
        }
        self.effects.insert(handler_id, effect);
        Ok(())
    }

    /// Remove any stable external-effect identity from a handler.
    pub fn clear_effect_identity(&mut self, handler_id: u32) -> Result<(), ExecutionError> {
        if !self.handlers.contains_key(&handler_id) {
            return Err(ExecutionError::UnknownHandler(handler_id));
        }
        self.effects.remove(&handler_id);
        Ok(())
    }

    /// Return the stable external-effect identity for a handler, if one is bound.
    pub fn effect_identity(
        &self,
        handler_id: u32,
    ) -> Result<Option<EffectIdentity>, ExecutionError> {
        if !self.handlers.contains_key(&handler_id) {
            return Err(ExecutionError::UnknownHandler(handler_id));
        }
        Ok(self.effects.get(&handler_id).copied())
    }

    /// Mark a handler as one-shot for this [`PlanState`]. Once completed it is
    /// no longer admitted until that state is reset or its completion is cleared.
    pub fn set_once(&mut self, handler_id: u32) -> Result<(), ExecutionError> {
        if !self.handlers.contains_key(&handler_id) {
            return Err(ExecutionError::UnknownHandler(handler_id));
        }
        self.once.insert(handler_id);
        Ok(())
    }

    /// Restore the default recurring behavior for a handler.
    pub fn set_repeating(&mut self, handler_id: u32) -> Result<(), ExecutionError> {
        if !self.handlers.contains_key(&handler_id) {
            return Err(ExecutionError::UnknownHandler(handler_id));
        }
        self.once.remove(&handler_id);
        Ok(())
    }

    pub fn is_once(&self, handler_id: u32) -> bool {
        self.once.contains(&handler_id)
    }

    /// Make `handler_id` wait until `prerequisite` has completed.
    pub fn add_dependency(
        &mut self,
        handler_id: u32,
        prerequisite: u32,
    ) -> Result<(), ExecutionError> {
        if !self.handlers.contains_key(&handler_id) {
            return Err(ExecutionError::UnknownHandler(handler_id));
        }
        if !self.handlers.contains_key(&prerequisite) {
            return Err(ExecutionError::UnknownDependency {
                handler_id,
                prerequisite,
            });
        }
        if handler_id == prerequisite {
            return Err(ExecutionError::SelfDependency(handler_id));
        }
        if self.reaches(prerequisite, handler_id, &mut BTreeSet::new()) {
            return Err(ExecutionError::DependencyCycle {
                handler_id,
                prerequisite,
            });
        }
        self.dependencies
            .entry(handler_id)
            .or_default()
            .insert(prerequisite);
        Ok(())
    }

    pub fn dependencies(&self, handler_id: u32) -> Result<Vec<u32>, ExecutionError> {
        if !self.handlers.contains_key(&handler_id) {
            return Err(ExecutionError::UnknownHandler(handler_id));
        }
        Ok(self
            .dependencies
            .get(&handler_id)
            .map(|v| v.iter().copied().collect())
            .unwrap_or_default())
    }

    /// Set the profile in effect from logical tick 0 onward.
    pub fn set_profile(
        &mut self,
        handler_id: u32,
        profile: ExecutionProfile,
    ) -> Result<(), ExecutionError> {
        self.transition_profile(handler_id, Tick(0), profile)
    }

    /// Change policy at an explicit logical tick. Same-tick transitions replace.
    pub fn transition_profile(
        &mut self,
        handler_id: u32,
        at: Tick,
        profile: ExecutionProfile,
    ) -> Result<(), ExecutionError> {
        if !self.handlers.contains_key(&handler_id) {
            return Err(ExecutionError::UnknownHandler(handler_id));
        }
        let list = self.transitions.entry(handler_id).or_default();
        if let Some(existing) = list.iter_mut().find(|transition| transition.at == at) {
            existing.profile = profile;
        } else {
            list.push(ProfileTransition { at, profile });
            list.sort_by_key(|transition| transition.at);
        }
        Ok(())
    }

    pub fn transitions(&self, handler_id: u32) -> Result<&[ProfileTransition], ExecutionError> {
        if !self.handlers.contains_key(&handler_id) {
            return Err(ExecutionError::UnknownHandler(handler_id));
        }
        Ok(self
            .transitions
            .get(&handler_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]))
    }

    pub fn profile_at(
        &self,
        handler_id: u32,
        tick: Tick,
    ) -> Result<ExecutionProfile, ExecutionError> {
        let spec = *self
            .handlers
            .get(&handler_id)
            .ok_or(ExecutionError::UnknownHandler(handler_id))?;
        let mut profile = spec.profile();
        if let Some(transitions) = self.transitions.get(&handler_id) {
            for transition in transitions {
                if transition.at <= tick {
                    profile = transition.profile;
                } else {
                    break;
                }
            }
        }
        Ok(profile)
    }

    pub fn inspect(&self, tick: Tick, state: &PlanState) -> Vec<HandlerInspection> {
        self.handlers
            .values()
            .map(|spec| {
                let profile = self.profile_at(spec.id, tick).expect("known handler");
                let readiness = if self.once.contains(&spec.id) && state.is_completed(spec.id) {
                    HandlerReadiness::Completed
                } else if !spec.window.eligible(tick) {
                    HandlerReadiness::TemporalClosed
                } else if !self.dependencies_satisfied(spec.id, state) {
                    HandlerReadiness::WaitingForDependency
                } else {
                    HandlerReadiness::Ready
                };
                HandlerInspection {
                    id: spec.id,
                    tick,
                    readiness,
                    profile,
                }
            })
            .collect()
    }

    /// Backward-compatible admission view. Plans without dependencies behave as V1.
    pub fn assignments(&self, tick: Tick) -> Result<Vec<Assignment>, ExecutionError> {
        self.assignments_with_state(tick, &PlanState::default())
    }

    pub fn assignments_with_state(
        &self,
        tick: Tick,
        state: &PlanState,
    ) -> Result<Vec<Assignment>, ExecutionError> {
        self.assignments_internal(tick, state, None)
    }

    /// Capability-aware logical admission. Required handlers fail explicitly
    /// when their protection contract cannot be satisfied; opportunistic
    /// handlers with unavailable protection are omitted.
    pub fn assignments_with_capabilities(
        &self,
        tick: Tick,
        state: &PlanState,
        capabilities: ProtectionCapabilities,
    ) -> Result<Vec<Assignment>, ExecutionError> {
        self.assignments_internal(tick, state, Some(capabilities))
    }

    fn assignments_internal(
        &self,
        tick: Tick,
        state: &PlanState,
        capabilities: Option<ProtectionCapabilities>,
    ) -> Result<Vec<Assignment>, ExecutionError> {
        let mut occupied = Vec::<CpuId>::new();
        let mut out = Vec::new();
        let mut ready = Vec::<(u32, ExecutionProfile)>::new();

        for spec in self.handlers.values() {
            if !(self.once.contains(&spec.id) && state.is_completed(spec.id))
                && spec.window.eligible(tick)
                && self.dependencies_satisfied(spec.id, state)
            {
                let profile = self.profile_at(spec.id, tick)?;
                if let Some(capabilities) = capabilities {
                    if !capabilities.supports(profile.protection) {
                        if profile.obligation == Obligation::Required {
                            return Err(ExecutionError::ProtectionUnavailable {
                                handler_id: spec.id,
                                requirement: profile.protection,
                            });
                        }
                        continue;
                    }
                }
                ready.push((spec.id, profile));
            }
        }

        // Required fixed placements first: collisions are hard admission errors.
        for (handler_id, profile) in ready.iter().copied().filter(|(_, p)| {
            p.obligation == Obligation::Required && matches!(p.placement, Placement::Require(_))
        }) {
            let cpu = profile
                .placement
                .resolve(&self.allowed, &occupied)?
                .ok_or(ExecutionError::NoCpu { handler_id })?;
            occupied.push(cpu);
            out.push(Assignment {
                handler_id,
                cpu,
                obligation: profile.obligation,
            });
        }

        // Other required work receives its preference or deterministic fallback.
        for (handler_id, profile) in ready.iter().copied().filter(|(_, p)| {
            p.obligation == Obligation::Required && !matches!(p.placement, Placement::Require(_))
        }) {
            let cpu = profile
                .placement
                .resolve(&self.allowed, &occupied)?
                .ok_or(ExecutionError::NoCpu { handler_id })?;
            occupied.push(cpu);
            out.push(Assignment {
                handler_id,
                cpu,
                obligation: profile.obligation,
            });
        }

        // Opportunistic work never displaces required work and may be omitted.
        for (handler_id, profile) in ready
            .iter()
            .copied()
            .filter(|(_, p)| p.obligation == Obligation::Opportunistic)
        {
            if let Ok(Some(cpu)) = profile.placement.resolve(&self.allowed, &occupied) {
                occupied.push(cpu);
                out.push(Assignment {
                    handler_id,
                    cpu,
                    obligation: profile.obligation,
                });
            }
        }

        out.sort_by_key(|a| a.handler_id);
        Ok(out)
    }

    pub fn admit<'a, const LOW_BITS: usize>(
        &self,
        resources: &'a mut ResourceManager<LOW_BITS>,
        tick: Tick,
    ) -> Result<AdmittedPlan<'a, LOW_BITS>, ExecutionError> {
        self.admit_in_epoch(resources, tick, AdmissionEpoch::DEFAULT)
    }

    /// Admit a plan in an explicit caller-supplied execution epoch.
    ///
    /// The epoch does not affect deterministic placement or resource ordering.
    /// It exists so downstream capabilities can distinguish separate runs of
    /// the same logical handler/tick pair.
    pub fn admit_in_epoch<'a, const LOW_BITS: usize>(
        &self,
        resources: &'a mut ResourceManager<LOW_BITS>,
        tick: Tick,
        epoch: AdmissionEpoch,
    ) -> Result<AdmittedPlan<'a, LOW_BITS>, ExecutionError> {
        self.admit_internal(resources, tick, &PlanState::default(), None, epoch)
    }

    pub fn admit_with_state<'a, const LOW_BITS: usize>(
        &self,
        resources: &'a mut ResourceManager<LOW_BITS>,
        tick: Tick,
        state: &PlanState,
    ) -> Result<AdmittedPlan<'a, LOW_BITS>, ExecutionError> {
        self.admit_with_state_in_epoch(resources, tick, state, AdmissionEpoch::DEFAULT)
    }

    pub fn admit_with_state_in_epoch<'a, const LOW_BITS: usize>(
        &self,
        resources: &'a mut ResourceManager<LOW_BITS>,
        tick: Tick,
        state: &PlanState,
        epoch: AdmissionEpoch,
    ) -> Result<AdmittedPlan<'a, LOW_BITS>, ExecutionError> {
        self.admit_internal(resources, tick, state, None, epoch)
    }

    pub fn admit_with_capabilities<'a, const LOW_BITS: usize>(
        &self,
        resources: &'a mut ResourceManager<LOW_BITS>,
        tick: Tick,
        capabilities: ProtectionCapabilities,
    ) -> Result<AdmittedPlan<'a, LOW_BITS>, ExecutionError> {
        self.admit_with_capabilities_in_epoch(
            resources,
            tick,
            capabilities,
            AdmissionEpoch::DEFAULT,
        )
    }

    pub fn admit_with_capabilities_in_epoch<'a, const LOW_BITS: usize>(
        &self,
        resources: &'a mut ResourceManager<LOW_BITS>,
        tick: Tick,
        capabilities: ProtectionCapabilities,
        epoch: AdmissionEpoch,
    ) -> Result<AdmittedPlan<'a, LOW_BITS>, ExecutionError> {
        self.admit_internal(
            resources,
            tick,
            &PlanState::default(),
            Some(capabilities),
            epoch,
        )
    }

    pub fn admit_with_state_and_capabilities<'a, const LOW_BITS: usize>(
        &self,
        resources: &'a mut ResourceManager<LOW_BITS>,
        tick: Tick,
        state: &PlanState,
        capabilities: ProtectionCapabilities,
    ) -> Result<AdmittedPlan<'a, LOW_BITS>, ExecutionError> {
        self.admit_with_state_and_capabilities_in_epoch(
            resources,
            tick,
            state,
            capabilities,
            AdmissionEpoch::DEFAULT,
        )
    }

    pub fn admit_with_state_and_capabilities_in_epoch<'a, const LOW_BITS: usize>(
        &self,
        resources: &'a mut ResourceManager<LOW_BITS>,
        tick: Tick,
        state: &PlanState,
        capabilities: ProtectionCapabilities,
        epoch: AdmissionEpoch,
    ) -> Result<AdmittedPlan<'a, LOW_BITS>, ExecutionError> {
        self.admit_internal(resources, tick, state, Some(capabilities), epoch)
    }

    fn admit_internal<'a, const LOW_BITS: usize>(
        &self,
        resources: &'a mut ResourceManager<LOW_BITS>,
        tick: Tick,
        state: &PlanState,
        capabilities: Option<ProtectionCapabilities>,
        epoch: AdmissionEpoch,
    ) -> Result<AdmittedPlan<'a, LOW_BITS>, ExecutionError> {
        let mut assignments = self.assignments_internal(tick, state, capabilities)?;
        // Resource admission follows the same priority rule as CPU admission:
        // opportunistic work must never consume capacity required work needs.
        assignments.sort_by_key(|assignment| {
            let priority = match assignment.obligation {
                Obligation::Required => 0u8,
                Obligation::Opportunistic => 1u8,
            };
            (priority, assignment.handler_id)
        });
        let mut admitted: Vec<AdmittedHandler<LOW_BITS>> = Vec::new();

        for assignment in assignments {
            let spec = self
                .handlers
                .get(&assignment.handler_id)
                .expect("assignment came from plan");
            let profile = self.profile_at(spec.id, tick)?;
            let resource = match profile.resource {
                Some(need) => match resources.allocate(need.selector, need.chunks) {
                    Ok(handle) => Some(handle),
                    Err(err) if profile.obligation == Obligation::Opportunistic => {
                        let _ = err;
                        continue;
                    }
                    Err(err) => {
                        for handle in admitted.iter().filter_map(|handler| handler.resource) {
                            let _ = resources.release(handle);
                        }
                        return Err(ExecutionError::Resource(err));
                    }
                },
                None => None,
            };

            admitted.push(AdmittedHandler {
                id: spec.id,
                name: spec.name,
                cpu: assignment.cpu,
                obligation: profile.obligation,
                qos: profile.qos,
                protection: profile.protection,
                effect: self.effects.get(&spec.id).copied(),
                resource,
            });
        }

        admitted.sort_by_key(|handler| handler.id);
        let authorities = admitted
            .iter()
            .map(|handler| {
                (
                    handler.id,
                    Arc::new(AtomicU8::new(ExecutionAuthorityStatus::Active as u8)),
                )
            })
            .collect();
        Ok(AdmittedPlan {
            tick,
            epoch,
            resources,
            handlers: admitted,
            authorities,
        })
    }

    fn dependencies_satisfied(&self, handler_id: u32, state: &PlanState) -> bool {
        self.dependencies
            .get(&handler_id)
            .map(|deps| deps.iter().all(|id| state.is_completed(*id)))
            .unwrap_or(true)
    }

    fn reaches(&self, start: u32, target: u32, visited: &mut BTreeSet<u32>) -> bool {
        if start == target {
            return true;
        }
        if !visited.insert(start) {
            return false;
        }
        self.dependencies
            .get(&start)
            .map(|deps| deps.iter().any(|dep| self.reaches(*dep, target, visited)))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AdmittedHandler<const LOW_BITS: usize> {
    pub id: u32,
    pub name: &'static str,
    pub cpu: CpuId,
    pub obligation: Obligation,
    pub qos: QoSClass,
    pub protection: ProtectionRequirement,
    /// Stable opaque identity of the logical external effect, if the plan bound one.
    pub effect: Option<EffectIdentity>,
    pub resource: Option<ResourceHandle<LOW_BITS>>,
}

pub struct AdmittedPlan<'a, const LOW_BITS: usize> {
    tick: Tick,
    epoch: AdmissionEpoch,
    resources: &'a mut ResourceManager<LOW_BITS>,
    handlers: Vec<AdmittedHandler<LOW_BITS>>,
    authorities: BTreeMap<u32, Arc<AtomicU8>>,
}

impl<'a, const LOW_BITS: usize> AdmittedPlan<'a, LOW_BITS> {
    pub fn tick(&self) -> Tick {
        self.tick
    }
    pub fn epoch(&self) -> AdmissionEpoch {
        self.epoch
    }
    pub fn handlers(&self) -> &[AdmittedHandler<LOW_BITS>] {
        &self.handlers
    }
    pub fn handler(&self, id: u32) -> Option<&AdmittedHandler<LOW_BITS>> {
        self.handlers.iter().find(|handler| handler.id == id)
    }

    /// Borrow a lifetime-bound controller for one admitted handler.
    pub fn lease(&self, id: u32) -> Result<HandlerLease<'_, LOW_BITS>, ExecutionError> {
        let handler = self.handler(id).ok_or(ExecutionError::UnknownHandler(id))?;
        let state = self
            .authorities
            .get(&id)
            .ok_or(ExecutionError::UnknownHandler(id))?;
        let context = ExecutionContext::in_epoch(id, self.tick, self.epoch);
        let authority = ExecutionAuthority::new(context, Arc::clone(state));
        Ok(HandlerLease {
            tick: self.tick,
            epoch: self.epoch,
            handler,
            authority,
        })
    }

    /// Mark one admitted handler complete, release its admitted resource, and
    /// revoke only that handler's live authority. Other handlers remain live.
    pub fn complete_handler(&mut self, id: u32) -> Result<(), ExecutionError> {
        self.finish_handler(id, ExecutionAuthorityStatus::Completed)
    }

    /// Complete a handler and record plan progress in the caller-owned state.
    /// This keeps runtime liveness separate from replayable plan state while
    /// providing one operation when both transitions should happen together.
    pub fn complete_handler_and_mark(
        &mut self,
        id: u32,
        state: &mut PlanState,
    ) -> Result<(), ExecutionError> {
        self.complete_handler(id)?;
        state.mark_completed(id);
        Ok(())
    }

    /// Cancel one admitted handler and release its admitted resource.
    ///
    /// `Cancelled` is reserved for an explicit cancellation decision. It does
    /// not mean "deferred" or "retry later". Cancellation does not mark plan
    /// progress as completed. PETR does not own the higher-level retry policy.
    pub fn cancel_handler(&mut self, id: u32) -> Result<(), ExecutionError> {
        self.finish_handler(id, ExecutionAuthorityStatus::Cancelled)
    }

    /// End the current admission without claiming completion or cancellation.
    ///
    /// Higher layers use revocation when the current admission must die but the
    /// logical work may later be admitted again (for example after deferral,
    /// migration, or policy withdrawal). A later admission must use a fresh
    /// [`AdmissionEpoch`] when replay isolation matters.
    pub fn revoke_handler(&mut self, id: u32) -> Result<(), ExecutionError> {
        self.finish_handler(id, ExecutionAuthorityStatus::Revoked)
    }

    pub fn authority_status(&self, id: u32) -> Result<ExecutionAuthorityStatus, ExecutionError> {
        let state = self
            .authorities
            .get(&id)
            .ok_or(ExecutionError::UnknownHandler(id))?;
        Ok(ExecutionAuthorityStatus::from_u8(
            state.load(Ordering::Acquire),
        ))
    }

    fn finish_handler(
        &mut self,
        id: u32,
        terminal: ExecutionAuthorityStatus,
    ) -> Result<(), ExecutionError> {
        let state = Arc::clone(
            self.authorities
                .get(&id)
                .ok_or(ExecutionError::UnknownHandler(id))?,
        );
        let current = ExecutionAuthorityStatus::from_u8(state.load(Ordering::Acquire));
        if current != ExecutionAuthorityStatus::Active {
            return Err(ExecutionError::AuthorityTerminal {
                handler_id: id,
                status: current,
            });
        }

        let handler = self
            .handlers
            .iter_mut()
            .find(|handler| handler.id == id)
            .ok_or(ExecutionError::UnknownHandler(id))?;
        if let Some(handle) = handler.resource.take() {
            self.resources.release(handle)?;
        }

        state.store(terminal as u8, Ordering::Release);
        Ok(())
    }

    pub fn pin_current(&self, id: u32) -> Result<(), ExecutionError> {
        self.lease(id)?.pin_current()
    }

    pub fn run_scoped<'env, R>(
        &self,
        id: u32,
        env: &'env mut R,
        build: impl FnOnce(&mut scope::Scope<'env, R>),
    ) -> Result<scope::RunReport, ExecutionError> {
        self.lease(id).map(|lease| lease.run_scoped(env, build))
    }
}

impl<const LOW_BITS: usize> Drop for AdmittedPlan<'_, LOW_BITS> {
    fn drop(&mut self) {
        for state in self.authorities.values() {
            let _ = state.compare_exchange(
                ExecutionAuthorityStatus::Active as u8,
                ExecutionAuthorityStatus::Revoked as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
        for handle in self.handlers.iter().filter_map(|handler| handler.resource) {
            let _ = self.resources.release(handle);
        }
    }
}

/// A borrowed handler controller. Its lifetime cannot outlive the admitted plan
/// that owns the resource allocation and CPU assignment.
pub struct HandlerLease<'plan, const LOW_BITS: usize> {
    tick: Tick,
    epoch: AdmissionEpoch,
    handler: &'plan AdmittedHandler<LOW_BITS>,
    authority: ExecutionAuthority,
}

impl<const LOW_BITS: usize> HandlerLease<'_, LOW_BITS> {
    pub fn tick(&self) -> Tick {
        self.tick
    }
    pub fn epoch(&self) -> AdmissionEpoch {
        self.epoch
    }
    pub fn context(&self) -> ExecutionContext {
        self.authority.context()
    }
    /// Cloneable runtime token for this handler. It becomes terminal when the
    /// handler completes/cancels, or is revoked if the admitted plan drops first.
    pub fn authority(&self) -> ExecutionAuthority {
        self.authority.clone()
    }
    pub fn handler(&self) -> &AdmittedHandler<LOW_BITS> {
        self.handler
    }
    pub fn cpu(&self) -> CpuId {
        self.handler.cpu
    }
    pub fn effect_identity(&self) -> Option<EffectIdentity> {
        self.handler.effect
    }

    pub fn scheduler_config(&self) -> SchedulerConfig {
        SchedulerConfig {
            default_qos: self.handler.qos,
            ..SchedulerConfig::default()
        }
    }

    pub fn pin_current(&self) -> Result<(), ExecutionError> {
        pin_current(self.handler.cpu)?;
        Ok(())
    }

    pub fn run_scoped<'env, R>(
        &self,
        env: &'env mut R,
        build: impl FnOnce(&mut scope::Scope<'env, R>),
    ) -> scope::RunReport {
        self.run_scoped_with_config(env, self.scheduler_config(), build)
    }

    pub fn run_scoped_with_config<'env, R>(
        &self,
        env: &'env mut R,
        mut config: SchedulerConfig,
        build: impl FnOnce(&mut scope::Scope<'env, R>),
    ) -> scope::RunReport {
        config.default_qos = self.handler.qos;
        let mut scoped = scope::Scope::with_config(env, config);
        build(&mut scoped);
        scoped.run()
    }

    /// Pin the calling thread, then execute a structured scope on that thread.
    /// Pinning remains optional: without the affinity feature this returns an
    /// explicit `PinningUnsupported` error rather than changing normal behavior.
    pub fn run_bound_scoped<'env, R>(
        &self,
        env: &'env mut R,
        build: impl FnOnce(&mut scope::Scope<'env, R>),
    ) -> Result<scope::RunReport, ExecutionError> {
        self.pin_current()?;
        Ok(self.run_scoped(env, build))
    }
}
