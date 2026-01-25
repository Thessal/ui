
import unittest
from unittest.mock import MagicMock, patch
import os
import sys
import datetime

# Add src to path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../src')))

from rhetenor.aws import S3KlineWrapper

class TestS3KlineWrapperLogic(unittest.TestCase):
    def test_reconcile_and_put(self):
        # Setup Wrapper with Mock S3
        # We don't need real S3, just mock put_object
        with patch('boto3.client') as mock_boto:
             mock_s3 = MagicMock()
             mock_boto.return_value = mock_s3
             
             wrapper = S3KlineWrapper("bucket", "prefix")
             wrapper.s3 = mock_s3 # Explicit set in case init used return value
             
             # 1. Preload State
             # Timestamp A
             ts_a = "2024-01-25_1000"
             record_a = {
                 "timestamp": ts_a,
                 "fields": ["open", "close"],
                 "data": {"SYM1": [100, 100]}
             }
             wrapper.loaded_data_map = {ts_a: record_a}
             
             # 2. New Data (Update)
             # Same timestamp, same fields, new data for SYM1 (merge/update) and SYM2 (new)
             new_data_map = {
                 ts_a: {
                     "timestamp": ts_a,
                     "fields": ["open", "close"], # Match
                     "data": {
                         "SYM1": [100, 105], # Changed close
                         "SYM2": [200, 200]  # New symbol
                     }
                 },
                 "2024-01-25_1001": { # New timestamp entirely
                     "timestamp": "2024-01-25_1001",
                     "fields": ["open", "close"],
                     "data": {"SYM1": [100, 100]}
                 }
             }
             
             # 3. reconcile
             updates = wrapper.reconcile(new_data_map, exchange_code="J")
             
             # 4. Verify Local State Updated
             # Record A should be merged
             updated_a = wrapper.loaded_data_map[ts_a]
             self.assertEqual(updated_a['data']['SYM1'][1], 105) # Updated
             self.assertEqual(updated_a['data']['SYM2'][0], 200) # Added
             
             # New Record should be added
             self.assertIn("2024-01-25_1001", wrapper.loaded_data_map)
             
             # 5. Verify Updates Returned
             self.assertEqual(len(updates), 2)
             # Verify put NOT called by reconcile
             self.assertEqual(mock_s3.put_object.call_count, 0)

if __name__ == '__main__':
    unittest.main()
