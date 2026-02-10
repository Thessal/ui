import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

interface BacktestResult {
    // Basic structure based on Lean result
    Statistics: any;
    Runtime: any;
    // We mainly need the equity curve for now, which might be in "Charts" -> "Equity" -> "Series" -> "Equity" -> "Values"
    Charts: {
        [key: string]: {
            Series: {
                [key: string]: {
                    Values: { x: number, y: number }[]
                }
            }
        }
    };
}

interface StatisticsInput {
    equity_curve: number[];
    trading_days_per_year?: number;
    risk_free_rate?: number;
}

interface StatisticsOutput {
    sharpe_ratio: number | null;
    sortino_ratio: number | null;
    max_drawdown: number;
    cagr: number | null;
    annual_standard_deviation: number | null;
    annual_variance: number | null;
}

export class ReportGenerator {

    public async generateReport(resultPath: string): Promise<string> {
        const resultData = await fs.promises.readFile(resultPath, 'utf8');
        const backtestResult = JSON.parse(resultData) as BacktestResult;

        const equityCurve = this.extractEquityCurve(backtestResult);
        if (!equityCurve || equityCurve.length === 0) {
            throw new Error("No equity curve found in backtest result.");
        }

        const stats = await this.calculateStatistics(equityCurve);
        return this.renderHtmlReport(stats, backtestResult);
    }

    private extractEquityCurve(result: BacktestResult): number[] {
        // Look for Equity chart
        const equityChart = result.Charts?.['Equity'] || result.Charts?.['Strategy Equity'];
        if (equityChart && equityChart.Series) {
            const equitySeries = equityChart.Series['Equity'] || equityChart.Series['Strategy Equity'];
            if (equitySeries && equitySeries.Values) {
                return equitySeries.Values.map(v => v.y);
            }
        }
        return [];
    }

    private async calculateStatistics(equityCurve: number[]): Promise<StatisticsOutput> {
        return new Promise((resolve, reject) => {
            const input: StatisticsInput = {
                equity_curve: equityCurve,
                trading_days_per_year: 252,
                risk_free_rate: 0.0
            };

            const process = cp.spawn('rhetenor-statistics');
            let stdout = '';
            let stderr = '';

            process.stdout.on('data', (data) => {
                stdout += data.toString();
            });

            process.stderr.on('data', (data) => {
                stderr += data.toString();
            });

            process.on('close', (code) => {
                if (code !== 0) {
                    reject(new Error(`rhetenor-statistics process exited with code ${code}: ${stderr}`));
                } else {
                    try {
                        const output = JSON.parse(stdout);
                        resolve(output);
                    } catch (e) {
                        // Fallback or better error handling if JSON parse fails
                        reject(new Error(`Failed to parse rhetenor-statistics output: ${e}. Output was: ${stdout}`));
                    }
                }
            });

            process.on('error', (err) => {
                reject(new Error(`Failed to spawn rhetenor-statistics. Make sure it is in your PATH. Error: ${err.message}`));
            });

            process.stdin.write(JSON.stringify(input));
            process.stdin.end();
        });
    }

    private renderHtmlReport(stats: StatisticsOutput, result: BacktestResult): string {
        // Basic HTML template
        return `
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Backtest Report</title>
    <style>
        body { font-family: sans-serif; padding: 20px; }
        .stats-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 20px; }
        .stat-card { border: 1px solid #ccc; padding: 15px; border-radius: 5px; }
        .stat-value { font-size: 1.5em; font-weight: bold; }
        .stat-label { color: #666; }
    </style>
</head>
<body>
    <h1>Backtest Report</h1>
    
    <h2>Performance Statistics</h2>
    <div class="stats-grid">
        <div class="stat-card">
            <div class="stat-label">Sharpe Ratio</div>
            <div class="stat-value">${stats.sharpe_ratio !== null ? stats.sharpe_ratio.toFixed(2) : 'N/A'}</div>
        </div>
        <div class="stat-card">
            <div class="stat-label">Sortino Ratio</div>
            <div class="stat-value">${stats.sortino_ratio !== null ? stats.sortino_ratio.toFixed(2) : 'N/A'}</div>
        </div>
        <div class="stat-card">
            <div class="stat-label">Max Drawdown</div>
            <div class="stat-value">${(stats.max_drawdown * 100).toFixed(2)}%</div>
        </div>
        <div class="stat-card">
            <div class="stat-label">CAGR</div>
            <div class="stat-value">${stats.cagr !== null ? (stats.cagr * 100).toFixed(2) + '%' : 'N/A'}</div>
        </div>
        <div class="stat-card">
            <div class="stat-label">Annual Volatility</div>
            <div class="stat-value">${stats.annual_standard_deviation !== null ? (stats.annual_standard_deviation * 100).toFixed(2) + '%' : 'N/A'}</div>
        </div>
    </div>
</body>
</html>
        `;
    }
}
