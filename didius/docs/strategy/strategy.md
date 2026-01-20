# `didius/oms/strategy.py`

This module defines the execution strategies available in the OMS.

## Base Class

### `Strategy`
Abstract base class for all strategies.
- `on_market_data(order_book)`: Called on book updates. Returns list of Orders to place.
- `on_timer(current_time)`: Called periodically. Returns list of Orders to place.

## Implementations

### `StopOrderStrategy`
- **Logic**: Monitors `OrderBook` mid-price.
- **Trigger**: When price crosses `stop_price`.
- **Action**: Places a matching Market or Limit order.

### `ImmediateOrCancelStrategy` (IOC)
- **Logic**: Client-side emulation. Checks available liquidity at or better than limit price.
- **Action**: Sends a Limit order for the matching quantity immediately. Terminates.

### `FillOrKillStrategy` (FOK)
- **Logic**: Client-side emulation. Checks if *full* quantity is available at price.
- **Action**: Sends Limit order if available, else does nothing. Terminates.

### `TWAPStrategy`
- **Logic**: Splits total quantity into slices over `total_duration` based on `interval`.
- **Action**: Sends a slice (Market Order) every interval.

### `VWAPStrategy` (Placeholder)
- **Logic**: Targeted participation based on volume profile. (Requires trade feed or volume delta details).
