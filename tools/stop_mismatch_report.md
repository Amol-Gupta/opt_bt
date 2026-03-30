# Stop Mismatch Report

Date: 2026-02-28

## Scope
This report summarizes the remaining stop-loss mismatches between bt backtest output and Algotest output for the weekly straddle parity run.

## Evidence
Primary evidence file:
- tools/stop_mismatch_classification.csv

Supporting entry context:
- tools/entry_price_mismatch_pm1.csv
- tools/entry_price_mismatch_pm1.md

## Classification Summary
From tools/stop_mismatch_classification.csv:
- data-mismatch-no-hit: 6
- minute-offset: 4
- timing-mismatch: 1
- non-stop-control: 1

## Interpretation
- A majority of unresolved stop mismatches are cases where local post-entry highs do not cross the stop threshold, while Algotest marks stop-loss hit.
- Some differences are minute-level timestamp convention offsets.
- One row is a non-stop control leg and not a stop-loss discrepancy baseline.

## Final Attribution
At this stage, the remaining discrepancy is attributed to data mismatch between my dataset (mydata) and the Algotest dataset, rather than a deterministic bt stop-fill logic defect.

## Status
Issue closed for stop-fill parity under current datasets.
