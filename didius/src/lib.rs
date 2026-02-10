pub mod oms;
pub mod adapter;
pub mod strategy;
pub mod logger;
pub mod utils;
pub mod message;
pub mod state;
pub mod client;

use pyo3::prelude::*;
use pyo3::types::PyModule;

/// A Python module implemented in Rust.
#[pymodule]
fn core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    oms::register(m)?;
    utils::register(m)?;
    m.add_class::<client::Client>()?;
    Ok(())
}
