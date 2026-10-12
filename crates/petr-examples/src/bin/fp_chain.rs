use petr_dispatch::{default_table, ChainProgram, ChainStep, OpArg, OpFrame};
use petr_core::SelectorWord;

fn main() {
    let table = default_table();
    let mut chain = ChainProgram::new();

    // Keep the validating octet operation first while the text is still hex.
    // Then run string decoration after normalization.
    chain.push(ChainStep { selector: SelectorWord::new(1, 1, 0x20), arg: OpArg::default(), next: Some(1) });
    chain.push(ChainStep { selector: SelectorWord::new(1, 1, 0x01), arg: OpArg::default(), next: Some(2) });
    chain.push(ChainStep { selector: SelectorWord::new(1, 1, 0x02), arg: OpArg::default(), next: None });

    let out = chain.run(&table, 0, OpFrame::text("ff")).unwrap();
    println!("chain output = {:?}", out);
}
