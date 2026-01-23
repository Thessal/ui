use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::sync::Arc;
use crate::adapter::hantoo::HantooAdapter;
use crate::adapter::hantoo_ngt_futopt::HantooNightAdapter;
use crate::adapter::Adapter;
use serde_json::Value;
use std::sync::mpsc;
use crate::adapter::IncomingMessage;
use pyo3::types::PyAny;

#[pyclass(name = "HantooAdapter")]
pub struct PyHantooAdapter {
    pub(crate) adapter: Arc<HantooAdapter>,
}

#[pymethods]
impl PyHantooAdapter {
    #[new]
    fn new(config_path: String) -> PyResult<Self> {
        let adapter = HantooAdapter::new(&config_path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyHantooAdapter { adapter: Arc::new(adapter) })
    }

    fn subscribe_market(&self, symbols: Vec<String>) -> PyResult<()> {
        self.adapter.subscribe_market(&symbols)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    fn connect(&self) -> PyResult<()> {
        self.adapter.connect()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }
    
    fn set_debug_mode(&self, enabled: bool) {
        self.adapter.set_debug_mode(enabled);
    }
}

#[pyclass(name = "HantooNightAdapter")]
pub struct PyHantooNightAdapter {
    pub(crate) adapter: Arc<HantooNightAdapter>,
}

#[pymethods]
impl PyHantooNightAdapter {
    #[new]
    fn new(config_path: String) -> PyResult<Self> {
        let adapter = HantooNightAdapter::new(&config_path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyHantooNightAdapter { adapter: Arc::new(adapter) })
    }

    fn subscribe(&self, symbol: String) -> PyResult<()> {
        self.adapter.subscribe(&symbol)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    fn connect(&self) -> PyResult<()> {
        self.adapter.connect()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }
    
    fn set_debug_mode(&self, enabled: bool) {
        self.adapter.set_debug_mode(enabled);
    }

    fn get_night_future_list(&self, py: Python) -> PyResult<PyObject> {
        let list = self.adapter.get_night_future_list()
             .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(json_to_py(py, &Value::Array(list))?)
    }

    fn get_night_option_list(&self, py: Python) -> PyResult<PyObject> {
        let list = self.adapter.get_night_option_list()
             .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(json_to_py(py, &Value::Array(list))?)
    }
}

fn json_to_py(py: Python, value: &Value) -> PyResult<PyObject> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => Ok(b.into_py(py)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_py(py))
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_py(py))
            } else {
                Ok(py.None())
            }
        },
        Value::String(s) => Ok(s.into_py(py)),
        Value::Array(arr) => {
            let list = PyList::empty(py);
            for v in arr {
                list.append(json_to_py(py, v)?)?;
            }
            Ok(list.into())
        },
        Value::Object(obj) => {
            let dict = PyDict::new(py);
            for (k, v) in obj {
                dict.set_item(k, json_to_py(py, v)?)?;
            }
            Ok(dict.into())
        }
    }
}

pub fn extract_adapter(obj: &Bound<'_, PyAny>) -> PyResult<Arc<dyn Adapter>> {
    if let Ok(py_adapter) = obj.extract::<Py<PyHantooAdapter>>() {
        let adapter = py_adapter.borrow(obj.py()).adapter.clone();
        return Ok(adapter as Arc<dyn Adapter>);
    }
    
    if let Ok(py_night) = obj.extract::<Py<PyHantooNightAdapter>>() {
        let adapter = py_night.borrow(obj.py()).adapter.clone();
        return Ok(adapter as Arc<dyn Adapter>);
    }

    Err(pyo3::exceptions::PyTypeError::new_err("Unknown Adapter Type"))
}

pub fn initialize_monitor(obj: &Bound<'_, PyAny>, sender: mpsc::Sender<IncomingMessage>) -> PyResult<()> {
    if let Ok(py_adapter) = obj.extract::<Py<PyHantooAdapter>>() {
        py_adapter.borrow(obj.py()).adapter.set_monitor(sender);
        return Ok(());
    }
    
    if let Ok(py_night) = obj.extract::<Py<PyHantooNightAdapter>>() {
        py_night.borrow(obj.py()).adapter.set_monitor(sender);
        return Ok(());
    }
    
    Err(pyo3::exceptions::PyTypeError::new_err("Unknown Adapter Type or Adapter does not support monitoring"))
}
