pub mod universe;
use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let utils_module = PyModule::new(m.py(), "utils")?;
    universe::register(&utils_module)?;
    m.add_submodule(&utils_module)?;
    Ok(())
}

pub fn parse_decimal(s: &str) -> anyhow::Result<rust_decimal::Decimal> {
    use std::str::FromStr;
    rust_decimal::Decimal::from_str(s).map_err(|e| anyhow::anyhow!(e))
}
