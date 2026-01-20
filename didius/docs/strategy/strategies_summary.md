# Execution Strategies and Market Modeling

## 1. Execution Algorithms
Strategies aim to minimize market impact and execution costs while capturing alpha.

*   **Key Strategies**:
    *   **VWAP (Volume Weighted Average Price)**: Executes orders in proportion to the market volume distribution over the day. Requires accurate volume profiles.
    *   **Stop-Limit / Stop-Market**: Trigger-based orders. System monitors market price; when trigger price is reached, sends a Limit or Market order.

## 2. Trade Data Modeling
To execute effectively, the system must model market behavior.

*   **Normalization**:
    *   **Order Size**: Normalized by Average Daily Volume (ADV).
    *   **Time**: Characteristic Time (time for quote update) vs. Wall Clock Time.
    *   **Volatility**: Mid-quote volatility used to normalize price deviations.

*   **Profiles**:
    *   **Volume Profile**: "Volume Smile" (U-shaped). High activity at Open and Close.
    *   **Intraday Seasonality**: Strategies must adapt to time-of-day liquidity patterns.

## 3. Market Microstructure Signals
*   **Quote Imbalance**: Ratio of Bid Size vs. Ask Size. Predictive of short-term price moves.
*   **Microprice**: Weighted mid-price adjusting for imbalance.
*   **Hawkes Processes**: Advanced modeling of order arrival intensities (self-exciting processes) to predict liquidity and volatility clustering.

## 4. Limit Order Book Dynamics
*   **Event Types**: Limit Order Submission, Market Order Execution, Cancellation.
*   **Queuing Models**: Estimating probability of execution based on queue position.
