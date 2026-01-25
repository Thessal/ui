# Data Management

## Master Files

The `download_master` function in `rhetenor.data` handles downloading and parsing market master files (KOSPI, KOSDAQ, KONEX).

### Caching Strategy

To optimize initialization time and reduce bandwidth, `download_master` employs an S3-based caching strategy:

1.  **Check S3**: Checks the `rhetenor` bucket for a cached JSON file at `hantoo_master/{Date}_{Market}.json` (e.g., `hantoo_master/20260125_kospi.json`). (Note: Prefix changed to `hantoo_master`).
2.  **Download from S3**: If found, it downloads and parses the JSON directly.
3.  **Fallback to Web**: If not found in S3 (cache miss):
    - Downloads the ZIP file from the official DWS source.
    - Parses the `.mst` file content using specs from `rhetenor.hantoo_mst_spec`.
    - **Uploads to S3**: Stores the parsed data as a JSON file in S3 for future use.

### Usage

```python
from rhetenor.data import download_master

# Download KOSPI master data (uses cache if available)
kospi_data = download_master("kospi", verbose=True)

# Download KOSDAQ
kosdaq_data = download_master("kosdaq", verbose=True)
```

## Kline Data Logger

`HantooKlineLogger` manages the real-time or catch-up logging of 1-minute candle data from Korea Investment Securities (Hantoo).

### Features
- **S3 Storage**: Stores aggregated minute bars in S3 with ZSTD compression (`hantoo-stock-kline-1m/{StartTime}_{EndTime}_{exchange_code}_{RetrievalTime}.jsonl.zstd`).
- **Format**: 
  ```json
  {
      "timestamp": "YYYY-MM-DD_HH:MM",
      "fields": ["open", "high", "low", "close", "volume", "acc_vol"],
      "data": {
          "005930": [75000, 75100, 74900, 75000, 1000, 5000000],
          ...
      }
  }
  ```
- **Holidays**: Automatically checks for business days and targets the last valid trading day if today is a holiday.
- **Deduplication**: Checks existing S3 data before uploading to prevent duplicate records.
- **Async Fetch**: Uses threading to efficiently fetch snapshots for many symbols.

### Usage

```python
from rhetenor.data import HantooKlineLogger

logger = HantooKlineLogger(symbols=["005930", "000660"])
# Init flow automatically checks S3, fetches current status, and updates if needed.
```
