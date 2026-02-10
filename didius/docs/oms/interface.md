# `didius::Client`

The `Client` class (exposed as `Didius` in Python) provides the main entry point for the Order Management System. It manages the connection to the underlying venue adapter and provides a unified interface for trading.

## Class `Didius`

**Constructor:**
- `Didius(venue: str, config_path: str = None, s3_bucket: str = None, s3_region: str = None, s3_prefix: str = None)`
    - `venue`: The trading venue to connect to. Supported values:
        - `"hantoo"`: Korea Investment & Securities (KIS)
        - `"hantoo_night"`: KIS Night Market (Derivatives)
        - `"mock"`: Mock environment for testing
    - `config_path`: Path to the configuration file (required for "hantoo" and "hantoo_night").
    - `s3_*`: Optional parameters for S3 logging.

**Methods:**

- `connect() -> None`:
    - Connects to the venue.

- `disconnect() -> None`:
    - Disconnects from the venue.

- `place_order(order: Order) -> bool`:
    - Submits an order. Returns `True` if successfully submitted to the adapter.

- `cancel_order(order_id: str) -> bool`:
    - Cancels an order.

- `update_order(order_id: str, price: str = None, qty: int = None) -> bool`:
    - Modifies an existing order.

- `subscribe(symbols: List[str]) -> None`:
    - Subscribes to market data for the given symbols.

- `fetch_message(timeout_sec: float) -> Optional[str]`:
    - Fetches the next incoming message (order update, trade, etc.) as a JSON string.
    - Returns `None` if timeout occurs.

- `get_account_state(account_id: str) -> Optional[str]`:
    - Returns a JSON snapshot of the account state.

- `get_order_book(symbol: str) -> Optional[str]`:
    - Returns a JSON snapshot of the order book.
