fn main() {
    let out = petr_py_bridge::run_prefix_suffix("Data");
    println!("Python bridge core result = {out}");
    println!(
        "To build the PyO3 module later: maturin develop -m crates/petr_py_bridge/Cargo.toml --features python"
    );
}
