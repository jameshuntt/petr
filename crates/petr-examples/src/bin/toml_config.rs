use petr_dispatch::{OpFrame, table_from_toml};

fn main() {
    let config = include_str!("../../configs/operations.toml");
    let table = table_from_toml(config).unwrap();
    let a = table.execute(0x10, OpFrame::text("17")).unwrap();
    let b = table.execute(0x21, OpFrame::text("FF")).unwrap();
    println!("octal multiply => {:?}", a);
    println!("octet add => {:?}", b);
}
