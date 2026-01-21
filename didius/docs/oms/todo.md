# Future Tasks

## Multiple Venue Support (Deferred)
- [ ] **Adapter Aggregation**: Modify `OMSEngine` to hold a collection of adapters (e.g., `HashMap<VenueId, Arc<dyn Adapter>>`) instead of a single one.
- [ ] **Order Routing**: Implement logic to route `place_order` requests to the correct adapter based on the order's venue or symbol.
- [ ] **Liquidity Aggregation**: 
    -   Extend `OrderBook` to track `venue` per price level or maintain separate books per venue.
    -   Implement "Virtual Best Bid/Offer" (VBBO) aggregating liquidity from KRX, NXT, CME, etc.
- [ ] **Data Normalization**: Ensure all adapters normalize symbol names and price/quantity scales to a common format.

## Order/OrderBook
- [ ] Add `venue` field (KRX, NXT, CME) to `Order`, `Trade`, and `OrderBook` structs.