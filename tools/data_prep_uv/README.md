# data_prep_uv

Utility project for inspecting large parquet datasets and generating small fixture subsets used by tests.

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
