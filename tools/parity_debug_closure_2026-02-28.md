# Parity Debug Closure (2026-02-28)

## Scope
This note closes the ongoing Algotest vs bt parity investigation for entry/stop behavior in the weekly straddle workflow.

## Final Conclusion
The remaining stop-loss mismatches are primarily consistent with source data differences between the local dataset and the Algotest dataset, not with a deterministic stop-fill logic defect in bt.

## Evidence Artifacts
- Entry mismatch context (+/-1 minute):
  - tools/entry_price_mismatch_pm1.csv
  - tools/entry_price_mismatch_pm1.md
- Stop mismatch classification:
  - tools/stop_mismatch_classification.csv

## Stop Mismatch Classification Summary
From tools/stop_mismatch_classification.csv:
- data-mismatch-no-hit: 6
- minute-offset: 4
- timing-mismatch: 1
- non-stop-control: 1

Interpretation:
- Most unresolved cases are data-trigger mismatches where local post-entry highs do not reach the computed stop while Algotest marks stop-hit.
- A smaller set are minute timestamp convention offsets.
- One row is a control leg (non-stop baseline on that side).

## Operational Decision
Close this issue as parity-limited by dataset/source differences unless a dataset normalization effort is started.
