use petr_collections::KeyedArena;
use petr_dispatch::{default_table, OpFrame};
use petr_core::{Key, PortHint, ProphecyBits, SelectorWord};

#[repr(align(32))]
#[derive(Debug)]
struct Node {
    name: &'static str,
}

fn prophecy(skip_hint: bool) -> ProphecyBits {
    ProphecyBits {
        maybe_zero: false,
        maybe_empty: false,
        hot: true,
        skip_hint,
        port_hint: PortHint::Dispatch,
    }
}

fn main() {
    let table = default_table();
    let mut arena = KeyedArena::<Node, 5>::with_key(Key::from_seed(0xA11E_CA7E));

    let encode = arena
        .insert(Node { name: "hex-normalizer" }, 0b10101, SelectorWord::new(1, 2, 0x20), prophecy(false))
        .unwrap();
    let skip = arena
        .insert(Node { name: "skip-prefix" }, 0b00011, SelectorWord::new(1, 2, 0x01), prophecy(true))
        .unwrap();

    println!("arena len={} capacity={}", arena.len(), arena.slot_capacity());
    println!("encode handle: index={} generation={} tag={} selector=0x{:04x}", encode.index(), encode.generation(), encode.tag(), encode.selector().raw());
    println!("encode node: {:?} name={}", arena.get(encode).unwrap(), arena.get(encode).unwrap().name);
    println!("skip node: {:?} name={}", arena.get(skip).unwrap(), arena.get(skip).unwrap().name);

    let out = arena.dispatch_handle(&table, encode, OpFrame::text("ff")).unwrap().unwrap();
    println!("encode dispatch => {:?}", out);

    let skipped = arena.dispatch_handle(&table, skip, OpFrame::text("ff")).unwrap().unwrap();
    println!("skip dispatch => {:?}", skipped);

    let encode2 = arena.set_tag(encode, 0b00100).unwrap();
    println!("old encode handle valid? {}", arena.contains(encode));
    println!("new encode tag={}", encode2.tag());

    let removed = arena.remove(encode2).unwrap();
    println!("removed node: {:?}", removed);
    println!("old handle valid after remove? {}", arena.contains(encode2));
}
