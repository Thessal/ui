
import unittest
from unittest.mock import MagicMock, patch
import os
import sys
import datetime
from datetime import timedelta

# Add src to path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../src')))

from rhetenor.data import HantooKlineLogger

class TestHantooKlineLoggerReconcile(unittest.TestCase):
    @patch('rhetenor.data.S3KlineWrapper')
    @patch('rhetenor.data.HantooClient')
    @patch('rhetenor.data.datetime') # To mock now()
    @patch('time.sleep')
    def test_reconcile_logic(self, mock_sleep, mock_dt, mock_hantoo_cls, mock_wrapper_cls):
        # Setup Mocks
        mock_wrapper = MagicMock()
        mock_wrapper_cls.return_value = mock_wrapper
        
        mock_client = MagicMock()
        mock_hantoo_cls.return_value = mock_client
        
        # Mock holiday check (Say today is open)
        mock_client.check_holiday.return_value = {"output": [{"bass_dt": "20240125", "opnd_yn": "Y"}]}
        
        # Mock Time (Fixed time for consistent test)
        # Target Date: 2024-01-25
        fixed_now = datetime.datetime(2024, 1, 25, 10, 0, 0)
        mock_dt.now.return_value = fixed_now
        mock_dt.combine = datetime.datetime.combine
        mock_dt.strptime = datetime.datetime.strptime
        mock_dt.min = datetime.datetime.min
        
        # 1. Initialize Logger
        # Mock S3 having one record for 10:00 (Record A)
        # Record A: Symbol "005930" only
        timestamp_str = "2024-01-25_1000"
        record_a = {
            "timestamp": timestamp_str,
            "fields": ["open","high","low","close","volume","acc_vol"],
            "data": {
                "005930": [70000, 70100, 69900, 70000, 100, 1000]
            }
        }
        
        mock_wrapper.get.return_value = [record_a]
        
        logger = HantooKlineLogger(symbols=["005930", "000660"], exchange_code="J")

        # Verify initial load
        self.assertTrue(mock_wrapper.load.called)
        # We can't check logger.existing_data_map as it is removed/delegated.

        
        def api_side_effect(*args, **kwargs):
            symbol = kwargs.get('symbol')
            if symbol == "005930":
                return {}, {"output2": [{
                    "stck_cntg_hour": "100000", # 10:00:00
                    "stck_oprc": "70000", "stck_hgpr": "70100", "stck_lwpr": "69900", "stck_prpr": "70050", # Changed close
                    "cntg_vol": "150", "acml_vol": "1200"
                }]}
            if symbol == "000660":
                return {}, {"output2": [{
                    "stck_cntg_hour": "100000",
                    "stck_oprc": "140000", "stck_hgpr": "140500", "stck_lwpr": "139500", "stck_prpr": "140000",
                    "cntg_vol": "50", "acml_vol": "500"
                }]}
            return {}, {}
            
        mock_client.inquire_time_itemchartprice.side_effect = api_side_effect
        
        def reconcile_side_effect(new_data_map, exchange_code):
            return list(new_data_map.values())
        mock_wrapper.reconcile.side_effect = reconcile_side_effect
        
        logger.fetch_and_update()
        
        # 3. Verify Delegation
        # We verify wrapper.reconcile was called.
        self.assertTrue(mock_wrapper.reconcile.called)
        
        # Verify put NOT called (updates < 15)
        self.assertFalse(mock_wrapper.put.called)
        
        # Verify accumulation
        self.assertEqual(len(logger.updates), 1)
        
        updated_data = logger.updates[0]
        # Check Symbol A (005930) - Updated Close
        self.assertEqual(updated_data['data']['005930'][3], "70050")
        # Check Symbol B - New
        self.assertEqual(updated_data['data']['000660'][0], "140000")

if __name__ == '__main__':
    unittest.main()


if __name__ == '__main__':
    unittest.main()
