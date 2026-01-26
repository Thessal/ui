
import os
import yaml
import boto3
import zstandard as zstd
import json
import io
from datetime import datetime
from typing import Iterator, Dict, Any, Optional


class S3Wrapper:
    """
    AWS S3 Wrapper for storing and retrieving data.
    Base class providing S3 client and common IO methods.
    """

    def __init__(self, bucket: str, prefix: str,
                 auth_config_path: str = "auth/aws_rhetenor.yaml", region: Optional[str] = None):
        """
        Initialize the S3Wrapper.

        Args:
            bucket: The S3 bucket name.
            prefix: The S3 key prefix where logs are stored.
            auth_config_path: Path to the AWS credentials YAML file.
            region: AWs region (overrides config if provided).
        """
        self.bucket = bucket
        self.prefix = prefix
        self.auth_config = self._load_credentials(auth_config_path)

        region_name = region or self.auth_config.get('region')

        self.s3 = boto3.client(
            's3',
            region_name=region_name,
            aws_access_key_id=self.auth_config.get('access_key_id'),
            aws_secret_access_key=self.auth_config.get('secret_access_key')
        )

    def _load_credentials(self, path: str) -> Dict[str, str]:
        """Load AWS credentials from a YAML file."""
        if not os.path.exists(path):
            raise FileNotFoundError(f"AWS config file not found at {path}")

        with open(path, 'r') as f:
            try:
                config = yaml.safe_load(f)
                return config
            except yaml.YAMLError as e:
                raise ValueError(f"Failed to parse AWS config YAML: {e}")

    def list_objects(self, start_date: Optional[datetime] = None, end_date: Optional[datetime] = None) -> Iterator[str]:
        """
        List S3 objects in the bucket with the given prefix, optionally filtered by date.
        Assumes filenames contain timestamps in the format: {prefix}/{timestamp}_...
        Timestamp format: %Y%m%d_%H%M%S
        """
        paginator = self.s3.get_paginator('list_objects_v2')
        page_iterator = paginator.paginate(
            Bucket=self.bucket, Prefix=self.prefix)

        for page in page_iterator:
            if 'Contents' not in page:
                continue

            for obj in page['Contents']:
                key = obj['Key']
                if not key.endswith('.jsonl.zstd') and not key.endswith('.json'):
                    continue

                if start_date or end_date:
                    try:
                        filename = os.path.basename(key)
                        # Try to parse timestamp from filename start
                        # Case 1: YYYYMMDD_HHMMSS (Kline)
                        # Case 2: YYYYMMDD (Master)
                        parts = filename.split('_')
                        if len(parts) >= 1:
                            date_part = parts[0]
                            # Try long format first
                            try:
                                file_dt = datetime.strptime(
                                    date_part, "%Y%m%d")  # minimal matching
                                # If it has time part, it might be separate or attached?
                                # This simple check might be enough for prefixes.

                                if start_date and file_dt < start_date:
                                    continue
                                if end_date and file_dt > end_date:
                                    continue
                            except ValueError:
                                pass
                    except (ValueError, IndexError):
                        pass

                yield key

    def download_and_parse(self, key: str):
        """
        Download a specific object, decompress it (if zstd), and parse JSON/JSONL.
        """
        raise NotImplemented
    
    def _download_jsonl_zstd(self, key: str) -> Iterator[Dict[str, Any]]:
        try:
            response = self.s3.get_object(Bucket=self.bucket, Key=key)
            body = response['Body']

            # Check extension
            if key.endswith('.jsonl.zstd'):
                dctx = zstd.ZstdDecompressor()
                with dctx.stream_reader(body) as reader:
                    text_stream = io.TextIOWrapper(reader, encoding='utf-8')
                    for line in text_stream:
                        if line.strip():
                            try:
                                yield json.loads(line)
                                # yield self._parse_postprocess(json.loads(line))
                            except json.JSONDecodeError as e:
                                print(
                                    f"Error decoding JSON in file {key}: {e}")
            else:
                print(f"Skipping {key}")
                pass

        except Exception as e:
            print(f"Error processing file {key}: {e}")
            raise

    def _download_json(self, key: str) -> Optional[Dict[str, Any]]:
        try:
            response = self.s3.get_object(Bucket=self.bucket, Key=key)
            body = response['Body']

            # Check extension
            if key.endswith('.json'):
                try:
                    return json.loads(body.read())
                except json.JSONDecodeError as e:
                    print(
                        f"Error decoding JSON in file {key}: {e}")
            else:
                print(f"Skipping {key}")
                return None

        except Exception as e:
            print(f"Error processing file {key}: {e}")
            raise

    def put(self):
        raise NotImplementedError

    def get(self):
        raise NotImplementedError


class S3MasterWrapper(S3Wrapper):
    """
    AWS S3 Wrapper for storing and retrieving Hantoo master data.
    """

    def __init__(self, bucket: str = "rhetenor", prefix: str = "hantoo_master",
                 auth_config_path: str = "auth/aws_rhetenor.yaml", region: Optional[str] = None):
        super().__init__(bucket, prefix, auth_config_path, region)

    def _parse_postprocess(self, x):
        return x

    def put(self, data: Dict[str, Any], market: str, date_str: str):
        """
        Upload master data to S3.
        Key Format: {prefix}/{date_str}_{market}.json
        """
        key = f"{self.prefix}/{date_str}_{market}.json"
        print(f"Uploading master data to {key}...")

        json_body = json.dumps(data, ensure_ascii=False)
        self.s3.put_object(Bucket=self.bucket, Key=key,
                           Body=json_body.encode('utf-8'))

    def get(self, market: str, date_str: str) -> Optional[Dict[str, Any]]:
        """
        Get master data from S3.
        """
        key = f"{self.prefix}/{date_str}_{market}.json"
        print(f"Checking S3 for {key}...")
        return self._download_json(key)

class S3KlineWrapper(S3Wrapper):
    """
    AWS S3 Wrapper for storing and retrieving Hantoo kline data.
    """

    def __init__(self, exchange_code: str, bucket: str, prefix: str = "hantoo_stk_kline_1m",
                 auth_config_path: str = "auth/aws_rhetenor.yaml", region: Optional[str] = None):
        # Exchange code : "J", "NX", "UN"
        super().__init__(bucket, prefix, auth_config_path, region)
        self.loaded_data_map = {}
        self.exchange_code = exchange_code

    def load(self, datetime_from: datetime, datetime_to: datetime):
        """
        Load data from S3 into loaded_data_map.
        """
        records = self.get(datetime_from, datetime_to)
        self.loaded_data_map = {}
        for rec in records:
            t = rec.get('timestamp')
            if t:
                self.loaded_data_map[t] = rec
        print(f"S3KlineWrapper loaded {len(self.loaded_data_map)} records.")

    def reconcile(self, new_data_map: Dict[str, dict]) -> list[dict]:
        """
        Compare new_data_map with loaded_data_map, merge if needed, and upload new records (as received; not merged).
        """
        updates = []
        for t_str, new_record in new_data_map.items():
            if t_str in self.loaded_data_map:
                old_record = self.loaded_data_map[t_str]
                new_record = self._parse_postprocess(new_record)

                # Check fields
                if old_record.get('fields') != new_record.get('fields'):
                    # Field diff -> Treat as new (overwrite/update)
                    self.loaded_data_map[t_str] = new_record
                    print(f"[aws.py] warning: field mismatch")
                    updates.append(new_record)
                else:
                    # Fields same -> Merge data
                    if old_record.get('data') == new_record.get('data'):
                        continue # do not append to updates
                    else:
                        # Merge Inconsistent data
                        print(f"[aws.py] updating inconsistent data : {t_str}")
                        debugstr_1 = str(old_record['data'])
                        old_record['data'].update(new_record['data'])
                        debugstr_2 = str(old_record['data'])
                        print(f"old : {debugstr_1[:50]}..{debugstr_1[-50:]}")
                        print(f"new : {debugstr_2[:50]}..{debugstr_2[-50:]}")
                        # Upload
                        updates.append(new_record) # not merged record
            else:
                # New data
                self.loaded_data_map[t_str] = new_record
                updates.append(new_record)
        return updates

    def put(self, data: list[dict], retrieval_time: Optional[datetime] = None):
        """
        Upload kline data to S3, splitting by date.

        Args:
            data: List of dictionaries containing kline data. 
                  Must have a 'timestamp' field.
            exchange_code: 'J', 'NX', 'UN'. Included in filename.
        """
        if not data:
            return

        # Helper to parse timestamp from dict
        def get_ts(d):
            ts_val = d.get('timestamp')
            if not ts_val:
                return datetime.min
            if isinstance(ts_val, datetime):
                return ts_val
            # Common format used in examples
            return datetime.strptime(ts_val, "%Y%m%d%H%M%S")

        # Group data by date
        data_by_date = {}
        for entry in data:
            ts = get_ts(entry)
            if isinstance(ts, datetime) and ts != datetime.min:
                date_key = ts.strftime("%Y%m%d")
            else:
                date_key = "unknown"

            if date_key not in data_by_date:
                data_by_date[date_key] = []
            data_by_date[date_key].append(entry)

        if not retrieval_time:
            retrieval_time = datetime.now()
        retrieval_str = retrieval_time.strftime("%Y%m%d%H%M%S")

        # Format for filename: YYYYMMDDHHMMSS
        def fmt_ts(dt):
            if isinstance(dt, datetime):
                return dt.strftime("%Y%m%d%H%M%S")
            return str(dt).replace("-", "").replace("_", "").replace(":", "")

        for date_key, entries in data_by_date.items():
            if not entries:
                continue

            sorted_data = sorted(entries, key=get_ts)
            start_entry = sorted_data[0]
            end_entry = sorted_data[-1]

            start_ts_obj = get_ts(start_entry)
            end_ts_obj = get_ts(end_entry)

            start_str = fmt_ts(start_ts_obj)
            end_str = fmt_ts(end_ts_obj)

            # Format: {StartTime}_{EndTime}_{exchange_code}_{RetrievalTime}.jsonl.zstd
            filename = f"{start_str}_{end_str}_{self.exchange_code}_{retrieval_str}.jsonl.zstd"
            key = f"{self.prefix}/{filename}"

            # Prepare content
            lines = []
            for entry in sorted_data:
                lines.append(json.dumps(entry, default=str))

            full_text = '\n'.join(lines) + '\n'
            cctx = zstd.ZstdCompressor()
            compressed_data = cctx.compress(full_text.encode('utf-8'))

            # Upload
            print(f"Uploading {key}...")
            self.s3.put_object(Bucket=self.bucket, Key=key,
                               Body=compressed_data)

    def _parse_postprocess(self, x):
        """
        Parse string values in json data
        (converts str to int)
        """
        for symbol, entries in x["data"].items():
            x["data"][symbol] = [int(y) for y in entries]
        return x
    
    def get(self, datetime_from: datetime, datetime_to: datetime) -> list[dict]:
        """
        Get kline data from S3 within the specified range.
        Optimized to strictly query date prefixes.

        Args:
            datetime_from: Start datetime.
            datetime_to: End datetime.

        Returns:
            Merged list of kline dictionaries.
        """
        from datetime import timedelta

        candidates = []
        paginator = self.s3.get_paginator('list_objects_v2')

        # Calculate list of dates to query
        curr_date = datetime_from.date()
        end_date = datetime_to.date()

        target_dates = []
        while curr_date <= end_date:
            target_dates.append(curr_date.strftime("%Y%m%d"))
            curr_date += timedelta(days=1)

        for date_str in target_dates:
            # Construct prefix: {prefix}/{date_str}
            # The 'put' logic creates files like `{prefix}/YYYYMMDDHHMMSS_...`
            # So the prefix `{prefix}/YYYYMMDD` will efficiently filter them.

            query_prefix = f"{self.prefix}/{date_str}"

            page_iterator = paginator.paginate(
                Bucket=self.bucket, Prefix=query_prefix)

            for page in page_iterator:
                if 'Contents' not in page:
                    continue
                for obj in page['Contents']:
                    key = obj['Key']
                    if not key.endswith('.jsonl.zstd'):
                        continue

                    fname = os.path.basename(key)
                    base = fname.replace('.jsonl.zstd', '')
                    parts = base.split('_')
                    # format: Start_End_Exchange_Retrieval (4 parts)

                    f_start_str = parts[0]
                    f_end_str = parts[1]
                    f_exchange = parts[2]
                    f_retrieval_str = parts[3]

                    f_start = datetime.strptime(
                        f_start_str, "%Y%m%d%H%M%S")
                    f_end = datetime.strptime(f_end_str, "%Y%m%d%H%M%S")
                    f_retrieval = datetime.strptime(
                        f_retrieval_str, "%Y%m%d%H%M%S")

                    # Check overlap
                    if f_exchange == self.exchange_code and f_start <= datetime_to and f_end >= datetime_from:
                        candidates.append({
                            'key': key,
                            'retrieval_time': f_retrieval,
                            'start': f_start,
                            'end': f_end
                        })

        # Sort candidates by retrieval time (ascending)
        candidates.sort(key=lambda x: x['retrieval_time'])

        merged_data = {}

        for can in candidates:
            # download_and_parse is in base class
            for record in self._download_jsonl_zstd(can['key']):
                record = self._parse_postprocess(record)
                ts_val = record.get('timestamp')
                if not ts_val:
                    continue

                rec_dt = None
                try:
                    rec_dt = datetime.strptime(ts_val, "%Y%m%d%H%M%S")
                except ValueError:
                    pass

                if rec_dt:
                    if rec_dt < datetime_from or rec_dt > datetime_to:
                        continue

                # Merge Logic
                if ts_val in merged_data:
                    existing = merged_data[ts_val]
                    # Check fields
                    if existing.get('fields') == record.get('fields'):
                        # Fields match, merge data
                        # We assume 'data' is a dict of symbol -> values
                        if 'data' in existing and 'data' in record:
                            existing['data'].update(record['data'])
                    else:
                        # Fields differ, overwrite
                        merged_data[ts_val] = record
                else:
                    merged_data[ts_val] = record

        def sort_key(item):
            t = item.get('timestamp')
            try:
                return datetime.strptime(t, "%Y%m%d%H%M%S")
            except:
                return t

        return sorted(merged_data.values(), key=sort_key)
