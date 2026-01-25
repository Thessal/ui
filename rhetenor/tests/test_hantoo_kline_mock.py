
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
    @patch('rhetenor.data.S3KlineWrapper')
    @patch('builtins.open', new_callable=mock_open, read_data="app_key: test\napp_secret: test")
    @patch('os.path.exists', return_value=True)
    @patch('time.sleep')
    def test_init_flow(self, mock_sleep, mock_exists, mock_file, mock_wrapper_cls, mock_hantoo_cls, mock_download_master):
        # Setup Mocks
        # download_master returns dict of dicts
        mock_master_data = {'005930': {'korean_name': 'Samsung', 'market': 'kospi'}, '000660': {'korean_name': 'SK Hynix', 'market': 'kospi'}}
        mock_download_master.return_value = mock_master_data
        
        mock_hantoo_instance = mock_hantoo_cls.return_value
        # Mock holiday check
        mock_hantoo_instance.check_holiday.return_value = {"rt_cd": "0", "output": []}
        
        # Mock S3 Wrapper
        mock_wrapper = mock_wrapper_cls.return_value
        # Mock load (no return needed, just side effect or nothing)
        mock_wrapper.load.return_value = None
        
        # Mock Kline Data
        # symbol, date, time -> return valid data
        mock_hantoo_instance.inquire_time_itemchartprice.return_value = (
             {}, 
             {
                "rt_cd": "0",
                "output2": [
                    {
                        "stck_bsop_date": "20240101",
                        "stck_cntg_hour": "100100", # 10:01:00
                        "stck_oprc": "100", "stck_hgpr": "110", "stck_lwpr": "90", "stck_prpr": "105", "cntg_vol": "1000"
                    }
                ]
            }
        )
        
        # Init Loader
        loader = HantooKlineLogger(symbols=['005930'], 
                                   hantoo_config_path="auth/hantoo.yaml", 
                                   aws_config_path="auth/aws.yaml")
        
        
        # Verify Wrapper Load called
        mock_wrapper.load.assert_called()
        
        # Verify Inquire called
        mock_hantoo_instance.inquire_time_itemchartprice.assert_called()
        
        # Verify Reconcile called
        mock_wrapper.reconcile.assert_called()

    @patch('requests.get')
    def test_download_master(self, mock_get):
        # Create a valid zip file in memory
        mf = io.BytesIO()
        with zipfile.ZipFile(mf, 'w', zipfile.ZIP_DEFLATED) as zf:
            # Create a fake mst file content
            # A005930     ... Samsung ...
            # Needs to be somewhat valid for the parser
            # Parser expects len > 228 (suffix) + 21 (part1)
            # part1: short_code(9) + standard_code(12) + name...
            # suffix: 228
            
            part1 = "A005930".ljust(9) + "StandardCode".ljust(12) + "SamsungElec".ljust(50)
            suffix = " " * 228
            content = part1 + suffix + "\n"
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
        entry = result['A005930']
        self.assertEqual(entry['korean_name'], 'SamsungElec')
        self.assertEqual(entry['standard_code'], 'StandardCode')

if __name__ == '__main__':
    unittest.main()
