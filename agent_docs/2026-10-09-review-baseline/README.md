# #49 review kind baseline — 20 runs, before/after

Per-run evidence for #91's prompt reword and `_normalise_issue_kind` (`src-tauri/sidecar/routes/jobs.py`), requested in review (#91, Ask 4) so the before/after table in the PR body is reviewable without reaching into a contributor's own machine.

## What changed between the two runs

Both are 20 reviews of the same fictional sample NDA (`sample_nda.txt`, not included here — it carries no evidence of its own, only the model's output does) against a scratch sidecar running `llama3.2`, through the real `/review` → poll-for-result route.

- **Before**: the `REVIEW` prompt (`services/prompts.py`) showed the five issue kinds as its own literal example, `risk|ambiguity|missing|style|other` — the model sometimes echoed that pipe-joined string straight back as a `kind` value. No normalisation existed; an off-contract `kind` just fell through whatever happened to read it.
- **After**: the prompt instead describes the five kinds in words and asks for exactly one per issue. `_normalise_issue_kind` also now recovers an obvious kind from a pipe-joined or pluralised value before validation; anything it can't resolve is left for `_safe_construct`'s enum validation to reset to the field's own default (`"other"`).

## Before/after

| | Before | After |
|---|---|---|
| Completed (parsed) | 14/20 | 17/20 |
| Failed (unparseable JSON) | 6/20 (30%) | 3/20 (15%) |
| Runs with ≥1 off-contract `kind` | 4/14 | 0/17 |
| Issues `_safe_construct` would drop | 0/20 | 0/20 |

The off-contract `kind` values disappeared entirely in the "after" run — consistent with the reworded prompt removing the literal pipe-joined template text the model had been echoing back. The parse-failure drop (30% → 15%) is noted but not claimed as proven: n=20 is a small, noisy sample for that number, and the prompt change targeted `kind`, not JSON validity. Either way, `_safe_construct` never dropped an issue in either run (every issue had a string `message`, the only required field), so returning the validated payload from `get_review_result` loses nothing in this sample.

## The data

- `summary-before.json` — one record per run, before the prompt reword and kind normalisation.
- `summary.json` — one record per run, after.

Each record: `run` (1–20), `job_id`, `elapsed_seconds`, `job_status` (`completed`/`failed`), `raw_issue_count`, `raw_issues_is_list`, `kind_values_seen`, `kind_values_outside_contract`, `survives_safe_construct`, `surviving_issue_count`. No document text or model output content is included — only shape and the `kind` label strings themselves (e.g. `"risk|ambiguity"`, `"rhetorical error"`), which is what this baseline measures.

## How the runs were made

`run_baseline.py` (the harness itself, not committed here — this folder is the evidence it produced, not the tool) posts the sample NDA to `/review` and polls `/review/{job_id}/result` until `completed`/`failed`, importing the sidecar's own `_safe_construct`/`ReviewResponse` (not a reimplementation) to check whether each raw result would survive validation. Run from the sidecar's own venv against a local scratch sidecar on `:11437`:

```
cd src-tauri/sidecar && uv run python <path-to-run_baseline.py>
```
