
import argparse
import sys
import os
from .backtest_interface import run_backtest

def main():
    parser = argparse.ArgumentParser(description="Rhetenor CLI")
    subparsers = parser.add_subparsers(dest="command", help="Command to run")

    run_parser = subparsers.add_parser("run", help="Run a backtest")
    run_parser.add_argument("strategy_file", help="Path to the strategy file (.bf)")
    # Optional arguments for data, etc.
    run_parser.add_argument("--data", help="Path to data file or directory", default=None)
    run_parser.add_argument("--output", help="Path to save results (image)", default=None)
    run_parser.add_argument("--json", help="Path to save results (JSON)", default=None)

    args = parser.parse_args()

    if args.command == "run":
        try:
            run_backtest(args.strategy_file, args.data, args.output, args.json)
        except Exception as e:
            print(f"Error running backtest: {e}")
            import traceback
            traceback.print_exc()
            sys.exit(1)
    else:
        parser.print_help()

if __name__ == "__main__":
    main()
