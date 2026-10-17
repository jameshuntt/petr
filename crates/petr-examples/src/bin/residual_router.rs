use petr_core::ResidualRouter;

fn main() {
    let router = ResidualRouter::new(7)
        .unwrap()
        .set(0, 0x10)
        .unwrap()
        .set(1, 0x11)
        .unwrap()
        .set(2, 0x20)
        .unwrap();
    for word in [0x1000usize, 0x1008, 0x1010, 0x1018] {
        println!(
            "word={word:#x} residue={} selector={:#04x}",
            word % router.modulus(),
            router.selector_for(word)
        );
    }
}
