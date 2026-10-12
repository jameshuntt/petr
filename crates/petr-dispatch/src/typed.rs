use petr_core::SelectorWord;

/// Function pointer used by a typed dense operation table.
///
/// Unlike [`crate::OperationTable`], this form is not coupled to `OpFrame`.
/// It can mutate an application-owned state value while reading an immutable
/// execution context.
pub type TypedDenseOpFn<State, Context, Error> =
    fn(&mut State, &Context) -> Result<(), Error>;

/// Fixed 256-slot selector table for application-specific state machines.
///
/// Every slot is initialized with the caller-provided fallback operation. The
/// hot path is therefore a selector-indexed function-pointer call without an
/// `Option` lookup. This is still an indirect branch at the CPU level.
pub struct TypedDenseOperationTable<State, Context, Error> {
    entries: [TypedDenseOpFn<State, Context, Error>; 256],
}

impl<State, Context, Error> core::fmt::Debug
    for TypedDenseOperationTable<State, Context, Error>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TypedDenseOperationTable")
            .field("slots", &self.entries.len())
            .finish()
    }
}

impl<State, Context, Error> Clone
    for TypedDenseOperationTable<State, Context, Error>
{
    fn clone(&self) -> Self {
        Self { entries: self.entries }
    }
}

impl<State, Context, Error> TypedDenseOperationTable<State, Context, Error> {
    /// Create a table with all selectors bound to `fallback`.
    pub fn new(fallback: TypedDenseOpFn<State, Context, Error>) -> Self {
        Self { entries: [fallback; 256] }
    }

    /// Register an application operation at a raw byte selector.
    pub fn register(
        &mut self,
        selector: u8,
        operation: TypedDenseOpFn<State, Context, Error>,
    ) {
        self.entries[selector as usize] = operation;
    }

    /// Register using the function byte of a 4/4/8 selector word.
    pub fn register_word(
        &mut self,
        selector: SelectorWord,
        operation: TypedDenseOpFn<State, Context, Error>,
    ) {
        self.register(selector.function_id(), operation);
    }

    /// Execute by raw selector.
    pub fn execute(
        &self,
        selector: u8,
        state: &mut State,
        context: &Context,
    ) -> Result<(), Error> {
        (self.entries[selector as usize])(state, context)
    }

    /// Execute using the function byte of a selector word.
    pub fn execute_word(
        &self,
        selector: SelectorWord,
        state: &mut State,
        context: &Context,
    ) -> Result<(), Error> {
        self.execute(selector.function_id(), state, context)
    }
}
