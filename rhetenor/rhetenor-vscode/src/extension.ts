
import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';

import { ReportGenerator } from './report/reportGenerator';

export function activate(context: vscode.ExtensionContext) {
    console.log('Rhetenor extension is now active!');

    let reportDisposable = vscode.commands.registerCommand('rhetenor.generateReport', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showErrorMessage('No active editor found. Open a backtest result JSON file first to generate a report for it, or run a backtest.');
            return;
        }

        const document = editor.document;
        const filePath = document.uri.fsPath;

        if (!filePath.endsWith('.json')) {
            vscode.window.showErrorMessage('Active file is not a JSON file. Please open a backtest result JSON.');
            return;
        }

        try {
            vscode.window.showInformationMessage('Generating Report...');
            const generator = new ReportGenerator();
            const html = await generator.generateReport(filePath);

            const panel = vscode.window.createWebviewPanel(
                'rhetenorReport',
                'Backtest Report',
                vscode.ViewColumn.Two,
                { enableScripts: true }
            );
            panel.webview.html = html;
        } catch (err: any) {
            vscode.window.showErrorMessage(`Failed to generate report: ${err.message}`);
        }
    });

    context.subscriptions.push(reportDisposable);

    let disposable = vscode.commands.registerCommand('rhetenor.runBacktest', () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showErrorMessage('No active editor found.');
            return;
        }

        const document = editor.document;
        if (document.languageId !== 'butterflow') {
            vscode.window.showErrorMessage('Active file is not a Butterflow file.');
            return;
        }

        const filePath = document.uri.fsPath;
        const config = vscode.workspace.getConfiguration('rhetenor');
        const pythonPath = config.get<string>('pythonPath', 'python');
        const modulePath = config.get<string>('modulePath', 'rhetenor');

        // Create output channel
        const outputChannel = vscode.window.createOutputChannel('Rhetenor Backtest');
        outputChannel.show();
        outputChannel.appendLine(`Running backtest for: ${filePath}`);

        // Use the workspace folder as CWD if available
        const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
        const cwd = workspaceFolder?.uri.fsPath || path.dirname(filePath);

        // Output JSON path
        const outputJson = path.join(cwd, 'backtest_result.json');

        // Construct command
        // python -m rhetenor.cli run <filePath> --json <outputJson>
        const args = ['-m', modulePath + '.cli', 'run', filePath, '--json', outputJson];

        outputChannel.appendLine(`Command: ${pythonPath} ${args.join(' ')}`);
        outputChannel.appendLine(`CWD: ${cwd}`);

        const child = cp.spawn(pythonPath, args, { cwd: cwd });

        child.stdout.on('data', (data) => {
            outputChannel.append(data.toString());
        });

        child.stderr.on('data', (data) => {
            outputChannel.append(data.toString());
        });

        child.on('close', (code) => {
            outputChannel.appendLine(`Process exited with code ${code}`);
            if (code === 0) {
                vscode.window.showInformationMessage('Rhetenor: Backtest completed successfully.');

                // Show Result in Webview
                if (fs.existsSync(outputJson)) {
                    const dataContent = fs.readFileSync(outputJson, 'utf8');
                    const data = JSON.parse(dataContent);

                    const panel = vscode.window.createWebviewPanel(
                        'rhetenorResult',
                        'Backtest Result',
                        vscode.ViewColumn.Two,
                        {
                            enableScripts: true,
                            localResourceRoots: [
                                vscode.Uri.file(path.join(context.extensionPath, 'media'))
                            ]
                        }
                    );

                    // Prepare data for Charts
                    // Signal is 1D array. Map to { time, value }
                    // JSON now contains: signal, timestamps, pnl, position, turnover

                    const timestamps = data.timestamps || [];

                    const mapData = (arr: any[]) => arr.map((val: any, idx: number) => ({
                        time: timestamps[idx] || idx,
                        value: val
                    }));

                    const signalSeries = data.signal ? mapData(data.signal) : [];
                    const pnlSeries = data.pnl_cumulative ? mapData(data.pnl_cumulative) : [];
                    const positionSeries = data.position ? mapData(data.position) : [];
                    const turnoverSeries = data.turnover ? mapData(data.turnover) : [];

                    const chartData = {
                        signal: signalSeries,
                        pnl: pnlSeries,
                        position: positionSeries,
                        turnover: turnoverSeries
                    };

                    panel.webview.html = getWebviewContent(panel.webview, context.extensionPath, chartData);
                } else {
                    outputChannel.appendLine(`Output JSON not found at ${outputJson}`);
                }

            } else {
                vscode.window.showErrorMessage('Rhetenor: Backtest failed. Check output.');
            }
        });
    });

    context.subscriptions.push(disposable);
}

function getWebviewContent(webview: vscode.Webview, extensionPath: string, data: any) {
    // Get path to lightweight-charts
    const lwChartPathOnDisk = vscode.Uri.file(
        path.join(extensionPath, 'media', 'lightweight-charts.standalone.production.js')
    );
    const lwChartUri = webview.asWebviewUri(lwChartPathOnDisk);

    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src ${webview.cspSource} 'unsafe-inline';">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Backtest Result</title>
    <script src="${lwChartUri}"></script>
    <style>
        body { margin: 0; padding: 0; background-color: #1e1e1e; color: #ccc; display: grid; grid-template-rows: 1fr 1fr 1fr 1fr; height: 100vh; }
        .chart-container { width: 100%; height: 100%; border-bottom: 1px solid #2B2B43; position: relative; }
        .chart-label { position: absolute; top: 10px; left: 10px; z-index: 100; color: #d1d4f9; font-weight: bold; background: rgba(0,0,0,0.5); padding: 2px 5px; }
        #debug-log { grid-row: 5; background: #000; color: #0f0; overflow-y: scroll; padding: 10px; font-family: monospace; font-size: 12px; border-top: 1px solid #333; }
    </style>
</head>
<body>
    <div id="chart-signal" class="chart-container"><div class="chart-label">Signal</div></div>
    <div id="chart-pnl" class="chart-container"><div class="chart-label">Cumulative PnL</div></div>
    <div id="chart-pos" class="chart-container"><div class="chart-label">Position</div></div>
    <div id="chart-turnover" class="chart-container"><div class="chart-label">Turnover</div></div>
    <div id="debug-log"><h3>Debug Log</h3></div>
    <script>
        document.addEventListener('DOMContentLoaded', () => {
            const logDiv = document.getElementById('debug-log');
            function log(msg) {
                const div = document.createElement('div');
                div.textContent = msg;
                logDiv.appendChild(div);
                console.log(msg);
            }

            log('DOM Content Loaded');

            if (typeof LightweightCharts === 'undefined') {
                log('ERROR: LightweightCharts is undefined! Script not loaded?');
                return;
            } else {
                log('LightweightCharts is loaded.');
            }

            function createChart(id, color, data) {
                 const chart = LightweightCharts.createChart(document.getElementById(id), {
                    width: document.getElementById(id).clientWidth,
                    height: document.getElementById(id).clientHeight,
                    layout: {
                        backgroundColor: '#1e1e1e',
                        textColor: '#d1d4f9',
                    },
                    grid: {
                        vertLines: { color: '#2B2B43' },
                        horzLines: { color: '#2B2B43' },
                    },
                    timeScale: {
                        timeVisible: true,
                        secondsVisible: false,
                    }
                });

                const lineSeries = chart.addLineSeries({
                    color: color,
                    lineWidth: 2,
                });

                lineSeries.setData(data);
                chart.timeScale().fitContent();
                return chart;
            }
            
            const signalData = ${JSON.stringify(data.signal)};
            const pnlData = ${JSON.stringify(data.pnl)};
            const posData = ${JSON.stringify(data.position)};
            const turnoverData = ${JSON.stringify(data.turnover)};

            log('Data lengths: Signal=' + signalData.length + ', PnL=' + pnlData.length + ', Pos=' + posData.length + ', Turnover=' + turnoverData.length);
            
            if (signalData.length > 0) log('First signal point: ' + JSON.stringify(signalData[0]));
            
            const charts = [];
            
            if (signalData.length > 0) charts.push(createChart('chart-signal', '#e91e63', signalData));
            if (pnlData.length > 0) charts.push(createChart('chart-pnl', '#4caf50', pnlData));
            if (posData.length > 0) charts.push(createChart('chart-pos', '#2196f3', posData));
            if (turnoverData.length > 0) charts.push(createChart('chart-turnover', '#ff9800', turnoverData));

            window.addEventListener('resize', () => {
                charts.forEach(chart => {
                     // Simple resize logic, ideally bind to container id
                     // check lightweight-charts docs for specific resize logic per container
                });
                // Reload for simplicity on resize or implement ResizeObserver
                location.reload(); 
            });
        });
    </script>
</body>
</html>`;
}

export function deactivate() { }
