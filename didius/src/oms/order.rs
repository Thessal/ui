use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Local};
use uuid::Uuid;
use rust_decimal::Decimal;
use std::str::FromStr;

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
    
    // Internal Decimal fields, exposed via custom getters/setters as String
    pub price: Option<Decimal>,
    
    #[pyo3(get, set)]
    pub order_id: Option<String>,
    #[pyo3(get, set)]
    pub exchange_order_id: Option<String>,
    #[pyo3(get, set)]
    pub state: OrderState,
    #[pyo3(get, set)]
    pub filled_quantity: i64,
    
    pub average_fill_price: Decimal,
    
    #[pyo3(get, set)]
    pub strategy: ExecutionStrategy,
    #[pyo3(get, set)]
    pub strategy_params: HashMap<String, String>, 
    
    pub limit_price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    
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
        price: Option<String>,
        strategy: Option<ExecutionStrategy>,
        strategy_params: Option<HashMap<String, String>>,
        stop_price: Option<String>,
    ) -> Self {
        let now = Local::now().timestamp_millis() as f64 / 1000.0;
        
        let price_dec = price.and_then(|p| Decimal::from_str(&p).ok());
        let stop_dec = stop_price.and_then(|p| Decimal::from_str(&p).ok());

        Order {
            symbol,
            side,
            order_type,
            quantity,
            price: price_dec,
            order_id: None,
            exchange_order_id: None,
            state: OrderState::CREATED,
            filled_quantity: 0,
            average_fill_price: Decimal::ZERO,
            strategy: strategy.unwrap_or(ExecutionStrategy::NONE),
            strategy_params: strategy_params.unwrap_or_default(),
            limit_price: None,
            stop_price: stop_dec,
            created_at: now,
            updated_at: now,
            error_message: None,
        }
    }

    #[getter(price)]
    fn get_price(&self) -> Option<String> {
        self.price.map(|d| d.to_string())
    }

    #[setter(price)]
    fn set_price(&mut self, value: Option<String>) {
        self.price = value.and_then(|s| Decimal::from_str(&s).ok());
    }

    #[getter(average_fill_price)]
    fn get_average_fill_price(&self) -> String {
        self.average_fill_price.to_string()
    }
    
    // No setter needed for fill price usually, but if needed for persistence/testing:
    // #[setter(average_fill_price)] ...

    #[getter(stop_price)]
    fn get_stop_price(&self) -> Option<String> {
        self.stop_price.map(|d| d.to_string())
    }

    #[setter(stop_price)]
    fn set_stop_price(&mut self, value: Option<String>) {
        self.stop_price = value.and_then(|s| Decimal::from_str(&s).ok());
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
