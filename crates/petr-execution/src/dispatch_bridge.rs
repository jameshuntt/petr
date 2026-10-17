//! Optional temporal gating for `petr-dispatch`.
//!
//! The dispatch crate remains completely usable on its own. This module merely
//! decides whether an existing dispatch operation is eligible at a logical tick.

use petr_dispatch::{DispatchResult, OpFrame, OperationTable, TypedDenseOperationTable};
use petr_temporal::{Tick, Window};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchStatus {
    Deferred,
    Executed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome<T> {
    Deferred(T),
    Executed(T),
}

pub fn execute_if_eligible(
    window: Window,
    tick: Tick,
    table: &OperationTable,
    selector: u8,
    frame: OpFrame,
) -> DispatchResult<DispatchOutcome<OpFrame>> {
    if !window.eligible(tick) {
        return Ok(DispatchOutcome::Deferred(frame));
    }
    table
        .execute(selector, frame)
        .map(DispatchOutcome::Executed)
}

pub fn execute_typed_if_eligible<State, Context, Error>(
    window: Window,
    tick: Tick,
    table: &TypedDenseOperationTable<State, Context, Error>,
    selector: u8,
    state: &mut State,
    context: &Context,
) -> Result<DispatchStatus, Error> {
    if !window.eligible(tick) {
        return Ok(DispatchStatus::Deferred);
    }
    table.execute(selector, state, context)?;
    Ok(DispatchStatus::Executed)
}
