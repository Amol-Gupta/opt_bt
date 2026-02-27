# Sample Dataset Fixtures

This folder contains lightweight parquet fixtures derived from large source files under `/quant` for faster local testing.

## Files
- `merged_data.sample.parquet`: subset derived from `/quant/merged_data.parquet`.
- `niftyIndex2024.sample.parquet`: subset derived from `/quant/niftyIndex2024.parquet`.
- `dataset_inspection.json`: schema and extraction metadata for both files.

## Regenerate
Run from repository root:

```bash
cd tools/data_prep_uv
uv run main.py
```

The extraction is deterministic (`sort by time + head N`) and additionally limits symbols for broad datasets.
