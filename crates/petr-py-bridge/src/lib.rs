#![cfg_attr(feature = "python", allow(unsafe_op_in_unsafe_fn))]

//! Optional PyO3 bridge skeleton for using the dispatch engine from Python.

use petr_dispatch::{default_table, OpFrame};

/// Pure Rust helper that mirrors the Python-facing function.
pub fn run_prefix_suffix(text: &str) -> String {
    let table = default_table();
    let frame = table.execute(0x01, OpFrame::text(text)).and_then(|f| table.execute(0x02, f));
    frame.map(|f| f.text).unwrap_or_else(|e| format!("error: {e}"))
}

#[cfg(feature = "python")]
mod py {
    use pyo3::prelude::*;
    use pyo3::types::PyModule;

    /// Run a tiny dispatch pipeline from Python.
    #[pyfunction]
    fn run_prefix_suffix_py(text: &str) -> String {
        crate::run_prefix_suffix(text)
    }

    /// Python module initializer.
    #[pymodule]
    fn petr_py_bridge(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add_function(wrap_pyfunction!(run_prefix_suffix_py, m)?)?;
        Ok(())
    }
}
