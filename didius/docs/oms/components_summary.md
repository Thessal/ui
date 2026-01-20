# Components of a Trading System

## 1. Trading System Architecture
The trading system connects to external venues (exchanges, ECNs) to collect market data and execute orders.

### Critical Components
1.  **Gateways**: 
    *   **Function**: Interface with venues. Handle protocol translation (e.g., FIX, specific API protocols).
    *   **Responsibilities**: 
        *   Data Collection: Subscribe to price/book updates.
        *   Order Routing: Send orders to venues and receive execution reports/acknowledgments.
    *   **Performance**: Must be highly optimized for latency (especially in HFT).

2.  **Book Builder**:
    *   **Function**: Constructs the Local Order Book (LOB) from gateway price updates.
    *   **Logic**:
        *   Maintains Bids and Asks sorted by price.
        *   Updates must be O(1) or O(log n) (using Maps/Vectors).
        *   Handles Snapshots vs. Incremental Updates.

3.  **Strategy**:
    *   **Function**: The "Brain" of the system.
    *   **Logic**:
        *   **Signal Generation**: Decides *when* to trade based on LOB data or other signals.
        *   **Execution**: Decides *how* to trade (e.g., passive posting vs. crossing the spread).

4.  **Order Management System (OMS)**:
    *   **Function**: Central hub for order lifecycle management.
    *   **Responsibilities**:
        *   Validates orders from Strategy (risk checks, position limits).
        *   Tracks state: Created -> PendingNew -> New -> PartiallyFilled -> Filled / Canceled / Rejected.
        *   Handles error reporting and state reconciliation.

### Non-Critical Components
*   **Command and Control (CNC)**: User interface for monitoring and manual intervention.
*   **Position Server**: Tracks aggregated positions and P&L.
*   **Logging/Reporting**: For debugging and post-trade analysis.

## 2. Order Book Management
*   **Data Structures**: 
    *   Use Hash Maps for O(1) order ID lookups.
    *   Use Vectors/Trees for price levels to allow fast iteration of best prices.
*   **Operations**: Insertion, Amendment, Cancellation.

