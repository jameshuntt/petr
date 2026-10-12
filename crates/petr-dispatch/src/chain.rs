use petr_core::SelectorWord;

use crate::{DispatchError, DispatchResult, OpArg, OpFrame, OperationTable};

/// One continuation-program step.
#[derive(Debug, Clone)]
pub struct ChainStep {
    /// Selector describing trait/impl/function ids.
    pub selector: SelectorWord,
    /// Optional argument override for future engines. The current engine uses table-bound args.
    pub arg: OpArg,
    /// Next program-counter index. `None` means stop.
    pub next: Option<usize>,
}

/// A tail-call-ish chain that repeatedly dispatches by selector.
#[derive(Debug, Clone, Default)]
pub struct ChainProgram {
    steps: Vec<ChainStep>,
}

impl ChainProgram {
    /// Create an empty chain.
    pub fn new() -> Self { Self { steps: Vec::new() } }

    /// Push a step and return its index.
    pub fn push(&mut self, step: ChainStep) -> usize {
        let idx = self.steps.len();
        self.steps.push(step);
        idx
    }

    /// Run the chain from `start`.
    pub fn run(&self, table: &OperationTable, start: usize, mut frame: OpFrame) -> DispatchResult<OpFrame> {
        let mut pc = Some(start);
        let mut guard = 0usize;
        while let Some(i) = pc {
            let step = self.steps.get(i).ok_or(DispatchError::InvalidContinuation(i))?;
            frame = table.execute_word(step.selector, frame)?;
            pc = step.next;
            guard += 1;
            if guard > self.steps.len().saturating_mul(4).max(64) {
                return Err(DispatchError::InvalidInput("chain guard tripped; possible cycle".into()));
            }
        }
        Ok(frame)
    }
}
