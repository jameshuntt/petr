use petr_collections::DispatchVec;
use petr_core::{PortHint, ProphecyBits, SelectorWord};
use petr_dispatch::{OpFrame, default_table};

fn main() {
    let prophecy = ProphecyBits {
        maybe_zero: false,
        maybe_empty: false,
        hot: true,
        skip_hint: false,
        port_hint: PortHint::Dispatch,
    };
    let skip = ProphecyBits {
        maybe_zero: true,
        maybe_empty: false,
        hot: false,
        skip_hint: true,
        port_hint: PortHint::Dispatch,
    };
    let mut vec = DispatchVec::<String, 3>::new();
    vec.push(
        "first".to_string(),
        0b001,
        SelectorWord::new(1, 1, 0x01),
        prophecy,
    )
    .unwrap();
    vec.push(
        "second".to_string(),
        0b010,
        SelectorWord::new(1, 1, 0x02),
        skip,
    )
    .unwrap();
    let table = default_table();
    let out = vec.dispatch_all(&table, OpFrame::text("Data")).unwrap();
    println!("dispatch vec outputs = {:?}", out);
}
