
# Rhetenor VS Code Extension Installation

This extension integrates **Rhetenor** (a Python-based backtesting engine) with **Butterflow** (a strategy definition language) into VS Code.

## Prerequisites

1.  **VS Code**: Ensure you have Visual Studio Code installed.
2.  **Node.js & npm**: Required to build the extension.
3.  **Python 3.10+**: Required to run the Rhetenor backend.
4.  **Rhetenor Package**: The `rhetenor` Python package must be installed in your environment.

## Installation Steps

### 1. Install Python Dependencies

Ensure `rhetenor` and its dependencies (including `butterflow`) are installed in your Python environment.

```bash
# In the root of the rhetenor repo
pip install -e .
```

### 2. Build and Install the Extension

The extension source code is located in `rhetenor-vscode`.

#### Option A: Run from Source (Debugging)
1.  Open the `rhetenor-vscode` folder in VS Code.
2.  Run `npm install` to install dependencies.
3.  Press **F5** to launch a new Extension Development Host window with the extension loaded.

#### Option B: Package and Install (.vsix)
1.  Navigate to the `rhetenor-vscode` directory.
2.  Install `vsce` (Visual Studio Code Extensions) globally if not already installed:
    ```bash
    npm install -g @vscode/vsce
    ```
3.  Package the extension:
    ```bash
    vsce package
    ```
    This will generate a `.vsix` file (e.g., `rhetenor-vscode-0.0.1.vsix`).
4.  Install the `.vsix` in VS Code:
    -   Open VS Code.
    -   Go to the Extensions view (Ctrl+Shift+X).
    -   Click the "..." menu at the top right -> `Install from VSIX...`
    -   Select the generated `.vsix` file.

## Configuration

After installation, you may need to configure the extension to point to your correct Python environment.

1.  Open VS Code Settings (Ctrl+,).
2.  Search for `rhetenor`.
3.  **Rhetenor: Python Path**: Set this to the absolute path of the python executable where `rhetenor` is installed (e.g., `/path/to/venv/bin/python`).
    -   Default: `python` (assumes it's in your system PATH).
4.  **Rhetenor: Module Path**: The python module to run. Default is `rhetenor`.

## Usage

1.  Open a Butterflow strategy file (`.bf`).
2.  Open the Command Palette (Ctrl+Shift+P).
3.  Run the command: `Rhetenor: Run Backtest`.
4.  The extension will:
    -   Execute the backtest using the configured Python environment.
    -   Generate a simulation result.
    -   Open a **Webview** displaying the interactive PnL/Signal chart.
