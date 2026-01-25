
import unittest
from unittest.mock import MagicMock, patch
import os
import sys

# Ensure src is in path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../src")))

from rhetenor.data import download_master

class TestHantooMaster(unittest.TestCase):
    @patch('rhetenor.data.S3MasterWrapper')
    @patch('rhetenor.data.requests.get')
    def test_download_kospi_s3_hit(self, mock_requests, mock_wrapper_cls):
        """Test KOSPI master download when S3 cache exists."""
        # Mock S3 wrapper
        mock_wrapper = MagicMock()
        mock_wrapper_cls.return_value = mock_wrapper
        
        # Mock S3 Hit (return parsed dict)
        expected_data = {"005930": {"standard_code": "KR7005930003", "korean_name": "Samsung"}}
        mock_wrapper.get.return_value = expected_data
        
        data = download_master("kospi", verbose=True)
        
        self.assertEqual(data["005930"]["korean_name"], "Samsung")
        # Ensure requests.get (web download) was NOT called
        mock_requests.assert_not_called()

    @patch('rhetenor.data.S3MasterWrapper')
    @patch('rhetenor.data.requests.get')
    @patch('zipfile.ZipFile')
    def test_download_kospi_s3_miss(self, mock_zipfile, mock_requests, mock_wrapper_cls):
        """Test KOSPI master download when S3 cache misses (Parsing test)."""
        # Mock S3 miss
        mock_wrapper = MagicMock()
        mock_wrapper_cls.return_value = mock_wrapper
        mock_wrapper.get.return_value = None
        
        # Mock Web Download
        mock_web_resp = MagicMock()
        mock_web_resp.content = b'dummy_zip_content'
        mock_requests.return_value = mock_web_resp
        
        # Mock Zip
        mock_zf = MagicMock()
        mock_zipfile.return_value.__enter__.return_value = mock_zf
        mock_zf.namelist.return_value = ['kospi_code.mst']
        
        # Mock MST content
        # Line > 228 (suffix) + 21 (prefix) = 249
        # Construct a valid line
        # Prefix: Short(9) + Std(12) + Name(variable)
        part1 = "005930   KR7005930003Samsung Electronics" # 9+12+19 = 40 chars
        part1 = part1.ljust(50) 
        
        # Suffix: 228 bytes for KOSPI
        part2 = "A" * 228
        
        line = part1 + part2
        
        mock_f = MagicMock()
        mock_f.read.return_value.decode.return_value = line
        mock_zf.open.return_value.__enter__.return_value = mock_f
        
        data = download_master("kospi", verbose=True)
        
        # Verify result
        self.assertIn("005930", data)
        self.assertEqual(data["005930"]["standard_code"], "KR7005930003")
        self.assertEqual(data["005930"]["market"], "kospi")
        
        # Verify S3 upload called
        self.assertTrue(mock_wrapper.put.called)
        call_args = mock_wrapper.put.call_args
        self.assertEqual(call_args[0][1], "kospi")

if __name__ == '__main__':
    unittest.main()
