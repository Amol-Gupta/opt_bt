use std::fs;
use std::path::Path;

use crate::reporting::json::{BacktestReport, TradeRecord};

pub fn generate_html_report(report: &BacktestReport) -> String {
    let mut trades_rows = String::new();
    for trade in &report.trades {
        trades_rows.push_str(&render_trade_row(trade));
    }

    let mut strategy_rows = String::new();
    for entry in &report.portfolio.strategy_breakdown {
        strategy_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}%</td></tr>",
            html_escape(&entry.strategy_id),
            entry.trade_count,
            entry.realized_pnl,
            entry.fees_paid,
            entry.gross_notional,
            entry.pnl_contribution_pct,
        ));
    }

    let mut warning_items = String::new();
    for warning in &report.warnings {
        warning_items.push_str(&format!("<li>{}</li>", html_escape(warning)));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Backtest Report</title>
  <style>
    body {{ font-family: Arial, sans-serif; margin: 24px; color: #111; }}
    h1, h2 {{ margin: 0 0 12px; }}
    .section {{ margin-top: 24px; }}
    .grid {{ display: grid; grid-template-columns: repeat(3, minmax(180px, 1fr)); gap: 12px; }}
    .card {{ border: 1px solid #ddd; padding: 12px; border-radius: 8px; }}
    .label {{ color: #666; font-size: 12px; margin-bottom: 4px; }}
    .value {{ font-size: 18px; font-weight: 600; }}
    table {{ border-collapse: collapse; width: 100%; margin-top: 8px; }}
    th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; font-size: 13px; }}
    th {{ background: #f7f7f7; }}
    ul {{ margin: 8px 0 0 20px; }}
  </style>
</head>
<body>
  <h1>Backtest Report</h1>

  <div class="section">
    <h2>Summary Metrics</h2>
    <div class="grid">
      <div class="card"><div class="label">Total Return</div><div class="value">{:.2}%</div></div>
      <div class="card"><div class="label">CAGR</div><div class="value">{:.2}%</div></div>
      <div class="card"><div class="label">Max Drawdown</div><div class="value">{:.2}%</div></div>
      <div class="card"><div class="label">Sharpe</div><div class="value">{:.2}</div></div>
      <div class="card"><div class="label">Sortino</div><div class="value">{:.2}</div></div>
      <div class="card"><div class="label">Trade Count</div><div class="value">{}</div></div>
    </div>
  </div>

  <div class="section">
    <h2>Portfolio View</h2>
    <div class="grid">
      <div class="card"><div class="label">Strategies</div><div class="value">{}</div></div>
      <div class="card"><div class="label">Total Realized PnL</div><div class="value">{:.2}</div></div>
      <div class="card"><div class="label">Total Fees</div><div class="value">{:.2}</div></div>
    </div>
    <table>
      <thead>
        <tr><th>Strategy</th><th>Trades</th><th>Realized PnL</th><th>Fees</th><th>Gross Notional</th><th>PnL Contribution</th></tr>
      </thead>
      <tbody>{}</tbody>
    </table>
  </div>

  <div class="section">
    <h2>Trades</h2>
    <table>
      <thead>
        <tr><th>ID</th><th>Strategy</th><th>Symbol</th><th>Side</th><th>Qty</th><th>Entry</th><th>Exit</th><th>PnL</th></tr>
      </thead>
      <tbody>{}</tbody>
    </table>
  </div>

  <div class="section">
    <h2>Warnings</h2>
    <ul>{}</ul>
  </div>
</body>
</html>
"#,
        report.metrics.total_return_pct,
        report.metrics.cagr_pct,
        report.metrics.max_drawdown_pct,
        report.metrics.sharpe_ratio,
        report.metrics.sortino_ratio,
        report.metrics.trade_count,
        report.portfolio.strategy_count,
        report.portfolio.total_realized_pnl,
        report.portfolio.total_fees_paid,
        strategy_rows,
        trades_rows,
        warning_items,
    )
}

pub fn write_html_report<P: AsRef<Path>>(report: &BacktestReport, output_path: P) -> std::io::Result<()> {
    fs::write(output_path, generate_html_report(report))
}

fn render_trade_row(trade: &TradeRecord) -> String {
    format!(
        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td></tr>",
        trade.id,
        html_escape(&trade.strategy_id),
        html_escape(&trade.symbol),
        html_escape(&trade.side),
        trade.qty,
        trade.entry_price,
        trade.exit_price,
        trade.pnl,
    )
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
      .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::reporting::json::{
        BacktestReport, DatasetMetadata, Metrics, Reproducibility, Simulation, StrategyAttributionRecord,
        TradeRecord,
    };
    use crate::reporting::portfolio::PortfolioView;
    use crate::reporting::post_analysis::{PostAnalysisSummary, TaxSummary};

    fn sample_report() -> BacktestReport {
        BacktestReport {
            reproducibility: Reproducibility {
                engine_version: "v0".to_string(),
                strategy_version: "s0".to_string(),
                strategy_name: "demo".to_string(),
                parameters: HashMap::new(),
                config: HashMap::new(),
                dataset: DatasetMetadata {
                    source: "x".to_string(),
                  sha256: Some("y".to_string()),
                    granularity: "1m".to_string(),
                    start_date: "2024-01-01".to_string(),
                    end_date: "2024-01-02".to_string(),
                },
            },
            simulation: Simulation {
                start_time: "a".to_string(),
                end_time: "b".to_string(),
                duration_ms: 1,
                instrument_count: 1,
            },
            metrics: Metrics {
                total_return_pct: 1.5,
                cagr_pct: 1.2,
                sharpe_ratio: 0.9,
                sortino_ratio: 1.1,
                max_drawdown_pct: -2.3,
                trade_count: 1,
                win_rate_pct: 50.0,
                profit_factor: 1.0,
                margin_utilization_pct: 10.0,
                final_cash_balance: 1000.0,
            },
            post_analysis: PostAnalysisSummary {
                tax: TaxSummary {
                    model_name: "none".to_string(),
                    total_gross_pnl: 0.0,
                    total_tax: 0.0,
                    total_net_pnl: 0.0,
                    trade_count: 0,
                    trades: vec![],
                },
            },
            portfolio: PortfolioView {
                strategy_count: 1,
                total_trade_count: 1,
                total_realized_pnl: 10.0,
                total_fees_paid: 1.0,
                total_gross_notional: 100.0,
                strategy_breakdown: vec![],
            },
            strategy_attribution: HashMap::<String, StrategyAttributionRecord>::new(),
            trades: vec![TradeRecord {
                id: 1,
                strategy_id: "s1".to_string(),
                symbol: "NIFTY".to_string(),
                side: "Buy".to_string(),
                entry_time: "t1".to_string(),
                exit_time: "t2".to_string(),
                qty: 1,
                entry_price: 100.0,
                exit_price: 101.0,
                pnl: 1.0,
                stale_fill: false,
            }],
            warnings: vec!["ok".to_string()],
        }
    }

    #[test]
    fn test_generate_html_contains_core_sections() {
        let html = generate_html_report(&sample_report());
        assert!(html.contains("Backtest Report"));
        assert!(html.contains("Summary Metrics"));
        assert!(html.contains("Portfolio View"));
        assert!(html.contains("Trades"));
    }

    #[test]
    fn test_html_escape_applies() {
        let escaped = html_escape("a<b&c>");
        assert_eq!(escaped, "a&lt;b&amp;c&gt;");
    }
}
