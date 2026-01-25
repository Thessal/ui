
from rhetenor.aws import S3KlineWrapper
from rhetenor.data import download_master, HantooClient, FIELD_INTERNAL, FIELD_MAPPING
import sys
import os
import json
import yaml
import time
import requests
from datetime import datetime, timedelta, date
from typing import List, Dict, Any

# Add src to path to import rhetenor
sys.path.insert(0, os.path.join(os.getcwd(), 'src'))

# Configuration
HANTOO_CONFIG = "auth/hantoo.yaml"
HANTOO_TOKEN = "auth/hantoo_token.yaml"
AWS_CONFIG = "auth/aws_rhetenor.yaml"


def load_yaml(path):
    if not os.path.exists(path):
        raise FileNotFoundError(f"Config file not found: {path}")
    with open(path, 'r') as f:
        return yaml.safe_load(f)


def get_kospi50_symbols(master_data: Dict[str, Any]) -> List[str]:
    """
    Get symbols for KOSPI 50.
    """
    kospi_syms = []
    for k, v in master_data.items():
        if v.get('market') == 'kospi':
            # Check kospi50 flag.
            # User instruction: "Use symbols with kospi50 = Y or True"
            # We check typical Hantoo flag values (1, Y).
            # If the parser returns raw string, it might be '1' or 'Y'.
            val = v.get('kospi50', '').strip()
            if val in ('Y', '1', 'True', True):
                kospi_syms.append(k)

    kospi_syms.sort()
    return kospi_syms


def get_client():
    try:
        h_conf = load_yaml(HANTOO_CONFIG)
        client = HantooClient(
            app_key=h_conf.get('my_app'),
            app_secret=h_conf.get('my_sec'),
            account_no=h_conf.get('my_acct_stock'),
            token_path=HANTOO_TOKEN
        )
        return client
    except Exception as e:
        print(f"Failed to initialize client: {e}")
        return None


def main():
    # 0. Params
    market = "kospi"  # defaults to kospi

    # 1. Get Master
    print(f"Downloading master for {market}...")
    master = download_master(market=market, verbose=True)

    if market.lower() != "kospi":
        raise NotImplementedError(
            "Only KOSPI is supported for KOSPI50 filtering.")

    # 2. Filter KOSPI50
    symbols = get_kospi50_symbols(master)
    print(f"Target Symbols (KOSPI50 proxy): {len(symbols)} symbols")
    print(f"Sample: {symbols[:5]}")

    client = get_client()
    if not client:
        return

    s3_wrapper = S3KlineWrapper("rhetenor", "hantoo_stk_kline_1m", AWS_CONFIG)

    # 3. Find Last 5 Non-Holiday Days
    print("Identifying last 5 non-holiday days...")
    target_dates = []

    check_date = date.today() - timedelta(days=1)  # Start from yesterday
    while len(target_dates) < 5:
        d_str = check_date.strftime("%Y%m%d")

        # Check holiday
        is_holiday = False
        try:
            res = client.check_holiday(d_str)
            output = res.get('output', [])
            if output:
                for item in output:
                    if item.get('bass_dt') == d_str:
                        if item.get('opnd_yn') == 'N':
                            is_holiday = True
                        break
        except Exception as e:
            print(f"Holiday check failed for {d_str}: {e}")
            is_holiday = True  # Fail safe

        if not is_holiday:
            target_dates.append(check_date)

        check_date -= timedelta(days=1)

    # Sort chronological
    target_dates.sort()
    print(f"Target Dates: {[d.strftime('%Y%m%d') for d in target_dates]}")

    # 4. Backfill Loop
    import concurrent.futures

    for d in target_dates:
        d_str = d.strftime("%Y%m%d")
        print(f"\nProcessing {d_str}...")

        daily_kline_buffer = {}  # Timestamp -> Record

        # We need to fetch minute data for each symbol for this day.
        # inquire_time_dailychartprice returns data for specific day up to time.
        # If we use time "235959", we get the whole day (descending).

        # Rate Limiting & Concurrency
        # Limit: 2 req/s -> 0.5s interval per thread?
        # If we use 5 threads, we need to ensure global rate or per-thread rate?
        # User said "Add rate limit(2 request/s)".
        # 2 req/s TOTAL is strict.
        # If we use threads, we might exceed this easily.
        # It's better to use sequential processing or a TokenBucket if threaded.
        # Given the error report (SSLEOFError), sequential or single-threaded with strict sleep is safer.
        # Let's switch to sequential or reduce threads to 1 and sleep 0.5s.
        # Or keep threads but use a semaphore/delay.
        # Simplest & most robust for "2 req/s": Sequential processing in main thread with time.sleep(0.5).
        # "SSLEOFError" often happens when hitting server too fast in parallel.

        MAX_RETRIES = 5

        def safe_fetch(sym):
            for i in range(MAX_RETRIES):
                try:
                    # Request
                    headers, res = client.inquire_time_dailychartprice(
                        sym, d_str, "235959", period_code="N", market_code="J"
                    )
                    return res.get('output2', [])
                except Exception as e:
                    if i == MAX_RETRIES - 1:
                        print(
                            f"Failed to fetch {sym} after {MAX_RETRIES} attempts: {e}")
                        raise e  # Raise final exception

                    # Backoff
                    wait_time = (2 ** i) * 0.5
                    # print(f"Retry {i+1}/{MAX_RETRIES} for {sym} after {wait_time}s error: {e}")
                    time.sleep(wait_time)
            return []

        print(f"Fetching {len(symbols)} symbols...")
        # Sequential execution to strictly control rate

        total_syms = len(symbols)
        for idx, sym in enumerate(symbols):
            start_t = time.time()

            # Progress
            print(
                f"{d_str} [{idx+1}/{total_syms}] Fetching {sym}...", end='\r')

            try:
                rows = safe_fetch(sym)
                if rows:
                    # Aggregate
                    for r in rows:
                        if r.get('stck_bsop_date') != d_str:
                            continue

                        r_time = r.get('stck_cntg_hour')
                        ts_str = f"{d.strftime('%Y%m%d')}{r_time}"

                        if ts_str not in daily_kline_buffer:
                            daily_kline_buffer[ts_str] = {
                                "timestamp": ts_str,
                                "fields": FIELD_INTERNAL,
                                "data": {}
                            }

                        vals = [str(r.get(x)) for x in FIELD_MAPPING]
                        daily_kline_buffer[ts_str]["data"][sym] = vals
            except Exception as e:
                # If safe_fetch raised exception (MAX_RETRIES exceeded)
                # Ensure we handle it (maybe skip symbol or stop?)
                # User says "raise Exception". So we let it bubble up and stop implementation?
                # "error handling (retry max 5 times, and then raise Exeption)"
                # This implies the script should crash? Or just print?
                # Usually we want the script to stop if network is dead.
                print(f"\nCritical Error processing {sym}: {e}")
                raise e

            # Rate Limit: 2 req/s -> 0.5s per request
            elapsed = time.time() - start_t
            if elapsed < 0.5:
                time.sleep(0.5 - elapsed)

        print(
            f"\nAggregated {len(daily_kline_buffer)} timestamps for {d_str}.")

        # Upload check
        print("Reconciling with S3...")
        start_dt = datetime.combine(d, datetime.min.time())
        end_dt = datetime.combine(d, datetime.max.time())

        # Load existing state into wrapper
        s3_wrapper.load(start_dt, end_dt)

        # Reconcile and Upload
        updates = s3_wrapper.reconcile(daily_kline_buffer, exchange_code="UN")

        if updates:
            print(f"Uploading {len(updates)} records...")
            s3_wrapper.put(updates, exchange_code="UN")
        else:
            print("No updates needed (all duplicates).")


if __name__ == "__main__":
    main()
