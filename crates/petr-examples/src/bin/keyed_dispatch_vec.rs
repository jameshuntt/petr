use petr_collections::KeyedDispatchVec;
use petr_dispatch::{default_table, OpFrame};
use petr_core::{Key, PortHint, ProphecyBits, SelectorWord};

#[repr(align(32))]
#[derive(Debug)]
struct Node(&'static str);

fn prophecy() -> ProphecyBits {
    ProphecyBits { maybe_zero: false, maybe_empty: false, hot: true, skip_hint: false, port_hint: PortHint::Dispatch }
}

fn main() {
    let table = default_table();
    let key = Key::<5>::from_seed(0xA11CE);
    let mut vec = KeyedDispatchVec::<Node, 5>::with_key(key);

    let handle = vec.push(Node("owned-provenance"), 0b10101, SelectorWord::new(1, 2, 0x20), prophecy()).unwrap();
    let value = vec.get(handle).unwrap();
    let output = vec.dispatch_handle(&table, handle, OpFrame::text("ff")).unwrap().unwrap();

    println!("value={:?} payload={}", value, value.0);
    println!("handle index={} generation={} tag={:#b} check={:#x}", handle.index(), handle.generation(), handle.tag(), handle.check_word());
    println!("dispatch output={:?}", output);
}
