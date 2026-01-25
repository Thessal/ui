
import unittest
from unittest.mock import MagicMock, patch, mock_open
import sys
import os
import json
from datetime import datetime, timedelta
import io
import zipfile

# Adjust path to import the module
sys.path.append(os.path.join(os.getcwd(), 'src'))
# sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '../src')))

from rhetenor.data import HantooKlineLogger, HantooClient, download_master

class TestHantooKlineLogger(unittest.TestCase):

    @patch('rhetenor.data.download_master')
    @patch('rhetenor.data.HantooClient')
    @patch('rhetenor.data.DataLoader')
    @patch('builtins.open', new_callable=mock_open, read_data="app_key: test\napp_secret: test")
    @patch('os.path.exists', return_value=True)
    def test_init_flow(self, mock_exists, mock_file, mock_dataloader_cls, mock_hantoo_cls, mock_download_master):
        # Setup Mocks
        mock_master_data = {'005930': 'Samsung', '000660': 'SK Hynix'}
        mock_download_master.return_value = mock_master_data
        
        mock_hantoo_instance = mock_hantoo_cls.return_value
        # Mock holiday check
        mock_hantoo_instance.check_holiday.return_value = {"rt_cd": "0", "output": []}
        
        # Mock S3 list objects
        mock_s3_loader = mock_dataloader_cls.return_value
        # Return a fake key
        mock_s3_loader.list_objects.return_value = iter(['hantoo_stk_kline_1m/20240101_100000.jsonl.zstd'])
        
        # Mock Kline Data
        # symbol, date, time -> return valid data
        mock_hantoo_instance.inquire_time_dailychartprice.return_value = {
            "rt_cd": "0",
            "output2": [
                {
                    "stck_bsop_date": "20240101",
                    "stck_cntg_hour": "100100", # 10:01:00
                    "stck_oprc": "100", "stck_hgpr": "110", "stck_lwpr": "90", "stck_prpr": "105", "cntg_vol": "1000"
                }
            ]
        }
        
        # Init Loader
        loader = HantooKlineLogger(symbols=['005930'], 
                                   hantoo_config_path="auth/hantoo.yaml", 
                                   aws_config_path="auth/aws.yaml")
        
        # Verify Master Download called
        mock_download_master.assert_called_once()
        
        # Verify S3 list objects called
        mock_s3_loader.list_objects.assert_called()
        
        # Verify Last Timestamp set (should be updated after fill)
        expected_ts = datetime(2024, 1, 1, 10, 1, 0)
        self.assertEqual(loader.last_timestamp, expected_ts)
        
        # Verify Inquire called (Gap fill)
        # Verify S3 upload called
        # We need to check if put_object was called on s3_client (which is mock_s3_loader.s3)
        mock_s3 = mock_s3_loader.s3
        mock_s3.put_object.assert_called()
        
        # Check upload key
        call_args = mock_s3.put_object.call_args
        self.assertIn('20240101_100100.jsonl.zstd', call_args[1]['Key'])

    @patch('requests.get')
    def test_download_master(self, mock_get):
        # Create a valid zip file in memory
        mf = io.BytesIO()
        with zipfile.ZipFile(mf, 'w', zipfile.ZIP_DEFLATED) as zf:
            # Create a fake mst file content
            # A005930     ... Samsung ...
            # Needs to be somewhat valid for the parser
            # Parser expects len > 228
            # And expects 9th char to be short code end?
            # actually row[0:9] short code.
            
            # Pad with spaces to reach length
            line = "A005930".ljust(9) + "StandardCode".ljust(12) + "SamsungElec".ljust(50) + " " * 200
            content = line + "\n"
            zf.writestr('kospi_code.mst', content.encode('cp949'))
        
        mf.seek(0)
        
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.content = mf.read()
        mock_get.return_value = mock_resp
        
        # Call function
        result = download_master(market="kospi", verbose=False)
        
        # Verify
        self.assertIn('A005930', result)
        self.assertEqual(result['A005930'], 'SamsungElec')

if __name__ == '__main__':
    unittest.main()
