// Link helper for PyO3 extension-module builds on macOS.
//
// The workspace all-features gate enables `petr_py_bridge/python`, which pulls
// in PyO3's `extension-module` feature. On macOS those extension modules are
// expected to resolve CPython symbols from the loading interpreter instead of
// linking libpython directly, so cargo needs the extension-module linker args
// when it builds the cdylib. Maturin usually supplies this for wheel builds;
// this build script makes plain `cargo test --workspace --all-features` work too.
fn main() {
    #[cfg(all(feature = "python", target_os = "macos"))]
    pyo3_build_config::add_extension_module_link_args();
}
