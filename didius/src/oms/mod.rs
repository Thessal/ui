pub mod order;
pub mod order_book;
pub mod account;
pub mod engine;
// pub mod interface;

use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<order::OrderSide>()?;
    m.add_class::<order::OrderType>()?;
    m.add_class::<order::OrderState>()?;
    m.add_class::<order::ExecutionStrategy>()?;
    m.add_class::<order::Order>()?;
    
    // OrderBook and AccountState are no longer exposed directly.
    // They are accessed via Interface returning Dicts.

    // m.add_class::<interface::Interface>()?;
    // m.add_class::<crate::adapter::interface::PyHantooAdapter>()?;
    // m.add_class::<crate::adapter::interface::PyHantooNightAdapter>()?;
    Ok(())
}
