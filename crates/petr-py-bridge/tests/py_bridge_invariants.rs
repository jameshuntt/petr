#[test]
fn pure_rust_python_bridge_helper_uses_dispatch_pipeline() {
    assert_eq!(
        petr_py_bridge::run_prefix_suffix("Data"),
        "Prefix_Data_Suffix"
    );
}
