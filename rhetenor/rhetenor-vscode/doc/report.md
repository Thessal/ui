# Lean Report Functionality Summary

The `Lean/Report` module is responsible for generating comprehensive HTML and PDF reports based on backtest and live trading results.

## Key Components

### 1. `Report.cs` (Main Orchestrator)
- **Role**: Entry point for report generation.
- **Workflow**:
    - Initializes with strategy metadata (Name, Description, Version).
    - Takes `BacktestResult` and `LiveResult` objects.
    - Loads an HTML template (`template.html`).
    - Instantiates a list of `IReportElement` objects, passing relevant data to each.
    - Iterates through elements, calling `Render()` on each.
    - Injects rendered HTML/JSON into the template.
    - Outputs the final HTML string and a `report-statistics.json` file.

### 2. Report Elements (`Lean/Report/ReportElements/`)
Each element calculates a specific set of metrics or generates a chart.

#### Performance Metrics (KPIs)
- **Sharpe Ratio**: (`SharpeRatioReportElement.cs`) Uses annualized returns and standard deviation.
- **CAGR**: (`CAGRReportElement.cs`) Compound Annual Growth Rate.
- **Drawdown**: (`MaxDrawdownReportElement.cs`, `MaxDrawdownRecoveryReportElement.cs`) Maximum peak-to-valley decline and recovery time.
- **Turnover**: (`TurnoverReportElement.cs`) Portfolio turnover rate.
- **Sortino Ratio**: (`SortinoRatioReportElement.cs`) Like Sharpe, but only penalizes downside volatility.
- **Information Ratio**: (`InformationRatioReportElement.cs`) Excess return relative to a benchmark per unit of tracking error.
- **PSR**: (`PSRReportElement.cs`) Probabilistic Sharpe Ratio.
- **Capacity**: (`EstimatedCapacityReportElement.cs`) Estimated maximum AUM before significant slippage.
- **Trades Per Day**: (`TradesPerDayReportElement.cs`) Average daily trading frequency.

#### Charts
- **Cumulative Returns**: (`CumulativeReturnsReportElement.cs`) Equity curve vs Benchmark.
- **Daily/Monthly/Annual Returns**: Visualizations of periodic returns.
- **Drawdown Plot**: Visualization of drawdown periods over time.
- **Rolling Metrics**: Rolling Beta, Rolling Sharpe over 6/12 month windows.
- **Asset Allocation**: (`AssetAllocationReportElement.cs`) Pie chart of time-weighted holdings.
- **Leverage & Exposure**: Gross leverage usage over time.

#### Crisis Analysis (`CrisisReportElement.cs`)
- Simulates strategy performance during historical market stress events (e.g., 2008 Financial Crisis, 2020 COVID Crash) by overlaying the strategy's behavior on those timelines (or by specialized stress testing logic if available).

### 3. Chart Generation (`ReportCharts.py`)
- Python script using `matplotlib` to generate static images (base64 encoded) for the report.
- Charts include:
    - Cumulative Returns
    - Returns per Trade (Histogram)
    - Drawdown underwater plot
    - Rolling Beta/Sharpe
    - Monthly Returns Heatmap

## Data Flow
`Algorithm Results (JSON)` -> `Report.cs` -> `ReportElements` -> `Rendered HTML` -> `Final Report`
