#![no_main]
use libfuzzer_sys::fuzz_target;

use petr_collections::{EncodedArenaHandle, KeyedArena};
use petr_core::{Key, PortHint, ProphecyBits, SelectorWord};

#[repr(align(32))]
#[derive(Debug)]
struct FuzzItem(u64);

fn prophecy(byte: u8) -> ProphecyBits {
    ProphecyBits {
        maybe_zero: byte & 1 != 0,
        maybe_empty: byte & 2 != 0,
        hot: byte & 4 != 0,
        skip_hint: byte & 8 != 0,
        port_hint: PortHint::Dispatch,
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 24 { return; }
    let mut arena = KeyedArena::<FuzzItem, 5>::with_key(Key::from_seed(0xF00D));
    let issued = arena
        .insert(
            FuzzItem(u64::from(data[0])),
            (data[1] as usize) & 0x1F,
            SelectorWord::new(data[2], data[3], data[4]),
            prophecy(data[5]),
        )
        .unwrap();

    let mut b8 = [0u8; 8];
    b8.copy_from_slice(&data[8..16]);
    let check = usize::from_ne_bytes(b8);
    let mut b4 = [0u8; 4];
    b4.copy_from_slice(&data[16..20]);
    let index = u32::from_ne_bytes(b4);
    b4.copy_from_slice(&data[20..24]);
    let generation = u32::from_ne_bytes(b4);

    let encoded = EncodedArenaHandle {
        index,
        generation,
        check,
        selector: u16::from(data[6]) << 8 | u16::from(data[7]),
        prophecy: data[5],
        tag: u16::from(data[1]),
    };

    if let Some(handle) = encoded.to_handle::<FuzzItem, 5>() {
        let _ = arena.get(handle);
    }

    let known = EncodedArenaHandle::from(issued);
    let known = known.to_handle::<FuzzItem, 5>().unwrap();
    let _ = arena.get(known).unwrap().0;
});
