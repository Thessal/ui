# `didius_oms::oms::order_book`

This module handles the Order Book management.

## Structs

### `PriceLevel`
Represents a single price level (Price, Quantity).

### `OrderBookDelta`
Represents an incremental update to the Order Book.

**Attributes:**
- `symbol` (`String`)
- `bids` (`Vec<(f64, i64)>`): List of Bid updates (Price, Qty). Qty <= 0 means delete.
- `asks` (`Vec<(f64, i64)>`): List of Ask updates.
- `update_id` (`i64`)
- `timestamp` (`f64`)

### `OrderBook`
Maintains the Limit Order Book (LOB) state.

**Attributes:**
- `symbol` (`String`)
- `bids` (`HashMap<String, i64>`): Map of price -> quantity. Keys are strings to avoid float precision hashing issues in simple export, though internally logic may optimize.
- `asks` (`HashMap<String, i64>`): Map of price -> quantity.
- `last_update_id` (`i64`)
- `timestamp` (`f64`)

**Methods:**
- `rebuild(bids, asks, last_update_id, timestamp)`: Reinitialize book from snapshot.
- `apply_delta(delta)`: Apply an `OrderBookDelta`.
- `get_best_bid() -> Option<(f64, i64)>`: Returns (Price, Qty) of best (highest) bid.
- `get_best_ask() -> Option<(f64, i64)>`: Returns (Price, Qty) of best (lowest) ask.
- `get_mid_price() -> Option<f64>`: `(Best Bid + Best Ask) / 2`.
- `validate() -> bool`: Checks for crossed book (Best Bid >= Best Ask).
