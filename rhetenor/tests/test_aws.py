
import unittest
import time
import os
import sys
from datetime import datetime
import random
import json
import zstandard as zstd
import io
from unittest.mock import MagicMock, patch

# Ensure src is in path
sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '../src')))

from rhetenor.aws import S3KlineWrapper

class TestS3KlineWrapperMocks(unittest.TestCase):
    def setUp(self):
        self.mock_s3 = MagicMock()
        self.wrapper = S3KlineWrapper("test-bucket", "test-prefix")
        self.wrapper.s3 = self.mock_s3

    def test_put_splitting(self):
        # Data spanning 2 days
        data = [
            {
                "timestamp": "20240101230000", 
                "fields": ["open"], 
                "data": {"005930": ["100"]}
            },
            {
                "timestamp": "20240102010000", 
                "fields": ["open"], 
                "data": {"005930": ["105"]}
            }
        ]
        
        self.wrapper.put(data)
        
        # Verify put_object called twice
        self.assertEqual(self.mock_s3.put_object.call_count, 2)
        
        cal1_args = self.mock_s3.put_object.call_args_list[0]
        key1 = cal1_args.kwargs['Key']
        # First file should be for 20240101
        self.assertIn("20240101", key1)
        
        call2_args = self.mock_s3.put_object.call_args_list[1]
        key2 = call2_args.kwargs['Key']
        # Second file should be for 20240102
        self.assertIn("20240102", key2)

    def test_get_optimized_query(self):
        # We assume self.prefix = "test-prefix"
        
        # Mocking S3 response
        self.mock_s3.get_paginator.return_value.paginate.return_value = []
        
        start = datetime(2024, 1, 1, 10, 0)
        end = datetime(2024, 1, 3, 10, 5) 
        # Range covers 2024-01-01, 02, 03
        
        self.wrapper.get(start, end)
        
        # Verify calls to list_objects_v2 (via paginator) used correct prefixes
        # We expect 3 calls (for 3 days) or 3 paginate calls.
        # Since we create a new paginator iterator for each date in loop.
        
        # We can check the arguments passed to paginate
        calls = self.mock_s3.get_paginator.return_value.paginate.call_args_list
        self.assertEqual(len(calls), 3)
        
        prefixes = [call.kwargs['Prefix'] for call in calls]
        self.assertIn("test-prefix/20240101", prefixes)
        self.assertIn("test-prefix/20240102", prefixes)
        self.assertIn("test-prefix/20240103", prefixes)

    def test_get_merge_logic(self):
        # Mocking S3 list_objects_v2 response
        # We will simulate 2 files. 
        # File 1: retrieved at T1, contains 10:00 (v1), 10:01 (v1)
        # File 2: retrieved at T2 (newer), contains 10:01 (v2), 10:02 (v2)
        
        # Expectation: 10:00 (v1), 10:01 (v2), 10:02 (v2)
        
        retrieval_t1 = "20240105120000"
        retrieval_t2 = "20240105130000" # Newer
        
        # Format: Start_End_Exchange_Retrieval
        key1 = f"test-prefix/20240101100000_20240101100100_J_{retrieval_t1}.jsonl.zstd"
        key2 = f"test-prefix/20240101100100_20240101100200_J_{retrieval_t2}.jsonl.zstd"
        
        self.mock_s3.get_paginator.return_value.paginate.return_value = [
            {
                'Contents': [
                    {'Key': key1},
                    {'Key': key2}
                ]
            }
        ]
        
        # Mocking get_object response for download
        def get_object_side_effect(Bucket, Key):
            data = []
            if Key == key1:
                data = [
                    {"timestamp": "20240101100000", "fields": ["v"], "data": {"A": [1]}},
                    {"timestamp": "20240101100100", "fields": ["v"], "data": {"A": [1]}}
                ]
            elif Key == key2:
                data = [
                    {"timestamp": "20240101100100", "fields": ["v"], "data": {"A": [2]}},
                    {"timestamp": "20240101100200", "fields": ["v"], "data": {"A": [2]}}
                ]
            
            # Compress
            lines = []
            for entry in data:
                lines.append(json.dumps(entry))
            full_text = '\n'.join(lines) + '\n'
            
            cctx = zstd.ZstdCompressor()
            compressed = cctx.compress(full_text.encode('utf-8'))
            
            return {'Body': io.BytesIO(compressed)}
            
        self.mock_s3.get_object.side_effect = get_object_side_effect
        
        # Execute get
        start = datetime(2024, 1, 1, 10, 0)
        end = datetime(2024, 1, 1, 10, 5)
        
        result = self.wrapper.get(start, end)
        
        self.assertEqual(len(result), 3)
        self.assertEqual(result[0]['timestamp'], "20240101100000")
        self.assertEqual(result[0]['data']['A'], [1])
        
        self.assertEqual(result[1]['timestamp'], "20240101100100")
        self.assertEqual(result[1]['data']['A'], [2]) # Reflected v2
        
        self.assertEqual(result[2]['timestamp'], "20240101100200")
        self.assertEqual(result[2]['data']['A'], [2])

class TestS3Integration(unittest.TestCase):
    def setUp(self):
        # User requested prefix "test/"
        # Assuming bucket "rhetenor" based on project context, or typical kline bucket.
        # The user didn't specify bucket, but "hantoo_stk_kline_1m" was previous prefix default.
        # Most likely the bucket name is 'rhetenor' based on data.py
        self.bucket_name = "rhetenor" 
        self.prefix = "test"
        
        # Ensure auth config exists or handle error
        # Assuming run from project root
        try:
             self.wrapper = S3KlineWrapper(self.bucket_name, self.prefix)
        except Exception as e:
             self.skipTest(f"Skipping integration test due to init failure (missing creds?): {e}")

    def test_overwrite_functionality(self):
        today = datetime.now()
        date_str = today.strftime("%Y%m%d")
        
        # Generate random data 1
        # Time: 09:00:00
        ts_1 = f"{date_str}090000"
        ts_2 = f"{date_str}090100"
        
        data1 = [
            {
                "timestamp": ts_1, 
                "fields": ["open"],
                "data": {
                    "005930": ["100"],
                    "000660": ["200"]
                }
            },
            {
                "timestamp": ts_2, 
                "fields": ["open"],
                "data": {
                    "005930": ["105"],
                    "000660": ["205"]
                }
            }
        ]
        
        print(f"\n[Test] Uploading Batch 1... {data1}")
        self.wrapper.put(data1)
        
        # Sleep to ensure next upload has later retrieval time (seconds precision)
        print("[Test] Sleeping 2 seconds to ensure newer timestamp...")
        time.sleep(2)
        
        # Generate random data 2 (Overwriting same timestamps)
        data2 = [
            {
                "timestamp": ts_1, 
                "fields": ["open"],
                "data": {
                    "005930": ["999"], # Changed
                    "000660": ["888"]
                }
            },
            {
                "timestamp": ts_2, 
                "fields": ["open"],
                "data": {
                    "005930": ["999"], # Changed
                    "000660": ["888"]
                }
            }
        ]
        
        print(f"[Test] Uploading Batch 2 (Overwrite)... {data2}")
        self.wrapper.put(data2)
        
        # Sleep a bit to ensure S3 eventual consistency 
        time.sleep(1)
        
        # Retrieve
        start_dt = datetime.strptime(ts_1, "%Y%m%d%H%M%S")
        end_dt = datetime.strptime(ts_2, "%Y%m%d%H%M%S")
        
        print(f"[Test] Retrieving data from {start_dt} to {end_dt}...")
        results = self.wrapper.get(start_dt, end_dt)
        
        print(f"[Test] Retrieved: {results}")
        
        # Verification
        self.assertEqual(len(results), 2)
        
        # Check if values match data2
        for res in results:
            ts = res['timestamp']
            sym_data = res['data']
            if ts == ts_1:
                self.assertEqual(sym_data['005930'], [999])
            elif ts == ts_2:
                self.assertEqual(sym_data['005930'], [999])

if __name__ == '__main__':
    unittest.main()
