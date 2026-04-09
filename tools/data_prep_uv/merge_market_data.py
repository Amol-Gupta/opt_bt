from __future__ import annotations

import argparse
import json
import math
import shutil
import tempfile
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Iterable

import polars as pl
import pyarrow.parquet as pq


IST_OFFSET_MINUTES = 5 * 60 + 30
MARKET_OPEN_MINUTE = 9 * 60 + 15
MARKET_CLOSE_MINUTE = 15 * 60 + 30
INDEX_COMPLETENESS_START_MINUTE = 9 * 60 + 16
INDEX_EXPECTED_MINUTES = 375

SYMBOL_CANDIDATES = [
    "ticker",
    "symbol",
    "instrument",
    "tradingsymbol",
    "instrument_name",
    "name",
]
TIMESTAMP_CANDIDATES = ["timestamp", "datetime", "time", "date", "ts"]
OPEN_CANDIDATES = ["open", "o"]
HIGH_CANDIDATES = ["high", "h"]
LOW_CANDIDATES = ["low", "l"]
CLOSE_CANDIDATES = ["close", "c"]
VOLUME_CANDIDATES = ["volume", "qty", "size"]


@dataclass
class FileColumns:
    symbol: str
    timestamp: str
    open: str
    high: str
    low: str
    close: str
    volume: str | None


@dataclass
class FileTimezoneDecision:
    path: str
    shift_minutes: int
    score_raw: float
    score_minus_330: float
    score_plus_330: float
    sample_size: int
    mode: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Merge multiple CSV/Parquet market-data files into bt-compatible parquet with "
            "automatic timezone-shift correction and NSE trading-hours filtering."
        )
    )
    parser.add_argument(
        "--input",
        dest="inputs",
        action="append",
        required=True,
        help="Input file path (repeatable). Supports .parquet and .csv",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Output merged parquet file path",
    )
    parser.add_argument(
        "--index-symbol",
        action="append",
        default=["NIFTY 50"],
        help="Index symbol to validate completeness against (repeatable)",
    )
    parser.add_argument(
        "--drop-incomplete-days",
        action="store_true",
        default=True,
        help="Drop incomplete index days (default: true)",
    )
    parser.add_argument(
        "--keep-incomplete-days",
        action="store_true",
        help="Keep incomplete index days (overrides --drop-incomplete-days)",
    )
    parser.add_argument(
        "--drop-scope",
        choices=["index-only", "all-symbols"],
        default="index-only",
        help="When dropping incomplete days, drop only index rows or all rows for those dates",
    )
    parser.add_argument(
        "--dedupe",
        action="store_true",
        help="Drop duplicate rows by (Ticker, timestamp)",
    )
    parser.add_argument(
        "--sample-size",
        type=int,
        default=100_000,
        help="Sample size per file for timezone-shift detection",
    )
    parser.add_argument(
        "--report-path",
        default=None,
        help="Optional JSON report output path",
    )
    return parser.parse_args()


def _is_csv(path: Path) -> bool:
    return path.suffix.lower() == ".csv"


def _scan(path: Path) -> pl.LazyFrame:
    if _is_csv(path):
        return pl.scan_csv(path, ignore_errors=True, infer_schema_length=10_000)
    return pl.scan_parquet(path)


def _find_column(schema: dict[str, Any], candidates: Iterable[str]) -> str | None:
    lowered = {col.lower(): col for col in schema}
    for candidate in candidates:
        if candidate in lowered:
            return lowered[candidate]
    return None


def _resolve_columns(path: Path) -> FileColumns:
    schema = _scan(path).collect_schema()
    symbol = _find_column(schema, SYMBOL_CANDIDATES)
    timestamp = _find_column(schema, TIMESTAMP_CANDIDATES)
    open_col = _find_column(schema, OPEN_CANDIDATES)
    high_col = _find_column(schema, HIGH_CANDIDATES)
    low_col = _find_column(schema, LOW_CANDIDATES)
    close_col = _find_column(schema, CLOSE_CANDIDATES)
    volume_col = _find_column(schema, VOLUME_CANDIDATES)

    missing = []
    if symbol is None:
        missing.append("symbol")
    if timestamp is None:
        missing.append("timestamp")
    if open_col is None:
        missing.append("open")
    if high_col is None:
        missing.append("high")
    if low_col is None:
        missing.append("low")
    if close_col is None:
        missing.append("close")

    if missing:
        raise ValueError(f"{path}: missing required columns: {missing}")

    return FileColumns(
        symbol=symbol,
        timestamp=timestamp,
        open=open_col,
        high=high_col,
        low=low_col,
        close=close_col,
        volume=volume_col,
    )


def _normalize_epoch_seconds(raw: int) -> int:
    abs_value = abs(raw)
    if abs_value >= 100_000_000_000_000_000:
        return raw // 1_000_000_000
    if abs_value >= 100_000_000_000_000:
        return raw // 1_000_000
    if abs_value >= 100_000_000_000:
        return raw // 1_000
    return raw


def _parse_timestamp_scalar(value: Any) -> datetime | None:
    if value is None:
        return None

    if isinstance(value, datetime):
        if value.tzinfo is None:
            return value.replace(tzinfo=timezone.utc)
        return value.astimezone(timezone.utc)

    if isinstance(value, str):
        text = value.strip()
        if not text:
            return None
        try:
            parsed = datetime.fromisoformat(text.replace("Z", "+00:00"))
            if parsed.tzinfo is None:
                return parsed.replace(tzinfo=timezone.utc)
            return parsed.astimezone(timezone.utc)
        except ValueError:
            pass
        for fmt in (
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M:%S.%f",
            "%d-%m-%Y %H:%M:%S",
            "%d/%m/%Y %H:%M:%S",
        ):
            try:
                return datetime.strptime(text, fmt).replace(tzinfo=timezone.utc)
            except ValueError:
                continue
        if text.isdigit() or (text.startswith("-") and text[1:].isdigit()):
            raw = int(text)
            seconds = _normalize_epoch_seconds(raw)
            return datetime.fromtimestamp(seconds, tz=timezone.utc)
        return None

    if isinstance(value, (int, float)) and not (isinstance(value, float) and math.isnan(value)):
        raw = int(value)
        seconds = _normalize_epoch_seconds(raw)
        return datetime.fromtimestamp(seconds, tz=timezone.utc)

    return None


def _minute_of_day_ist_from_utc(dt_utc: datetime, shift_minutes: int) -> int:
    corrected = dt_utc + timedelta(minutes=shift_minutes + IST_OFFSET_MINUTES)
    return corrected.hour * 60 + corrected.minute


def _score_market_alignment(values: list[Any], shift_minutes: int) -> float:
    usable = 0
    in_hours = 0
    for value in values:
        parsed = _parse_timestamp_scalar(value)
        if parsed is None:
            continue
        usable += 1
        minute_of_day = _minute_of_day_ist_from_utc(parsed, shift_minutes)
        if MARKET_OPEN_MINUTE <= minute_of_day <= MARKET_CLOSE_MINUTE:
            in_hours += 1

    if usable == 0:
        return 0.0
    return in_hours / usable


def _choose_shift_minutes(
    path: Path,
    cols: FileColumns,
    sample_size: int,
    index_symbols: set[str],
) -> FileTimezoneDecision:
    sample_df = (
        _scan(path)
        .select(
            pl.col(cols.symbol).cast(pl.Utf8).alias("_symbol"),
            pl.col(cols.timestamp).alias("_timestamp"),
        )
        .head(sample_size)
        .collect()
    )

    if sample_df.height == 0:
        return FileTimezoneDecision(
            path=str(path),
            shift_minutes=0,
            score_raw=0.0,
            score_minus_330=0.0,
            score_plus_330=0.0,
            sample_size=0,
            mode="empty",
        )

    pairs = list(zip(sample_df.get_column("_symbol").to_list(), sample_df.get_column("_timestamp").to_list()))
    index_values = [ts for symbol, ts in pairs if str(symbol).upper() in index_symbols]
    values = index_values if index_values else [ts for _, ts in pairs]
    mode = "index-only" if index_values else "all-symbols"

    score_raw = _score_market_alignment(values, 0)
    score_minus = _score_market_alignment(values, -IST_OFFSET_MINUTES)
    score_plus = _score_market_alignment(values, IST_OFFSET_MINUTES)

    scores = {
        0: score_raw,
        -IST_OFFSET_MINUTES: score_minus,
        IST_OFFSET_MINUTES: score_plus,
    }
    best_shift = max(scores, key=scores.get)
    best_score = scores[best_shift]

    # Prefer no shift unless a non-zero shift gives a clear improvement.
    if best_shift != 0 and (best_score - score_raw) < 0.10:
        best_shift = 0

    return FileTimezoneDecision(
        path=str(path),
        shift_minutes=best_shift,
        score_raw=score_raw,
        score_minus_330=score_minus,
        score_plus_330=score_plus,
        sample_size=len(values),
        mode=mode,
    )


def _timestamp_expr_for_dtype(dtype: pl.DataType, col_name: str) -> pl.Expr:
    col = pl.col(col_name)

    if isinstance(dtype, pl.Datetime):
        if dtype.time_zone is None:
            return col.dt.replace_time_zone("UTC")
        return col.dt.convert_time_zone("UTC")

    if dtype == pl.Date:
        return col.cast(pl.Datetime("us")).dt.replace_time_zone("UTC")

    if dtype in {
        pl.Int8,
        pl.Int16,
        pl.Int32,
        pl.Int64,
        pl.UInt8,
        pl.UInt16,
        pl.UInt32,
        pl.UInt64,
    }:
        int_col = col.cast(pl.Int64)
        sec = (
            pl.when(int_col.abs() >= pl.lit(100_000_000_000_000_000, dtype=pl.Int64))
            .then(int_col // 1_000_000_000)
            .when(int_col.abs() >= pl.lit(100_000_000_000_000, dtype=pl.Int64))
            .then(int_col // 1_000_000)
            .when(int_col.abs() >= pl.lit(100_000_000_000, dtype=pl.Int64))
            .then(int_col // 1_000)
            .otherwise(int_col)
        )
        return pl.from_epoch(sec, time_unit="s").dt.replace_time_zone("UTC")

    string_col = col.cast(pl.Utf8)
    return (
        string_col.str.to_datetime(time_zone="UTC", strict=False)
        .fill_null(string_col.str.to_datetime("%Y-%m-%d %H:%M:%S", time_zone="UTC", strict=False))
        .fill_null(string_col.str.to_datetime("%Y-%m-%d %H:%M:%S%.f", time_zone="UTC", strict=False))
        .fill_null(string_col.str.to_datetime("%d-%m-%Y %H:%M:%S", time_zone="UTC", strict=False))
        .fill_null(string_col.str.to_datetime("%d/%m/%Y %H:%M:%S", time_zone="UTC", strict=False))
    )


def _normalize_single_file(
    path: Path,
    output_path: Path,
    shift_minutes: int,
    dedupe: bool,
) -> dict[str, Any]:
    cols = _resolve_columns(path)
    lf = _scan(path)
    schema = lf.collect_schema()

    ts_expr = _timestamp_expr_for_dtype(schema[cols.timestamp], cols.timestamp)
    if shift_minutes != 0:
        ts_expr = ts_expr + pl.duration(minutes=shift_minutes)

    ts_expr = ts_expr.dt.convert_time_zone("Asia/Kolkata")

    normalized = lf.select(
        pl.col(cols.symbol).cast(pl.Utf8).alias("Ticker"),
        ts_expr.alias("timestamp"),
        pl.col(cols.open).cast(pl.Float64).alias("Open"),
        pl.col(cols.high).cast(pl.Float64).alias("High"),
        pl.col(cols.low).cast(pl.Float64).alias("Low"),
        pl.col(cols.close).cast(pl.Float64).alias("Close"),
        (
            pl.col(cols.volume).cast(pl.Float64).fill_null(0.0).round(0).cast(pl.UInt64)
            if cols.volume is not None
            else pl.lit(0, dtype=pl.UInt64)
        ).alias("Volume"),
    ).drop_nulls(["Ticker", "timestamp", "Open", "High", "Low", "Close"])

    minute_of_day = (
        pl.col("timestamp").dt.hour().cast(pl.Int32) * 60
        + pl.col("timestamp").dt.minute().cast(pl.Int32)
    )
    normalized = normalized.filter(
        (minute_of_day >= MARKET_OPEN_MINUTE) & (minute_of_day <= MARKET_CLOSE_MINUTE)
    )

    if dedupe:
        normalized = normalized.unique(subset=["Ticker", "timestamp"], keep="last")

    normalized = normalized.sort(["timestamp", "Ticker"])
    normalized.sink_parquet(output_path, compression="zstd")

    rows = pl.scan_parquet(output_path).select(pl.len()).collect().item()
    return {
        "path": str(path),
        "normalized_file": str(output_path),
        "rows_after_normalization": int(rows),
    }


def _merge_normalized_files(temp_files: list[Path], output_path: Path) -> int:
    if not temp_files:
        raise ValueError("No normalized files to merge")

    writer: pq.ParquetWriter | None = None
    total_rows = 0
    schema = None

    try:
        for file_path in temp_files:
            parquet_file = pq.ParquetFile(file_path)
            if schema is None:
                schema = parquet_file.schema_arrow
                writer = pq.ParquetWriter(
                    output_path,
                    schema=schema,
                    compression="zstd",
                    use_dictionary=True,
                )

            if parquet_file.schema_arrow != schema:
                raise ValueError(f"Schema mismatch in normalized file {file_path}")

            for row_group in range(parquet_file.num_row_groups):
                table = parquet_file.read_row_group(row_group)
                writer.write_table(table)
                total_rows += table.num_rows
    finally:
        if writer is not None:
            writer.close()

    return total_rows


def _collect_incomplete_index_days(
    merged_path: Path,
    index_symbols: set[str],
) -> list[str]:
    index_rows = pl.scan_parquet(merged_path).filter(
        pl.col("Ticker").str.to_uppercase().is_in(sorted(index_symbols))
    )

    minute_of_day = (
        pl.col("timestamp").dt.hour().cast(pl.Int32) * 60
        + pl.col("timestamp").dt.minute().cast(pl.Int32)
    )
    index_window = index_rows.filter(
        (minute_of_day >= INDEX_COMPLETENESS_START_MINUTE) & (minute_of_day <= MARKET_CLOSE_MINUTE)
    )

    per_day = (
        index_window
        .with_columns(pl.col("timestamp").dt.date().alias("trade_date"))
        .group_by("trade_date")
        .agg(pl.col("timestamp").n_unique().alias("minute_count"))
        .collect()
    )

    if per_day.height == 0:
        return []

    bad = per_day.filter(pl.col("minute_count") != INDEX_EXPECTED_MINUTES)
    return [str(v) for v in bad.get_column("trade_date").to_list()]


def _index_completeness_summary(
    merged_path: Path,
    index_symbols: set[str],
) -> dict[str, Any]:
    index_rows = pl.scan_parquet(merged_path).filter(
        pl.col("Ticker").str.to_uppercase().is_in(sorted(index_symbols))
    )

    minute_of_day = (
        pl.col("timestamp").dt.hour().cast(pl.Int32) * 60
        + pl.col("timestamp").dt.minute().cast(pl.Int32)
    )
    index_window = index_rows.filter(
        (minute_of_day >= INDEX_COMPLETENESS_START_MINUTE) & (minute_of_day <= MARKET_CLOSE_MINUTE)
    )

    per_day = (
        index_window
        .with_columns(pl.col("timestamp").dt.date().alias("trade_date"))
        .group_by("trade_date")
        .agg(pl.col("timestamp").n_unique().alias("minute_count"))
        .collect()
    )

    total_days = int(per_day.height)
    complete_days = int(per_day.filter(pl.col("minute_count") == INDEX_EXPECTED_MINUTES).height)
    incomplete_days = total_days - complete_days
    pct_complete = (complete_days / total_days * 100.0) if total_days else 0.0

    return {
        "total_days": total_days,
        "complete_days": complete_days,
        "incomplete_days": incomplete_days,
        "pct_complete": round(pct_complete, 2),
        "expected_minutes": INDEX_EXPECTED_MINUTES,
    }


def _drop_days(
    merged_path: Path,
    output_path: Path,
    bad_days: list[str],
    scope: str,
    index_symbols: set[str],
) -> int:
    if not bad_days:
        shutil.move(merged_path, output_path)
        return pl.scan_parquet(output_path).select(pl.len()).collect().item()

    base = pl.scan_parquet(merged_path).with_columns(pl.col("timestamp").dt.date().cast(pl.Utf8).alias("_date"))

    bad_day_match = pl.lit(False)
    for bad_day in bad_days:
        bad_day_match = bad_day_match | (pl.col("_date") == pl.lit(bad_day))

    if scope == "all-symbols":
        filtered = base.filter(~bad_day_match)
    else:
        index_filter = pl.col("Ticker").str.to_uppercase().is_in(sorted(index_symbols))
        filtered = base.filter(~(index_filter & bad_day_match))

    filtered = filtered.drop("_date").sort(["timestamp", "Ticker"])
    filtered.sink_parquet(output_path, compression="zstd")

    return int(pl.scan_parquet(output_path).select(pl.len()).collect().item())


def main() -> None:
    args = parse_args()

    drop_incomplete_days = args.drop_incomplete_days and not args.keep_incomplete_days
    input_paths = [Path(p).expanduser().resolve() for p in args.inputs]
    output_path = Path(args.output).expanduser().resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)

    for path in input_paths:
        if not path.exists():
            raise FileNotFoundError(f"Input file not found: {path}")
        if path.suffix.lower() not in {".parquet", ".csv"}:
            raise ValueError(f"Unsupported file extension for {path}; use .parquet or .csv")

    index_symbols = {symbol.upper() for symbol in args.index_symbol}

    tz_decisions: list[FileTimezoneDecision] = []
    per_file_reports: list[dict[str, Any]] = []
    temp_files: list[Path] = []

    with tempfile.TemporaryDirectory(prefix="bt_merge_") as temp_dir_name:
        temp_dir = Path(temp_dir_name)

        for idx, path in enumerate(input_paths, start=1):
            cols = _resolve_columns(path)
            decision = _choose_shift_minutes(
                path=path,
                cols=cols,
                sample_size=args.sample_size,
                index_symbols=index_symbols,
            )
            tz_decisions.append(decision)

            normalized_file = temp_dir / f"normalized_{idx:03d}.parquet"
            file_report = _normalize_single_file(
                path=path,
                output_path=normalized_file,
                shift_minutes=decision.shift_minutes,
                dedupe=args.dedupe,
            )
            temp_files.append(normalized_file)
            per_file_reports.append(file_report)

            print(
                "normalized",
                f"file={path}",
                f"shift_minutes={decision.shift_minutes}",
                f"rows={file_report['rows_after_normalization']}",
            )

        merged_tmp = temp_dir / "merged_tmp.parquet"
        merged_rows = _merge_normalized_files(temp_files=temp_files, output_path=merged_tmp)
        print(f"merged_temp_rows={merged_rows}")

        bad_days = _collect_incomplete_index_days(merged_tmp, index_symbols=index_symbols)
        completeness = _index_completeness_summary(merged_tmp, index_symbols=index_symbols)
        print(f"incomplete_index_days={len(bad_days)}")
        print(
            "index_completeness",
            f"complete_days={completeness['complete_days']}",
            f"total_days={completeness['total_days']}",
            f"pct_complete={completeness['pct_complete']}",
        )

        final_rows = (
            _drop_days(
                merged_path=merged_tmp,
                output_path=output_path,
                bad_days=bad_days if drop_incomplete_days else [],
                scope=args.drop_scope,
                index_symbols=index_symbols,
            )
            if drop_incomplete_days
            else (_merge_normalized_files(temp_files=temp_files, output_path=output_path))
        )

    report = {
        "inputs": [str(p) for p in input_paths],
        "output": str(output_path),
        "drop_incomplete_days": drop_incomplete_days,
        "drop_scope": args.drop_scope,
        "index_symbols": sorted(index_symbols),
        "timezone_decisions": [
            {
                "path": d.path,
                "shift_minutes": d.shift_minutes,
                "score_raw": d.score_raw,
                "score_minus_330": d.score_minus_330,
                "score_plus_330": d.score_plus_330,
                "sample_size": d.sample_size,
                "mode": d.mode,
            }
            for d in tz_decisions
        ],
        "file_reports": per_file_reports,
        "final_rows": int(final_rows),
        "incomplete_days": bad_days,
        "market_hours": "09:15-15:30 Asia/Kolkata",
        "index_completeness_window": "09:16-15:30 Asia/Kolkata",
        "index_expected_minutes": INDEX_EXPECTED_MINUTES,
        "index_completeness": completeness,
    }

    report_path = Path(args.report_path).expanduser().resolve() if args.report_path else output_path.with_suffix(".report.json")
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")

    print(f"output={output_path}")
    print(f"report={report_path}")
    print(f"final_rows={final_rows}")


if __name__ == "__main__":
    main()