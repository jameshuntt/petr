use petr_collections::ArenaEngine;
use petr_dispatch::{default_table, OpFrame};
use petr_core::{Key, PortHint, ProphecyBits, SelectorWord};

#[repr(align(32))]
#[derive(Debug)]
struct ArchiveObject {
    id: u64,
    label: &'static str,
    payload: &'static str,
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
    let mut engine = ArenaEngine::<ArchiveObject, 5>::with_key_and_table(Key::from_seed(0x00A2_C41E), table);

    let transcript = engine
        .insert(
            ArchiveObject { id: 1, label: "transcript-segment", payload: "ff" },
            0b00001,
            SelectorWord::new(1, 1, 0x20),
            prophecy(false),
        )
        .unwrap();

    let skipped_note = engine
        .insert(
            ArchiveObject { id: 2, label: "skip-note", payload: "do-not-prefix" },
            0b00010,
            SelectorWord::new(1, 1, 0x01),
            prophecy(true),
        )
        .unwrap();

    let encoded = engine.encode_handle(transcript).unwrap();
    let decoded = engine.decode_handle(encoded).unwrap();
    let object = engine.get(decoded).unwrap();
    let normalized = engine.dispatch(decoded, OpFrame::text(object.payload)).unwrap().unwrap();

    println!("event target={} label={} encoded_handle={:?}", object.id, object.label, encoded);
    println!("normalized transcript payload => {:?}", normalized);

    let skip_object = engine.get(skipped_note).unwrap();
    let skipped = engine.dispatch(skipped_note, OpFrame::text(skip_object.payload)).unwrap().unwrap();
    println!("skipped note {} stayed => {:?}", skip_object.label, skipped);
}
