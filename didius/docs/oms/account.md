# `didius::oms::account`

This module defines `AccountState` and `Position` for tracking user funds and holdings.

## Structs

### `Position`
Tracks a position in a single symbol.

**Attributes:**
- `symbol` (`String`): Symbol identifier.
- `quantity` (`i64`): Signed integer quantity (Positive = Long, Negative = Short).
- `average_price` (`f64`): Average entry price.
- `current_price` (`f64`): Latest known market price (used for Unrealized PnL).

**Methods:**
- `unrealized_pnl` (property): `(current_price - average_price) * quantity`.

### `AccountState`
Represents the snapshot of an account's balance and positions.

**Attributes:**
- `balance` (`f64`): Cash balance.
- `locked` (`f64`): Funds locked in active orders.
- `positions` (`HashMap<String, Position>`): Map of Symbol -> Position.

**Methods:**
- `rebuild(balance, locked, positions)`: Replaces the entire state with a snapshot.
- `update_position(symbol, quantity, price)`: Updates or adds a position directly.
- `on_execution(symbol, side, quantity, price, fee)`: Updates balance and position based on a trade execution.
    - Decrements balance by cost + fee.
    - Updates position quantity and Weighted Average Price.
