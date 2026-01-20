# `didius_oms::oms::interface`

The `Interface` class provides the main entry point for Python-Rust Inter-Process Communication (IPC) via PyO3. It replaces the HTTP-based interface used in the pure Python implementation.

## Class `Interface`

**Constructor:**
- `Interface(adapter)`: Initializes the OMS with a Python Adapter instance.

**Methods:**

- `start(account_id=None)`:
    - Starts the OMS Engine.
    - Connects the adapter.
    - Starts the internal strategy timer loop.

- `stop()`:
    - Stops the OMS Engine and disconnects the adapter.

- `place_order(order: Order) -> str`:
    - Submits an order to the OMS.
    - Returns the `order_id`.
    - `order` must be an instance of `didius_oms.Order`.

- `get_order_book(symbol: str) -> dict`:
    - Returns the current state of the Order Book for `symbol` as a dictionary.
    - Structure:
      ```json
      {
        "symbol": "AAPL",
        "bids": {"150.0": 100, ...},
        "asks": {"151.0": 50, ...},
        "last_update_id": 123,
        "timestamp": 123456789.0
      }
      ```

- `get_account() -> dict`:
    - Returns the current account state as a dictionary.
    - Structure:
      ```json
      {
        "balance": 10000.0,
        "locked": 500.0,
        "positions": {
           "AAPL": {
              "symbol": "AAPL",
              "quantity": 10,
              "average_price": 145.0,
              "current_price": 150.0,
              "unrealized_pnl": 50.0
           }
        }
      }
      ```

- `init_symbol(symbol: str)`:
    - Initializes a symbol by fetching the snapshot via the adapter.
