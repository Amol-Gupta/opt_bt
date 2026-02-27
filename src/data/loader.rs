use std::path::Path;
use std::sync::Arc;
use anyhow::{Result, Context};
use polars::prelude::*;
use crate::data::models::{MarketData, Bar};
use crate::common::types::PRICE_SCALE;

pub struct DataLoader;

impl DataLoader {
    pub fn load_parquet(path: &str) -> Result<Arc<MarketData>> {
        let file_path = Path::new(path);
        if !file_path.exists() {
            return Err(anyhow::anyhow!("Data file not found: {}", path));
        }

        let df = LazyFrame::scan_parquet(file_path, Default::default())?
            .collect()?;

        let column_names = df.get_column_names();

        let symbol_col = find_column(&column_names, &["ticker", "symbol", "instrument", "tradingsymbol"])
            .context("Missing symbol/ticker column in parquet")?;
        let ts_col = find_column(&column_names, &["timestamp", "datetime", "time", "date", "ts"])
            .context("Missing timestamp column in parquet")?;
        let open_col = find_column(&column_names, &["open"])
            .context("Missing open column in parquet")?;
        let high_col = find_column(&column_names, &["high"])
            .context("Missing high column in parquet")?;
        let low_col = find_column(&column_names, &["low"])
            .context("Missing low column in parquet")?;
        let close_col = find_column(&column_names, &["close"])
            .context("Missing close column in parquet")?;
        let volume_col = find_column(&column_names, &["volume", "qty", "size"]);

        let mut md = MarketData::new();
        let height = df.height();

        for row_idx in 0..height {
            let symbol_val = df.column(symbol_col)?.get(row_idx)?;
            let symbol = anyvalue_to_symbol(symbol_val)?;

            let ts_val = df.column(ts_col)?.get(row_idx)?;
            let timestamp = anyvalue_to_epoch_seconds(ts_val)?;

            let open_val = df.column(open_col)?.get(row_idx)?;
            let high_val = df.column(high_col)?.get(row_idx)?;
            let low_val = df.column(low_col)?.get(row_idx)?;
            let close_val = df.column(close_col)?.get(row_idx)?;

            let volume = if let Some(vol_name) = volume_col {
                let vol_val = df.column(vol_name)?.get(row_idx)?;
                anyvalue_to_u64(vol_val).unwrap_or(0)
            } else {
                0
            };

            let bar = Bar {
                timestamp,
                open: to_scaled_price(anyvalue_to_f64(open_val)?),
                high: to_scaled_price(anyvalue_to_f64(high_val)?),
                low: to_scaled_price(anyvalue_to_f64(low_val)?),
                close: to_scaled_price(anyvalue_to_f64(close_val)?),
                volume,
            };

            md.add_bar(&symbol, bar);
        }

        for bars in md.bars.values_mut() {
            bars.sort_by_key(|bar| bar.timestamp);
        }

        Ok(Arc::new(md))
    }
}

fn find_column<'a>(columns: &'a [&str], candidates: &[&str]) -> Option<&'a str> {
    for candidate in candidates {
        if let Some(found) = columns.iter().find(|name| name.eq_ignore_ascii_case(candidate)) {
            return Some(*found);
        }
    }
    None
}

fn anyvalue_to_symbol(value: AnyValue<'_>) -> Result<String> {
    match value {
        AnyValue::String(v) => Ok(v.to_string()),
        AnyValue::StringOwned(v) => Ok(v.to_string()),
        other => Ok(other.to_string()),
    }
}

fn anyvalue_to_f64(value: AnyValue<'_>) -> Result<f64> {
    match value {
        AnyValue::Float64(v) => Ok(v),
        AnyValue::Float32(v) => Ok(v as f64),
        AnyValue::Int64(v) => Ok(v as f64),
        AnyValue::Int32(v) => Ok(v as f64),
        AnyValue::UInt64(v) => Ok(v as f64),
        AnyValue::UInt32(v) => Ok(v as f64),
        AnyValue::UInt16(v) => Ok(v as f64),
        AnyValue::Int16(v) => Ok(v as f64),
        AnyValue::UInt8(v) => Ok(v as f64),
        AnyValue::Int8(v) => Ok(v as f64),
        AnyValue::Decimal(v, scale) => Ok((v as f64) / 10f64.powi(scale as i32)),
        AnyValue::String(v) => v.parse::<f64>().context("Cannot parse numeric string"),
        AnyValue::StringOwned(v) => v.parse::<f64>().context("Cannot parse numeric string"),
        other => other.to_string().parse::<f64>().context("Cannot parse numeric value"),
    }
}

fn anyvalue_to_u64(value: AnyValue<'_>) -> Result<u64> {
    match value {
        AnyValue::UInt64(v) => Ok(v),
        AnyValue::UInt32(v) => Ok(v as u64),
        AnyValue::UInt16(v) => Ok(v as u64),
        AnyValue::UInt8(v) => Ok(v as u64),
        AnyValue::Int64(v) => Ok(v.max(0) as u64),
        AnyValue::Int32(v) => Ok(v.max(0) as u64),
        AnyValue::Int16(v) => Ok(v.max(0) as u64),
        AnyValue::Int8(v) => Ok(v.max(0) as u64),
        AnyValue::Float64(v) => Ok(v.max(0.0).round() as u64),
        AnyValue::Float32(v) => Ok(v.max(0.0).round() as u64),
        AnyValue::String(v) => Ok(v.parse::<u64>()?),
        AnyValue::StringOwned(v) => Ok(v.parse::<u64>()?),
        other => Ok(other.to_string().parse::<u64>()?),
    }
}

fn anyvalue_to_epoch_seconds(value: AnyValue<'_>) -> Result<i64> {
    let raw = match value {
        AnyValue::Datetime(v, TimeUnit::Nanoseconds, _) => v / 1_000_000_000,
        AnyValue::Datetime(v, TimeUnit::Microseconds, _) => v / 1_000_000,
        AnyValue::Datetime(v, TimeUnit::Milliseconds, _) => v / 1_000,
        AnyValue::Date(v) => (v as i64) * 86_400,
        AnyValue::Int64(v) => normalize_epoch(v),
        AnyValue::Int32(v) => normalize_epoch(v as i64),
        AnyValue::UInt64(v) => normalize_epoch(v as i64),
        AnyValue::UInt32(v) => normalize_epoch(v as i64),
        AnyValue::String(v) => parse_string_timestamp(v)?,
        AnyValue::StringOwned(v) => parse_string_timestamp(&v)?,
        other => normalize_epoch(other.to_string().parse::<i64>()?),
    };

    Ok(raw)
}

fn normalize_epoch(raw: i64) -> i64 {
    let abs = raw.unsigned_abs();
    if abs >= 100_000_000_000_000_000 {
        raw / 1_000_000_000
    } else if abs >= 100_000_000_000_000 {
        raw / 1_000_000
    } else if abs >= 100_000_000_000 {
        raw / 1_000
    } else {
        raw
    }
}

fn parse_string_timestamp(v: &str) -> Result<i64> {
    if let Ok(parsed) = v.parse::<i64>() {
        return Ok(normalize_epoch(parsed));
    }

    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(v) {
        return Ok(dt.timestamp());
    }

    Err(anyhow::anyhow!("Unsupported timestamp string format: {}", v))
}

fn to_scaled_price(v: f64) -> i64 {
    (v * PRICE_SCALE as f64).round() as i64
}
