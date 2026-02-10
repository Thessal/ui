import asyncio
import logging
import sys
import os

# Ensure we can import didius
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../src_python')))

from didius import Didius, Order, OrderSide, OrderType
# from didius.core import HantooAdapter # Import Adapter to pass to Didius

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

async def main():
    logger.info("Starting Didius Async Verification")
    
    # 1. Create Adapter (Mock or Real)
    # We use None which might default to Mock if we handled it, 
    # but based on code we need to pass an adapter.
    # Let's try to create a MockAdapter from rust side? 
    # exposed as HantooAdapter? No, MockAdapter is not exposed in __init__.py?
    # __init__.py exposes HantooAdapter, HantooNightAdapter.
    # It does NOT expose MockAdapter.
    # Butoms/interface.rs:46 handles None -> MockAdapter.
    # Client::new (rust) extracts adapter.
    # If we pass None, extract_adapter might fail.
    # Let's check extract_adapter in interface.rs.

    try:
        # For now, let's try passing None and see if it fails (expect failure if strict).
        # Or better, we should expose MockAdapter or allow None.
        # user said "simulate Hantoo responses using the existing Mock adapter".
        # If MockAdapter is not exposed to Python, we can't instantiate it easily unless Interface does it.
        # But Client is new.
        # For now, I'll comment out actual connection until I know how to instantiate adapter.
        
        client = Didius(venue="mock")
        await client.connect()
        logger.info("Connected")
        
        # Verify we can send an order (mock adapter should handle this)
        # Required args: symbol, side, order_type, quantity
        order = Order("AAPL", OrderSide.BUY, OrderType.LIMIT, 100) 
        client.place_order(order)
        logger.info("Order placed")
        
        await asyncio.sleep(1)
        
        await client.disconnect()
        logger.info("Disconnected")
        
    except Exception as e:
        logger.error(f"Error: {e}")

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
