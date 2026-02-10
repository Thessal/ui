import asyncio
import logging
from typing import Optional, Dict, Any, List, Union
from .core import Client as RustClient, Order
from decimal import Decimal

logger = logging.getLogger(__name__)

class Didius:
    """
    Main entry point for Didius client.
    Provides an asyncio interface compatible with ib_async style.
    Wrapper around the Rust-based Core Client.
    """
    def __init__(self, venue: str = "mock", config_path: Optional[str] = None, s3_bucket: Optional[str] = None, s3_region: Optional[str] = None, s3_prefix: Optional[str] = None):
        """
        Initialize the Didius client.
        
        Args:
            venue: Backend venue ("mock", "hantoo", "hantoo_night").
            config_path: Path to configuration file (required for Hantoo venues).
            s3_bucket: Optional AWS S3 bucket for logging.
            s3_region: Optional AWS region.
            s3_prefix: Optional prefix for log files in S3.
        """
        self._loop = asyncio.get_event_loop()
        self.conn = RustClient(venue, config_path, s3_bucket, s3_region, s3_prefix)
        self.running = False
        self._message_task = None
        self.handlers = [] # List of callbacks

    async def connect(self):
        """Connect to the backend adapter."""
        # Run blocking connect in thread executor
        await self._loop.run_in_executor(None, self.conn.connect)
        self.running = True
        self._message_task = self._loop.create_task(self._process_messages())
        logger.info("Didius Client Connected")

    async def disconnect(self):
        """Disconnect from the backend."""
        self.running = False
        if self._message_task:
            self._message_task.cancel()
            try:
                await self._message_task
            except asyncio.CancelledError:
                pass
        
        await self._loop.run_in_executor(None, self.conn.disconnect)
        logger.info("Didius Client Disconnected")

    def place_order(self, order: Order) -> bool:
        """Place an order."""
        # Blocking call for now, could be async wrapped if needed but place_order is usually fast HTTP
        return self.conn.place_order(order)

    def cancel_order(self, order_id: str) -> bool:
        """Cancel an order."""
        return self.conn.cancel_order(order_id)
        
    def reqMktData(self, contract: Union[str, List[str]], genericTickList: str = "", snapshot: bool = False, regulatorySnapshot: bool = False, mktDataOptions: List[Any] = None):
        """
        Request market data (subscribe).
        Matches ib_async signature roughly but tailored for didius.
        contract: Symbol string or list of symbol strings.
        """
        if isinstance(contract, str):
            symbols = [contract]
        else:
            symbols = contract
            
        # Call subscribe on Rust Client
        # Rust Client::subscribe takes generic list? No, Vec<String>.
        self.conn.subscribe(symbols)

    async def _process_messages(self):
        """Loop to fetch messages from Rust core."""
        while self.running:
            try:
                # fetch_message with timeout (0.1s)
                # We use run_in_executor to avoid blocking asyncio loop
                msg_json = await self._loop.run_in_executor(None, self.conn.fetch_message, 0.1)
                
                if msg_json:
                    # Message received (JSON string).
                    # We can parse it here or dispatch generic event.
                    # For compatibility with ib_async, we might trigger events like 'updatePortfolio', 'execDetails', etc.
                    # For now just log or print.
                    # Or implement a callback system.
                    for handler in self.handlers:
                        try:
                            if asyncio.iscoroutinefunction(handler):
                                await handler(msg_json)
                            else:
                                handler(msg_json)
                        except Exception as e:
                            logger.error(f"Error in handler: {e}")
                            
            except Exception as e:
                logger.error(f"Error in message loop: {e}")
                await asyncio.sleep(1)

    def add_handler(self, callback):
        self.handlers.append(callback)
