# AWS S3 Wrapper

The `S3KlineWrapper` facilitates storing and retrieving kline (OHLCV) data using AWS S3. It allows for file-based versioning and merging of data segments.

## Usage

```python
from rhetenor.aws import S3KlineWrapper
from datetime import datetime

# Initialize
wrapper = S3KlineWrapper(bucket="my-bucket", prefix="hantoo_stk_kline_1m")

# Put Data
data = [
    {"timestamp": "2024-01-01_09:00", "open": 100, "close": 110},
    {"timestamp": "2024-01-01_09:01", "open": 110, "close": 115}
]
wrapper.put(data)

# Get Data
start = datetime(2024, 1, 1, 9, 0)
end = datetime(2024, 1, 1, 10, 0)
merged_data = wrapper.get(start, end)
```

## Storage Format

Files are stored in S3 as Zstd-compressed JSONL files. The `put` method automatically splits data into separate files for each date.

**Key Format:**
`{prefix}/{start_timestamp}_{end_timestamp}_{retrieval_timestamp}.jsonl.zstd`

*   `start_timestamp`: Timestamp of the first entry in the file (Format: `YYYYMMDDHHMMSS`).
*   `end_timestamp`: Timestamp of the last entry in the file (Format: `YYYYMMDDHHMMSS`).
*   `retrieval_timestamp`: Timestamp when the data was retrieved/uploaded (Format: `YYYYMMDDHHMMSS`).

By ensuring `start_timestamp` always begins with the date (since files are split by date), queries can efficiently filter files using S3 prefixes: `{prefix}/{YYYYMMDD}`.

## Merge Logic

When retrieving data using `get(start_dt, end_dt)`:

1.  **Prefix Filtering**: The wrapper queries S3 using date prefixes (`YYYYMMDD`) for every day in the requested range.
2.  **Selection**: It selects files that overlap with the requested datetime range.
3.  **Merging**: Files are processed in order of their `retrieval_timestamp`.
4.  **Overwrite**: Data from files with newer `retrieval_timestamp` overwrites data from older files.
