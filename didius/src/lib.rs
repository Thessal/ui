pub mod oms;
pub mod adapter;
pub mod strategy;
pub mod logger;

use pyo3::prelude::*;
use pyo3::types::PyModule;

/// A Python module implemented in Rust.
#[pymodule]
fn didius_oms(m: &Bound<'_, PyModule>) -> PyResult<()> {
    oms::register(m)?;
    Ok(())
}
