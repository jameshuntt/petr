use petr_dispatch::TypedDenseOperationTable;
use petr_core::SelectorWord;

#[derive(Debug, Default, PartialEq, Eq)]
struct State {
    value: i64,
}

#[derive(Debug, Clone, Copy)]
struct Context {
    delta: i64,
}

fn nop(_state: &mut State, _context: &Context) -> Result<(), &'static str> { Ok(()) }

fn add(state: &mut State, context: &Context) -> Result<(), &'static str> {
    state.value += context.delta;
    Ok(())
}

#[test]
fn typed_dense_table_routes_application_state_by_selector_word() {
    let selector = SelectorWord::new(1, 2, 0x44);
    let mut table = TypedDenseOperationTable::new(nop);
    table.register_word(selector, add);

    let mut state = State::default();
    table.execute_word(selector, &mut state, &Context { delta: 7 }).unwrap();
    assert_eq!(state, State { value: 7 });

    table.execute(0x99, &mut state, &Context { delta: 100 }).unwrap();
    assert_eq!(state, State { value: 7 });
}
