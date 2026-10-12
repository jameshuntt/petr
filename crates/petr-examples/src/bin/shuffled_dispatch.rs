use petr_dispatch::{default_table, OpFrame, ShuffledOperationTable};

fn main() {
    let logical = default_table();
    let shuffled = ShuffledOperationTable::new(logical, 0xCAFE_BABE);
    let physical = shuffled.encode_selector(0x20);
    let out = shuffled.execute_physical(physical, OpFrame::text("2a")).unwrap();
    println!("logical 0x20 encoded as physical {physical:#04x}; output={:?}", out);
}
