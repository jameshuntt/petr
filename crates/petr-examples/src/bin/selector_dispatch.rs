use petr_core::SelectorWord;
use petr_dispatch::{OpFrame, default_table};

fn main() {
    let table = default_table();
    let selector = SelectorWord::new(0x1, 0x2, 0x20);
    let out = table.execute_word(selector, OpFrame::text("ff")).unwrap();
    println!(
        "selector raw={:#06x} trait={} impl={} fn={:#04x} => {:?}",
        selector.raw(),
        selector.trait_id(),
        selector.impl_id(),
        selector.function_id(),
        out
    );
}
