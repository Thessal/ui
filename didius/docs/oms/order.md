# `didius_oms::oms::order`

This module defines the data structures and enumerations related to orders in the Rust OMS implementation.

## Enumerations

### `OrderSide`
- `BUY`
- `SELL`

### `OrderType`
- `MARKET`
- `LIMIT`

### `OrderState`
Tracks the lifecycle of an order:
- `CREATED`, `PENDING_NEW`, `NEW`
- `PARTIALLY_FILLED`, `FILLED`
- `CANCELED`, `REJECTED`
- `PENDING_CANCEL`, `PENDING_REPLACE`

### `ExecutionStrategy`
Defines the execution strategy for the order:
- `IOC` (Immediate-Or-Cancel)
- `FOK` (Fill-Or-Kill)
- `VWAP` (Volume Weighted Average Price)
- `TWAP` (Time Weighted Average Price)
- `STOP_LOSS`
- `TAKE_PROFIT`
- `NONE` (Default)

## Structs

### `Order`
A Rust `struct` representing a single order, exposed to Python via PyO3.

**Attributes:**
- `symbol` (`String`): Trading pair/symbol.
- `side` (`OrderSide`): Buy or Sell.
- `order_type` (`OrderType`): Market or Limit.
- `quantity` (`i64`): Original Order quantity.  
- `price` (`Option<f64>`): Limit price (None for Market).
- `order_id` (`Option<String>`): Local unique identifier.
- `exchange_order_id` (`Option<String>`): Identifier assigned by the exchange.
- `state` (`OrderState`): Current state of the order.
- `filled_quantity` (`i64`): Cumulative filled quantity.
- `average_fill_price` (`f64`): Average price of fills.
- `strategy` (`ExecutionStrategy`): Strategy to use for execution.
- `strategy_params` (`HashMap<String, String>`): Parameters for the strategy.
- `limit_price` (`Option<f64>`): Limit price for Stop Limit orders.
- `stop_price` (`Option<f64>`): Stop price for Stop orders.
- `created_at` (`f64`): Timestamp of creation (Unix timestamp).
- `updated_at` (`f64`): Timestamp of last update (Unix timestamp).
- `error_message` (`Option<String>`): Error details if rejected/failed.

**Methods:**
- `new(...)`: Constructor.
- `update_state(new_state, msg=None)`: Transitions the order to a new state and updates the timestamp.
- `is_active` (property): Returns `True` if the order is in an active state.
