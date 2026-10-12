#![no_main]
use libfuzzer_sys::fuzz_target;
use petr_core::{LowTagLayout, ResidualClass, ResidualRouter, SelectorWord, ValueWord};

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 { return; }
    let selector = SelectorWord::new(data[0], data[1], data[2]);
    let _ = selector.raw();
    let _ = selector.trait_id();
    let _ = selector.impl_id();
    let _ = selector.function_id();

    let mut word_bytes = [0u8; 8];
    word_bytes.copy_from_slice(&data[..8]);
    let raw = usize::from_ne_bytes(word_bytes);
    let tag = raw & LowTagLayout::<5>::LOW_MASK;
    let _ = LowTagLayout::<5>::validate_tag(tag);
    let _ = ValueWord::immediate_i31(raw as i32);
    let _ = ValueWord::handle(raw as u64);

    if data[3] != 0 {
        let modulus = data[3] as usize;
        let residue = (data[4] as usize) % modulus;
        let _ = ResidualClass::new(modulus, residue);
        if let Ok(router) = ResidualRouter::new(modulus) {
            if let Ok(router) = router.set(residue, data[2]) {
                let _ = router.selector_for(raw);
            }
        }
    }
});
