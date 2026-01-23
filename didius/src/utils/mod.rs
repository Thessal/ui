pub mod universe;
use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let utils_module = PyModule::new(m.py(), "utils")?;
    universe::register(&utils_module)?;
    m.add_submodule(&utils_module)?;
    Ok(())
}
