use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Local};
use uuid::Uuid;

#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum OrderSide {
    BUY,
    SELL,
}

#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum OrderType {
    MARKET,
    LIMIT,
}

#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum OrderState {
    CREATED,
    PENDING_NEW,
    NEW,
    PARTIALLY_FILLED,
    FILLED,
    CANCELED,
    REJECTED,
    PENDING_CANCEL,
    PENDING_REPLACE,
}

#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum ExecutionStrategy {
    IOC,
    FOK,
    VWAP,
    TWAP,
    STOP_LOSS,
    TAKE_PROFIT,
    NONE,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    #[pyo3(get, set)]
    pub symbol: String,
    #[pyo3(get, set)]
    pub side: OrderSide,
    #[pyo3(get, set)]
    pub order_type: OrderType,
    #[pyo3(get, set)]
    pub quantity: i64,
    #[pyo3(get, set)]
    pub price: Option<f64>,
    #[pyo3(get, set)]
    pub order_id: Option<String>,
    #[pyo3(get, set)]
    pub exchange_order_id: Option<String>,
    #[pyo3(get, set)]
    pub state: OrderState,
    #[pyo3(get, set)]
    pub filled_quantity: i64,
    #[pyo3(get, set)]
    pub average_fill_price: f64,
    #[pyo3(get, set)]
    pub strategy: ExecutionStrategy,
    // strategy_params is Dict[str, Any] in Python. 
    // In Rust/PyO3 for simple storage we can use PyObject or HashMap<String, String> if simple.
    // For now, let's use HashMap<String, String> for simplicity in Rust, 
    // but we might need more complex types later.
    #[pyo3(get, set)]
    pub strategy_params: HashMap<String, String>, 
    #[pyo3(get, set)]
    pub limit_price: Option<f64>,
    #[pyo3(get, set)]
    pub stop_price: Option<f64>,
    
    // Dates are a bit tricky with PyO3 <-> Chrono directly without wrappers sometimes,
    // but recent PyO3 versions support it well with `chrono` feature.
    // We will store as string or timestamps if needed, but let's try direct support.
    // Actually, passing datetime structs back and forth requires some care.
    // Storing as timestamp (f64 or i64) might be safer for MVP.
    // Let's use f64 (timestamp) for simplicity and performance.
    #[pyo3(get, set)]
    pub created_at: f64, 
    #[pyo3(get, set)]
    pub updated_at: f64,
    #[pyo3(get, set)]
    pub error_message: Option<String>,
}

#[pymethods]
impl Order {
    #[new]
    #[pyo3(signature = (symbol, side, order_type, quantity, price=None, strategy=None, strategy_params=None, stop_price=None))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: String,
        side: OrderSide,
        order_type: OrderType,
        quantity: i64,
        price: Option<f64>,
        strategy: Option<ExecutionStrategy>,
        strategy_params: Option<HashMap<String, String>>,
        stop_price: Option<f64>,
    ) -> Self {
        let now = Local::now().timestamp_millis() as f64 / 1000.0;
        Order {
            symbol,
            side,
            order_type,
            quantity,
            price,
            order_id: None,
            exchange_order_id: None,
            state: OrderState::CREATED,
            filled_quantity: 0,
            average_fill_price: 0.0,
            strategy: strategy.unwrap_or(ExecutionStrategy::NONE),
            strategy_params: strategy_params.unwrap_or_default(),
            limit_price: None,
            stop_price,
            created_at: now,
            updated_at: now,
            error_message: None,
        }
    }

    #[pyo3(signature = (new_state, msg=None))]
    pub fn update_state(&mut self, new_state: OrderState, msg: Option<String>) {
        self.state = new_state;
        self.updated_at = Local::now().timestamp_millis() as f64 / 1000.0;
        if let Some(m) = msg {
            self.error_message = Some(m);
        }
    }

    #[getter]
    fn is_active(&self) -> bool {
        matches!(
            self.state,
            OrderState::PENDING_NEW
                | OrderState::NEW
                | OrderState::PARTIALLY_FILLED
                | OrderState::PENDING_CANCEL
                | OrderState::PENDING_REPLACE
        )
    }
    
    fn __repr__(&self) -> String {
        format!("{:?}", self)
    }
}
