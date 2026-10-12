use petr_dispatch::{default_dense_table, OpFrame};

fn main() {
    let table = default_dense_table();
    let mut frame = OpFrame::text("ff");

    for selector in [0x20, 0x01, 0x02, 0x99] {
        let before = frame.clone();
        frame = table.execute(selector, frame).unwrap();
        println!("selector=0x{selector:02x} op={} before={before:?} after={frame:?}", table.name(selector));
    }
}
