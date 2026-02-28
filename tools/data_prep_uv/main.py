from __future__ import annotations

import json
import re
from datetime import timedelta
from pathlib import Path
from typing import Any

import polars as pl


ROOT = Path(__file__).resolve().parents[2]
SAMPLE_DIR = ROOT / "sample_data"
QUANT_DIR = Path("/quant")

TIME_COL_CANDIDATES = ["timestamp", "ts", "datetime", "date", "time"]
SYMBOL_COL_CANDIDATES = [
    "symbol",
    "ticker",
    "instrument",
    "tradingsymbol",
    "instrument_name",
    "name",
]
OHLC_COLS = ["open", "high", "low", "close"]


def _find_col(schema: dict[str, Any], candidates: list[str]) -> str | None:
    cols_lower = {name.lower(): name for name in schema}
    for candidate in candidates:
        if candidate in cols_lower:
            return cols_lower[candidate]
    return None


def _list_quant_parquet_files() -> list[Path]:
    if not QUANT_DIR.exists():
        return []
    return sorted(QUANT_DIR.glob("*.parquet"))


def _choose_symbols(path: Path, symbol_col: str, max_symbols: int) -> list[str]:
    preview = pl.scan_parquet(path).select(pl.col(symbol_col)).head(40_000).collect()
    symbols: list[str] = []
    seen: set[str] = set()
    for value in preview.get_column(symbol_col).to_list():
        if value is None:
            continue
        symbol = str(value)
        if symbol not in seen:
            seen.add(symbol)
            symbols.append(symbol)
        if len(symbols) >= max_symbols:
            break
    return symbols


def _extract_sample(path: Path, output_path: Path, max_rows: int, max_symbols: int) -> dict[str, Any]:
    schema = pl.scan_parquet(path).schema
    time_col = _find_col(schema, TIME_COL_CANDIDATES)
    symbol_col = _find_col(schema, SYMBOL_COL_CANDIDATES)

    lf = pl.scan_parquet(path)
    selected_symbols: list[str] = []

    if symbol_col is not None:
        selected_symbols = _choose_symbols(path, symbol_col, max_symbols=max_symbols)
        if selected_symbols:
            lf = lf.filter(pl.col(symbol_col).is_in(selected_symbols))

    if time_col is not None:
        lf = lf.sort(time_col)

    sample_df = lf.head(max_rows).collect()
    sample_df.write_parquet(output_path)

    return {
        "source": str(path),
        "output": str(output_path.relative_to(ROOT)),
        "rows": sample_df.height,
        "columns": sample_df.columns,
        "schema": {name: str(dtype) for name, dtype in schema.items()},
        "time_column": time_col,
        "symbol_column": symbol_col,
        "selected_symbols": selected_symbols,
    }


def _normalize_for_loader(path: Path) -> tuple[pl.LazyFrame, str, str]:
    schema = pl.scan_parquet(path).schema
    symbol_col = _find_col(schema, SYMBOL_COL_CANDIDATES)
    time_col = _find_col(schema, TIME_COL_CANDIDATES)
    if symbol_col is None or time_col is None:
        raise ValueError(f"Required symbol/time columns missing in {path}")

    lower_map = {name.lower(): name for name in schema}
    missing_ohlc = [name for name in OHLC_COLS if name not in lower_map]
    if missing_ohlc:
        raise ValueError(f"Missing OHLC columns {missing_ohlc} in {path}")

    lf = pl.scan_parquet(path).select(
        pl.col(symbol_col).cast(pl.Utf8).alias("Ticker"),
        pl.col(time_col).cast(pl.Int64).cast(pl.Datetime("ns")).alias("timestamp"),
        pl.col(lower_map["open"]).cast(pl.Float64).alias("Open"),
        pl.col(lower_map["high"]).cast(pl.Float64).alias("High"),
        pl.col(lower_map["low"]).cast(pl.Float64).alias("Low"),
        pl.col(lower_map["close"]).cast(pl.Float64).alias("Close"),
    )
    return lf, symbol_col, time_col


def _build_nifty_consolidated_dataset(index_path: Path, options_path: Path) -> dict[str, Any]:
    index_lf, _, _ = _normalize_for_loader(index_path)
    options_lf, _, _ = _normalize_for_loader(options_path)

    option_symbol_pattern = r"^NIFTY\d{2}[A-Z]{3}\d{2}\d+(CE|PE)$"

    options_filtered = options_lf.filter(
        pl.col("Ticker").str.contains(option_symbol_pattern)
    )

    minute_counts = (
        options_filtered
        .group_by("timestamp")
        .agg(pl.len().alias("cnt"))
        .sort("cnt", descending=True)
        .head(1)
        .collect()
    )

    if minute_counts.height == 0:
        raise RuntimeError("No NIFTY option rows found in options dataset")

    anchor_ts = minute_counts.item(0, "timestamp")
    if hasattr(anchor_ts, "year"):
        ts_lower = anchor_ts - timedelta(days=7)
        ts_upper = anchor_ts + timedelta(days=7)
    else:
        ts_lower = anchor_ts - 7 * 24 * 60 * 60 * 1_000_000_000
        ts_upper = anchor_ts + 7 * 24 * 60 * 60 * 1_000_000_000

    options_window = options_filtered.filter(
        (pl.col("timestamp") >= pl.lit(ts_lower)) & (pl.col("timestamp") <= pl.lit(ts_upper))
    )

    candidate_symbols = (
        options_window
        .group_by("Ticker")
        .agg(pl.len().alias("cnt"))
        .sort("cnt", descending=True)
        .head(20)
        .select("Ticker")
        .collect()
        .get_column("Ticker")
        .to_list()
    )

    options_final = options_window.filter(pl.col("Ticker").is_in(candidate_symbols))
    index_final = index_lf.filter(pl.col("Ticker").str.to_uppercase() == pl.lit("NIFTY 50"))

    combined = (
        pl.concat([index_final, options_final], how="vertical")
        .sort(["timestamp", "Ticker"])
        .collect()
    )

    output_path = SAMPLE_DIR / "nifty_with_options.sample.parquet"
    combined.write_parquet(output_path)

    unique_symbols = combined.select(pl.col("Ticker").n_unique()).item()

    return {
        "source": f"composed:{index_path}+{options_path}",
        "output": str(output_path.relative_to(ROOT)),
        "rows": combined.height,
        "columns": combined.columns,
        "schema": {name: str(dtype) for name, dtype in combined.schema.items()},
        "time_column": "timestamp",
        "symbol_column": "Ticker",
        "selected_symbols": candidate_symbols,
        "unique_symbols": unique_symbols,
        "anchor_timestamp": str(anchor_ts),
    }


def main() -> None:
    SAMPLE_DIR.mkdir(parents=True, exist_ok=True)

    sources = _list_quant_parquet_files()
    if not sources:
        raise FileNotFoundError("No parquet files found under /quant")

    reports: list[dict[str, Any]] = []
    for source in sources:
        output_name = source.stem + ".sample.parquet"
        output_path = SAMPLE_DIR / output_name
        max_rows = 4_000 if "merged" in source.name.lower() else 2_000
        max_symbols = 10 if "merged" in source.name.lower() else 5
        reports.append(
            _extract_sample(
                path=source,
                output_path=output_path,
                max_rows=max_rows,
                max_symbols=max_symbols,
            )
        )

    index_path = QUANT_DIR / "niftyIndex2024.parquet"
    options_path = QUANT_DIR / "merged_data.parquet"
    if index_path.exists() and options_path.exists():
        reports.append(_build_nifty_consolidated_dataset(index_path=index_path, options_path=options_path))

    inspection_path = SAMPLE_DIR / "dataset_inspection.json"
    inspection_path.write_text(json.dumps({"datasets": reports}, indent=2), encoding="utf-8")

    print(f"Discovered {len(sources)} parquet file(s) under /quant")
    for source in sources:
        print(f"  - {source}")
    print(f"Wrote sampled and consolidated parquet files to {SAMPLE_DIR}")
    print(f"Wrote dataset inspection report to {inspection_path}")


if __name__ == "__main__":
    main()
