# data_prep_uv

Utility project for inspecting large parquet datasets and generating small fixture subsets used by tests.

## Reusable merge utility

Use `merge_market_data.py` to merge multiple parquet/csv files into one `bt`-compatible parquet.

Features:
- accepts multiple `--input` files (`.parquet` and `.csv`)
- auto-detects common symbol/timestamp/OHLC column names
- auto-corrects timestamp drift by trying `-05:30`, `+00:00`, `+05:30` shift and choosing the best market-hours alignment
- standardizes timestamps to `Asia/Kolkata`
- drops candles outside NSE market hours `09:15-15:30`
- validates index completeness for `09:16-15:30` (375 minutes/day)
- drops incomplete index days from index rows by default (`--drop-scope index-only`)

Example:

```bash
uv run merge_market_data.py \
	--input /quant/nifty_option_1min_ohlc_01Jan2023_31Mar2026.parquet \
	--input /quant/nifty_1min_ohlc_01Jan2023_31Mar2026.parquet \
	--output /quant/nifty_with_options_1min_ohlc_01Jan2023_31Mar2026.parquet \
	--index-symbol "NIFTY 50" \
	--drop-scope index-only
```

Useful switches:
- `--keep-incomplete-days` to keep all days even if index has missing minutes
- `--drop-scope all-symbols` to drop all symbols for days where index is incomplete
- `--dedupe` to deduplicate `(Ticker, timestamp)`
- `--report-path <path>` to customize JSON report output

## Input datasets
- `/quant/merged_data.parquet`
- `/quant/niftyIndex2024.parquet`

## Run

```bash
uv run main.py
```

## Output
Files are written to `../../sample_data/`:
- `merged_data.sample.parquet`
- `niftyIndex2024.sample.parquet`
- `dataset_inspection.json`
