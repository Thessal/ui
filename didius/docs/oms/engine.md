# `didius::oms::engine`

The `OMSEngine` is the core coordinator implemented in Rust for high performance, exposed to Python.

## Structs

### `OMSEngine`
The central engine that manages orders, strategies, and state.

**Thread Safety:**
- Uses `Arc<Mutex<...>>` for internal state (`order_books`, `orders`, `account`).
- Capable of running a background thread for strategy timers (Rust thread).

**Attributes (Internal Rust State):**
- `adapter`: Reference to the Python Adapter object (PyObject).
- `order_books`: `HashMap<String, OrderBook>`.
- `account`: `AccountState`.
- `orders`: `HashMap<String, Order>`.

**Methods (Exposed to Python):**
- `new(adapter)`: Initialize with a Python Adapter instance.
- `start(account_id=None)`: 
    - connects adapter.
    - initializes account.
    - starts background timer thread in Rust.
- `stop()`: Stops engine and background thread.
- `initialize_symbol(symbol)`: Calls adapter to get snapshot and sets up book.
- `initialize_account(account_id)`: Calls adapter to get snapshot.
- `send_order(order)`:
    - Assigns UUID if missing.
    - Checks for Strategy (placeholder).
    - If valid, adds to local state and calls `adapter.place_order(order)`.
- `on_market_data(data)`: Callback for adapter to inject market data (`OrderBook` or `OrderBookDelta`).
- `on_account_update(data)`: Callback for account updates.
- `get_account() -> AccountState`: Returns a copy of the current account state.

## Integration

The Engine expects an `adapter` object from Python that implements:
- `connect()`
- `disconnect()`
- `get_order_book_snapshot(symbol)` -> Returns `OrderBook`
- `get_account_snapshot(account_id)` -> Returns `AccountState`
- `place_order(order)` -> Returns `bool`
