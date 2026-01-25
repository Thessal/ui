
import os
import yaml
import zstandard as zstd
import json
import io
from datetime import datetime, timedelta
from typing import Iterator, Dict, Any, Optional, List, Union
import requests
import zipfile
import threading
import time


from .aws import S3MasterWrapper, S3KlineWrapper


def download_master(market: str = "kospi", date_str: Optional[str] = None,
                    verbose: bool = False, upload_to_s3: bool = True,
                    bucket: str = "rhetenor", auth_config_path: str = "auth/aws_rhetenor.yaml") -> Dict[str, Dict[str, Any]]:
    """
    Downloads and parses the Master file (KOSPI, KOSDAQ, KONEX).
    Checks S3 cache first. If missing, downloads from web, parses, and uploads to S3.
    Use specs from `src/rhetenor/hantoo_mst_spec/`.

    Args:
        market: "kospi", "kosdaq", or "konex".
        date_str: Date string YYYYMMDD. Defaults to today.
        verbose: Print status.
        upload_to_s3: Whether to upload parsed data to S3.
        bucket: S3 bucket name.
        auth_config_path: Path to AWS credentials.

    Returns:
        Dictionary mapping short code to information dictionary.
    """
    market = market.lower()

    # Import Spec dynamically or via map
    try:
        if market == 'kospi':
            from .hantoo_mst_spec import kospi as spec
        elif market == 'kosdaq':
            from .hantoo_mst_spec import kosdaq as spec
        elif market == 'konex':
            from .hantoo_mst_spec import konex as spec
        else:
            raise ValueError(
                f"Invalid market: {market}. Must be 'kospi', 'kosdaq', or 'konex'.")
    except ImportError as e:
        raise ImportError(
            f"Spec for {market} not found or error importing: {e}")

    if not date_str:
        date_str = datetime.now().strftime("%Y%m%d")

    # S3 Setup
    # prefix is implicit in S3MasterWrapper (default "hantoo_master"), or we pass it?
    # User requested S3MasterWrapper has default prefix="hantoo_master".
    # And format YYYYMMDD_{market}.json

    loader = None
    try:
        loader = S3MasterWrapper(bucket, "hantoo_master", auth_config_path)
    except Exception as e:
        if verbose:
            print(f"S3MasterWrapper init failed (skipping S3 cache): {e}")

    # 1. Try S3 Download
    if loader:
        data = loader.get(market, date_str)
        if data:
            if verbose:
                print(
                    f"Loaded master data from S3: {loader.prefix}/{date_str}_{market}.json")
            return data

    # 2. Web Download
    url = spec.URL
    mst_filename = spec.FILENAME
    field_specs = spec.FIELD_SPECS
    part2_columns = spec.COLUMNS
    suffix_length = spec.SUFFIX_LENGTH

    try:
        if verbose:
            print(f"Downloading {market} master file from {url}...")

        response = requests.get(url, verify=False)
        response.raise_for_status()

        with zipfile.ZipFile(io.BytesIO(response.content)) as zf:
            if mst_filename not in zf.namelist():
                raise FileNotFoundError(
                    f"{mst_filename} not found in the downloaded zip file.")

            with zf.open(mst_filename) as f:
                content = f.read().decode('cp949')

        master_dict = {}
        for line in content.splitlines():
            # The line length logic assumes fixed bytes for part2.
            # If line is short, skip.
            if len(line) <= suffix_length:
                continue

            part1 = line[:-suffix_length]
            part2 = line[-suffix_length:]

            # Common Logic Part 1:
            # Short Code (9), Standard Code (12), Name (Rest)

            if len(part1) < 21:
                continue

            short_code = part1[0:9].strip()
            standard_code = part1[9:21].strip()
            korean_name = part1[21:].strip()

            entry = {
                "standard_code": standard_code,
                "korean_name": korean_name,
                "market": market
            }

            curr_idx = 0
            for i, width in enumerate(field_specs):
                if i >= len(part2_columns):
                    break

                col_name = part2_columns[i]
                if curr_idx + width <= len(part2):
                    val = part2[curr_idx: curr_idx + width].strip()
                    entry[col_name] = val
                else:
                    entry[col_name] = ""

                curr_idx += width

            master_dict[short_code] = entry

        if verbose:
            print(
                f"Parsed {len(master_dict)} entries from {market} master file.")

        # 3. Upload to S3
        if upload_to_s3 and loader:
            try:
                loader.put(master_dict, market, date_str)
            except Exception as e:
                print(f"Failed to upload to S3: {e}")

        return master_dict

    except Exception as e:
        print(f"Failed to download or parse {market} master file: {e}")
        raise

FIELD_INTERNAL = ["open", "high", "low", "close", "volume", "acc_krw_vol"]
FIELD_MAPPING = ['stck_oprc','stck_hgpr','stck_lwpr','stck_prpr','cntg_vol','acml_tr_pbmn' ]
                # 'prdy_vrss': '전일 대비',
                # 'prdy_vrss_sign': '전일 대비 부호',
                # 'prdy_ctrt': '전일 대비율',
                # 'stck_prdy_clpr': '전일대비 종가',
                # 'acml_vol': '누적 거래량',
                # 'acml_tr_pbmn': '누적 거래대금',
                # 'hts_kor_isnm': '한글 종목명',
                # 'stck_prpr': '주식 현재가',
                # 'stck_bsop_date': '주식 영업일자',
                # 'stck_cntg_hour': '주식 체결시간',
                # 'stck_prpr': '주식 현재가',
                # 'stck_oprc': '주식 시가',
                # 'stck_hgpr': '주식 최고가',
                # 'stck_lwpr': '주식 최저가',
                # 'cntg_vol': '체결 거래량',
                # 'acml_tr_pbmn': '누적 거래대금'
class HantooClient:
    """
    Client for interacting with Korea Investment Securities (Hantoo) API.
    Handles authentication, token management, and specific API calls.
    """
    BASE_URL_PROD = "https://openapi.koreainvestment.com:9443"

    def __init__(self, app_key: str, app_secret: str, account_no: str = "", mock: bool = False, token_path: str = "token.yaml"):
        self.app_key = app_key
        self.app_secret = app_secret
        self.account_no = account_no
        self.mock = mock
        self.base_url = self.BASE_URL_PROD  # Currently defaulting to PROD
        self.token_path = token_path
        self._access_token = None
        self._token_expiry = None

        self._load_token()

    def _load_token(self):
        """Load token from local file if valid, otherwise request new one."""
        if os.path.exists(self.token_path):
            with open(self.token_path, 'r') as f:
                try:
                    data = yaml.safe_load(f)
                    token = data.get('token')
                    # Format: YYYY-mm-dd HH:MM:SS
                    expiry_str = data.get('valid-date')

                    if token and expiry_str:
                        expiry_dt = datetime.strptime(
                            expiry_str, "%Y-%m-%d %H:%M:%S")
                        if expiry_dt > datetime.now():
                            self._access_token = token
                            self._token_expiry = expiry_dt
                            return
                except Exception as e:
                    print(f"Failed to load token from {self.token_path}: {e}")

        # If we reach here, we need a new token
        self._issue_token()

    def _issue_token(self):
        """Issue a new access token and save it."""
        url = f"{self.base_url}/oauth2/tokenP"
        headers = {"content-type": "application/json"}
        body = {
            "grant_type": "client_credentials",
            "appkey": self.app_key,
            "appsecret": self.app_secret
        }

        resp = requests.post(url, headers=headers, json=body)
        resp.raise_for_status()
        data = resp.json()

        self._access_token = data['access_token']
        # Format: 2022-08-30 08:30:00
        expiry_str = data['access_token_token_expired']
        self._token_expiry = datetime.strptime(expiry_str, "%Y-%m-%d %H:%M:%S")

        # Save to file
        os.makedirs(os.path.dirname(self.token_path), exist_ok=True)
        with open(self.token_path, 'w') as f:
            yaml.dump({
                'token': self._access_token,
                'valid-date': expiry_str
            }, f)

    def get_headers(self, tr_id: str, tr_cont: str = "") -> Dict[str, str]:
        """Generate headers for API requests."""
        if not self._access_token or (self._token_expiry and datetime.now() >= self._token_expiry):
            self._issue_token()

        return {
            "Content-Type": "application/json",
            "authorization": f"Bearer {self._access_token}",
            "appkey": self.app_key,
            "appsecret": self.app_secret,
            "tr_id": tr_id,
            "tr_cont": tr_cont,
            "custtype": "P"  # Individual
        }

    def check_holiday(self, date_str: str) -> Dict[str, Any]:
        """
        Check if a date is a holiday.
        API: /uapi/domestic-stock/v1/quotations/chk-holiday
        TR_ID: CTCA0903R
        """
        path = "/uapi/domestic-stock/v1/quotations/chk-holiday"
        url = f"{self.base_url}{path}"
        tr_id = "CTCA0903R"

        headers = self.get_headers(tr_id)
        params = {
            "BASS_DT": date_str,
            "CTX_AREA_FK": "",
            "CTX_AREA_NK": ""
        }

        resp = requests.get(url, headers=headers, params=params)

        if resp.status_code != 200:
            # Try to print error but don't fail immediately, return empty
            print(f"Holiday check failed: {resp.text}")
            return {}

        data = resp.json()
        if data['rt_cd'] != '0':
            print(f"API Error in holiday check: {data.get('msg1')}")
            return {}

        # The output is a list of days.
        # We are usually asking for a specific day or getting a range.
        # The API returns a list in 'output'.
        return data

    def inquire_time_dailychartprice(self, symbol: str, date: str, time_hhmmss: str, period_code: str = "N", include_fake: str = "", market_code: str = "UN", tr_cont: str = "") -> tuple[Dict[str, str], Dict[str, Any]]:
        """
        Get minute (kline) data.
        API: /uapi/domestic-stock/v1/quotations/inquire-time-dailychartprice
        TR_ID: FHKST03010230

        Returns:
            Tuple of (response_headers, response_json)
        """
        path = "/uapi/domestic-stock/v1/quotations/inquire-time-dailychartprice"
        url = f"{self.base_url}{path}"
        tr_id = "FHKST03010230"

        headers = self.get_headers(tr_id, tr_cont=tr_cont)

        # Cond market div code: J (Stock), but required parameter.
        # The example code uses "J".

        all_output2 = []
        last_data = {}
        curr_time = time_hhmmss
        seen_keys = set()

        while True:
            params = {
                "FID_COND_MRKT_DIV_CODE": market_code,  # J: KRX, NX: NXT, UN: Aggregated
                "FID_INPUT_ISCD": symbol,
                "FID_INPUT_HOUR_1": curr_time,
                "FID_INPUT_DATE_1": date,
                "FID_PW_DATA_INCU_YN": period_code,
                "FID_FAKE_TICK_INCU_YN": include_fake
            }

            resp = requests.get(url, headers=headers, params=params)

            if resp.status_code != 200:
                print(f"Kline fetch failed: {resp.text}")
                # If we have partial data, maybe better to return what we have?
                # For now, consistent with previous behavior: error out or return empty if first call?
                # If this is not the first call, we might want to return what we collected.
                if all_output2:
                    last_data['output2'] = all_output2
                    return resp.headers, last_data
                return {}, {}

            data = resp.json()
            if data['rt_cd'] != '0':
                print(f"API Error in kline fetch: {data.get('msg1')}")
                if all_output2:
                    last_data['output2'] = all_output2
                    return resp.headers, last_data
                # Potentially handle rate limiting here

            last_data = data
            output2 = data.get('output2', [])

            if not output2:
                break

            # Filter duplicates and valid check
            added_count = 0
            for row in output2:
                # Key: date + time
                r_date = row.get('stck_bsop_date')
                r_time = row.get('stck_cntg_hour')
                key = f"{r_date}_{r_time}"

                if key not in seen_keys:
                    seen_keys.add(key)
                    all_output2.append(row)
                    added_count += 1

            if added_count == 0:
                # All duplicates, stop
                break

            # Prepare for next iteration
            last_row = output2[-1]
            next_time = last_row.get('stck_cntg_hour')

            # If next_time is same as curr_time, we might be stuck or done.
            # But the API usually returns records *up to* time, or *before*?
            # 'inquire-time-dailychartprice' with time T usually returns candles leading up to T (descending?).
            # Actually, standard behavior for stock APIs (like this one) is often descending order.
            # So if we ask for 12:00:00, we get 12:00, 11:59, etc.
            # So next_time (last in list) will be smaller (earlier).
            # If next_time == curr_time, we made no progress.

            if next_time == curr_time:
                break

            curr_time = next_time

            # Rate limit/Safety break?
            # If output2 is small, we probably reached the end/start of day.
            # Usually batch is 120.
            if len(output2) < 120:
                break

            # Optional: Sleep to be nice to API?
            # The user code already has rate limiting outside this function.
            # But here we are doing multiple requests per ONE user call.
            # We should probably sleep a tiny bit.
            time.sleep(0.05)

        last_data['output2'] = all_output2

        return resp.headers, last_data

    def inquire_time_itemchartprice(self, market_code: str, symbol: str, time_hhmmss: str, include_past: str = "N", etc_code: str = "", tr_cont: str = "") -> tuple[Dict[str, str], Dict[str, Any]]:
        """
        Get today's minute chart price.
        API: /uapi/domestic-stock/v1/quotations/inquire-time-itemchartprice
        TR_ID: FHKST03010200

        Args:
            market_code (str): 'J', 'NX', 'UN' # 'J': KRX, 'NX': NXT, 'UN': Aggregated
            symbol (str): Stock code
            time_hhmmss (str): Time to query
            include_past (str): 'Y' or 'N' # Maybe related to pagenation?
            etc_code (str): 
            tr_cont (str): Continuation # Pagenation related?

        Returns:
            Tuple of headers, data
        """
        path = "/uapi/domestic-stock/v1/quotations/inquire-time-itemchartprice"
        url = f"{self.base_url}{path}"
        tr_id = "FHKST03010200"

        headers = self.get_headers(tr_id, tr_cont=tr_cont)

        all_output2 = []
        last_data = {}
        curr_time = time_hhmmss
        seen_keys = set()

        while True:
            params = {
                "FID_COND_MRKT_DIV_CODE": market_code,
                "FID_INPUT_ISCD": symbol,
                "FID_INPUT_HOUR_1": curr_time,
                "FID_PW_DATA_INCU_YN": include_past,
                "FID_ETC_CLS_CODE": etc_code
            }

            resp = requests.get(url, headers=headers, params=params)

            if resp.status_code != 200:
                print(f"Item chart price fetch failed: {resp.text}")
                if all_output2:
                    last_data['output2'] = all_output2
                    return resp.headers, last_data
                return {}, {}

            data = resp.json()
            if data['rt_cd'] != '0':
                print(
                    f"API Error in item chart price fetch: {data.get('msg1')}")
                if all_output2:
                    last_data['output2'] = all_output2
                    return resp.headers, last_data

            last_data = data
            output2 = data.get('output2', [])

            if not output2:
                break

            added_count = 0
            for row in output2:
                # Key: usually contains date+time or time
                # itemchartprice (FHKST03010200) usually returns intraday data (today).
                # output2 fields: stck_cntg_hour, stck_prpr, etc.
                r_time = row.get('stck_cntg_hour')
                # If date is present, use it too, but documentation says it's for today.
                # Just use time as unique key within a day.
                key = r_time

                if key not in seen_keys:
                    seen_keys.add(key)
                    all_output2.append(row)
                    added_count += 1

            if added_count == 0:
                break

            # Prepare next iteration
            last_row = output2[-1]
            next_time = last_row.get('stck_cntg_hour')

            if next_time == curr_time:
                break

            curr_time = next_time

            if len(output2) < 120:
                break

            time.sleep(0.05)

        last_data['output2'] = all_output2

        return resp.headers, last_data


class HantooKlineLogger:
    def __init__(self, symbols: List[str],
                 exchange_code: str = "J",  # default J
                 hantoo_config_path: str = "./auth/hantoo.yaml",
                 hantoo_token_path: str = "./auth/hantoo_token.yaml",
                 aws_config_path: str = "./auth/aws_rhetenor.yaml",
                 bucket: str = "rhetenor",
                 prefix: str = "hantoo-stock-kline-1m"):
        self.symbols = symbols
        self.exchange_code = exchange_code
        self.bucket = bucket
        self.prefix = prefix
        self.aws_config_path = aws_config_path
        self.updates = []

        # Load Hantoo Credentials
        def load_yaml(path):
            if not os.path.exists(path):
                raise FileNotFoundError(f"Config file not found: {path}")
            with open(path, 'r') as f:
                return yaml.safe_load(f)

        h_conf = load_yaml(hantoo_config_path)

        self.hantoo_client = HantooClient(
            app_key=h_conf.get('my_app'),
            app_secret=h_conf.get('my_sec'),
            account_no=h_conf.get('my_acct_stock'),
            token_path=hantoo_token_path
        )

        # Initialize AWS Kline Wrapper
        self.wrapper = S3KlineWrapper(bucket, prefix, aws_config_path)

        self.universe = {}

        # Calculate target date (Last non-holiday)
        # Check today first
        now = datetime.now()
        target_date = now.date()
        date_str = target_date.strftime("%Y%m%d")

        # If it's holiday or weekend, go back?
        # Use API to check holiday for 'today'. If holiday, find prev.
        # Simple backoff loop
        for _ in range(10):  # Try up to 10 days back
            date_str = target_date.strftime("%Y%m%d")
            resp = self.hantoo_client.check_holiday(date_str)
            output = resp.get('output', [])
            is_open = False
            if output:
                # Find the entry for this date
                for d in output:
                    if d.get('bass_dt') == date_str:
                        if d.get('opnd_yn') == 'Y':
                            is_open = True
                        break

            if is_open:
                break

            # Go back 1 day
            target_date -= timedelta(days=1)

        self.target_date = target_date
        print(f"Target Date: {self.target_date}")

        self.init_data_flow()

    def init_data_flow(self):
        """
        1. Load existing data for target date.
        2. Fetch current status.
        3. Update S3.
        """
        # Load existing data
        print("Loading existing data from S3...")
        start_dt = datetime.combine(self.target_date, datetime.min.time())
        # Use end of day
        from datetime import time as dt_time
        end_dt = datetime.combine(self.target_date, dt_time(23, 59, 59))

        self.wrapper.load(start_dt, end_dt)

        # Fetch current data asynchronously
        import asyncio

        self.fetch_and_update()

    def fetch_and_update(self):
        """
        Fetch snapshot for all symbols and aggregate.
        """
        timestamp_snapshot = datetime.now()
        req_time = timestamp_snapshot.strftime("%H%M%S")
        date_str = self.target_date.strftime("%Y%m%d")

        # We will fetch for all symbols.
        results = {}  # Sym -> Data

        import concurrent.futures

        def fetch(sym, exchange_code):
            # FID_PW_DATA_INCU_YN='N' means we only get the block relevant to time?
            # User sample code in test_kline_today uses 'N'.
            # It returns `output2` list.
            try:
                headers, res = self.hantoo_client.inquire_time_itemchartprice(
                    market_code=exchange_code,
                    symbol=sym,
                    time_hhmmss=req_time,
                    include_past="N"
                )
                return sym, res.get('output2', [])
            except Exception as e:
                print(f"Error fetching {sym}: {e}")
                return sym, []

        print(f"Fetching data for {len(self.symbols)} symbols...")
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
            # Pass self.exchange_code
            future_to_sym = {executor.submit(
                fetch, sym, self.exchange_code): sym for sym in self.symbols}
            for future in concurrent.futures.as_completed(future_to_sym):
                sym, rows = future.result()
                if rows:
                    results[sym] = rows

        # Aggregate
        # We have lists of bars for each symbol.
        # We need to invert this to: Time -> {Sym -> Bar}

        time_aggregated = {}

        for sym, rows in results.items():
            for row in rows:
                # row: start time? stck_cntg_hour
                r_time = row.get('stck_cntg_hour')

                # Construct timestamp YYYY-MM-DD_HH:MM
                # Hantoo time is HHMMSS.
                # Minute resolution
                t_str = f"{self.target_date.strftime('%Y%m%d')}{r_time[:4]}"

                if t_str not in time_aggregated:
                    time_aggregated[t_str] = {
                        "timestamp": t_str,
                        "fields": FIELD_INTERNAL,
                        "data": {}
                    }
                # Fields mapping
                try:
                    vals = [ str(row.get(x)) for x in FIELD_MAPPING ] 
                    time_aggregated[t_str]["data"][sym] = vals
                except:
                    print("[data.py] fetch_and_update : Error downloading minute data")

        # Compare and Update using loaded wrapper state
        updates = self.wrapper.reconcile(
            time_aggregated, exchange_code=self.exchange_code)

        if updates:
            self.updates.extend(updates)
            if len(self.updates) >= 15:
                print(f"Uploading {len(self.updates)} records to S3...")
                self.wrapper.put(
                    self.updates, exchange_code=self.exchange_code)
                self.updates = []
        else:
            print("No updates needed.")
