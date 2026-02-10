use serde::{Deserialize, Serialize};
use std::io::{self, Read};
use std::f64;

#[derive(Deserialize)]
struct InputData {
    equity_curve: Vec<f64>,
    trading_days_per_year: Option<f64>,
    risk_free_rate: Option<f64>,
}

#[derive(Serialize)]
struct OutputStats {
    sharpe_ratio: Option<f64>,
    sortino_ratio: Option<f64>,
    max_drawdown: f64,
    cagr: Option<f64>,
    annual_standard_deviation: Option<f64>,
    annual_variance: Option<f64>,
}

fn main() {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer).expect("Failed to read from stdin");

    let input: InputData = serde_json::from_str(&buffer).expect("Failed to parse JSON");
    let trading_days = input.trading_days_per_year.unwrap_or(252.0);
    let risk_free_rate = input.risk_free_rate.unwrap_or(0.0);

    let stats = calculate_statistics(&input.equity_curve, trading_days, risk_free_rate);

    let output_json = serde_json::to_string_pretty(&stats).expect("Failed to serialize output");
    println!("{}", output_json);
}

fn calculate_statistics(equity_curve: &[f64], trading_days: f64, risk_free_rate: f64) -> OutputStats {
    if equity_curve.len() < 2 {
        return OutputStats {
            sharpe_ratio: None,
            sortino_ratio: None,
            max_drawdown: 0.0,
            cagr: None,
            annual_standard_deviation: None,
            annual_variance: None,
        };
    }

    let mut returns = Vec::new();
    for i in 1..equity_curve.len() {
        if equity_curve[i-1] != 0.0 {
            returns.push((equity_curve[i] - equity_curve[i-1]) / equity_curve[i-1]);
        } else {
            returns.push(0.0);
        }
    }

    let max_drawdown = calculate_max_drawdown(equity_curve);
    let cagr = calculate_cagr(equity_curve, trading_days);
    
    let (annual_std_dev, annual_variance) = calculate_annual_risk(&returns, trading_days);
    let sharpe = calculate_sharpe_ratio(&returns, trading_days, risk_free_rate, annual_std_dev);
    let sortino = calculate_sortino_ratio(&returns, trading_days, risk_free_rate);

    OutputStats {
        sharpe_ratio: sharpe,
        sortino_ratio: sortino,
        max_drawdown,
        cagr,
        annual_standard_deviation: annual_std_dev,
        annual_variance,
    }
}

fn calculate_max_drawdown(equity_curve: &[f64]) -> f64 {
    let mut max_drawdown = 0.0;
    let mut peak = equity_curve[0];

    for value in equity_curve {
        if *value > peak {
            peak = *value;
        }
        let drawdown = (peak - value) / peak;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }
    max_drawdown
}

fn calculate_cagr(equity_curve: &[f64], trading_days: f64) -> Option<f64> {
    let start_value = equity_curve.first()?;
    let end_value = equity_curve.last()?;
    let years = (equity_curve.len() as f64) / trading_days;

    if years <= 0.0 || *start_value <= 0.0 || *end_value <= 0.0 {
        return None;
    }

    Some((end_value / start_value).powf(1.0 / years) - 1.0)
}

fn calculate_annual_risk(returns: &[f64], trading_days: f64) -> (Option<f64>, Option<f64>) {
     if returns.len() < 2 {
        return (None, None);
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
    
    let annual_variance = variance * trading_days;
    let annual_std_dev = annual_variance.sqrt();

    (Some(annual_std_dev), Some(annual_variance))
}

fn calculate_sharpe_ratio(returns: &[f64], trading_days: f64, risk_free_rate: f64, annual_std_dev: Option<f64>) -> Option<f64> {
    let std_dev = annual_std_dev?;
    if std_dev == 0.0 {
        return None;
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let annualized_return = (1.0 + mean).powf(trading_days) - 1.0;
    
    Some((annualized_return - risk_free_rate) / std_dev)
}

fn calculate_sortino_ratio(returns: &[f64], trading_days: f64, risk_free_rate: f64) -> Option<f64> {
     if returns.len() < 2 {
        return None;
    }

    let downside_returns: Vec<f64> = returns.iter().filter(|&&r| r < 0.0).cloned().collect();
    if downside_returns.is_empty() {
        return None; 
    }
    
    let downside_variance = downside_returns.iter().map(|&x| x.powi(2)).sum::<f64>() / downside_returns.len() as f64;
    let annualized_downside_deviation = (downside_variance * trading_days).sqrt();

    if annualized_downside_deviation == 0.0 {
        return None;
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let annualized_return = (1.0 + mean).powf(trading_days) - 1.0;

    Some((annualized_return - risk_free_rate) / annualized_downside_deviation)
}
