from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import polars as pl


ROOT = Path(__file__).resolve().parents[2]
SAMPLE_DIR = ROOT / "sample_data"

SOURCES = [
    Path("/quant/merged_data.parquet"),
    Path("/quant/niftyIndex2024.parquet"),
]

TIME_COL_CANDIDATES = ["timestamp", "ts", "datetime", "date", "time"]
SYMBOL_COL_CANDIDATES = [
    "symbol",
    "ticker",
    "instrument",
    "tradingsymbol",
    "instrument_name",
    "name",
]


def _find_col(schema: dict[str, Any], candidates: list[str]) -> str | None:
    cols_lower = {name.lower(): name for name in schema}
    for candidate in candidates:
        if candidate in cols_lower:
            return cols_lower[candidate]
    return None


def _choose_symbols(path: Path, symbol_col: str, max_symbols: int) -> list[str]:
    preview = pl.scan_parquet(path).select(pl.col(symbol_col)).head(20_000).collect()
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


def main() -> None:
    SAMPLE_DIR.mkdir(parents=True, exist_ok=True)

    reports: list[dict[str, Any]] = []

    for source in SOURCES:
        if not source.exists():
            raise FileNotFoundError(f"Missing parquet file: {source}")

        output_name = source.stem + ".sample.parquet"
        output_path = SAMPLE_DIR / output_name
        max_rows = 4_000 if "merged" in source.name.lower() else 2_000
        max_symbols = 10 if "merged" in source.name.lower() else 3

        report = _extract_sample(
            path=source,
            output_path=output_path,
            max_rows=max_rows,
            max_symbols=max_symbols,
        )
        reports.append(report)

    inspection_path = SAMPLE_DIR / "dataset_inspection.json"
    inspection_path.write_text(json.dumps({"datasets": reports}, indent=2), encoding="utf-8")

    print(f"Wrote {len(reports)} sampled parquet files to {SAMPLE_DIR}")
    print(f"Wrote dataset inspection report to {inspection_path}")


if __name__ == "__main__":
    main()
