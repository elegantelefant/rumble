# #90 review parse-reliability evidence

Evidence behind #92's repair-before-retry fix, requested in review (#92, Ask 4) so the before/after numbers in the PR body are reviewable without reaching into a contributor's own machine, the same way as #91's `agent_docs/2026-10-09-review-baseline/`.

## 1. Offline comparison, 16 real failed attempts (zero new model calls)

`offline-repair-comparison.json` — one entry per genuinely failed parse attempt from a 60-run live batch (done on `fix/49-review-reads-contract`, which already has #91's reworded REVIEW prompt). Each failed attempt's saved raw text was re-run offline against four parsing strategies and checked against `_safe_construct(ReviewResponse, ...)`:

| strategy | rescued | passes `_safe_construct` | drops content |
|---|---|---|---|
| `json.loads(text, strict=False)` | 0/16 | — | — |
| narrow repair (trailing comma / bad escape / stray control char) | 2/16 | 2/2 | 0 |
| narrow repair + `strict=False` | 2/16 | 2/2 | 0 |
| `json-repair` package | 15/16 | 15/15 | 0 |

The one failure nothing could fix was genuinely garbled structure — the model closed the issues array early, then wrote a second, separately-malformed fragment right after it — not a syntax slip.

Each entry has: `run`, `attempt`, `original_error` (the JSON decode error message), `issues_visible_in_raw` (how many issues were readable in the raw text despite the parse failure, for a rough sense of how much a rescue actually recovers), and `strategies` (one record per strategy: `rescued`, and when rescued, `passes_safe_construct`/`rescued_issue_count`/`drops_content`). No document text or model output content is included — only decode-error strings and counts.

## 2. Production batch, 60 reviews through the compiled binary's real `/review` endpoint

`production-batch-summary.json` — one entry per run (`run`, `job_id`, `status`, `elapsed_seconds`), stripped of each run's full `result` (summary/issues text) to keep this file to metadata only, the same way #91's baseline summaries only kept run metadata. **This batch used fresh `main`, without #91's reworded prompt** — the same original REVIEW wording the very first #49 baseline measured — confirmed directly (`git diff main -- services/prompts.py` was empty on the branch this ran on).

| | original prompt, no repair/retry (#91's `summary-before.json`) | original prompt, this batch (#92) |
|---|---|---|
| Completed | 14/20 (70%) | **60/60 (100%)** |
| Failed | 6/20 (30%) | 0/60 |

Repair fired 14 times (log-verified: `grep -c "Repaired malformed LLM JSON output"`); retry fired once (log-verified: `grep -c "Review retry: parsing failed"`), and that retry succeeded. Timing: median 26.2s, range 16.1–56.4s. These per-run repair/retry outcomes aren't fields in the summary (they were counted from the sidecar's own log, not derived from the result content) — the table above is the log-verified count, not a re-derivation from this file.

**This fix and #91's reworded prompt haven't yet been measured together end to end**: the offline test's 16 failures came from a batch using #91's prompt, and this production batch used the original prompt. The combined result — this fix's repair/retry *on top of* #91's prompt — is inferred from these two separate measurements, not directly measured. A combined 20-run batch is planned after #91 merges (#92, Ask 4 follow-up).

## 3. Combined batch, 2026-10-19: #91's prompt *and* this fix together, 20 reviews

`combined-batch-summary.json` — same shape as `production-batch-summary.json` plus a `repaired` field (the result's own `ReviewResponse.repaired`), one entry per run. Run after rebasing this branch onto fresh `main` (which carries #91's merged reworded REVIEW prompt) — confirmed by diffing `services/prompts.py` between `main` and this branch, which comes back empty, i.e. this branch's prompt IS main's post-#91 prompt, not a fork of it. This is the measurement Ask 4's follow-up asked for: the two fixes together, not inferred from two separate batches.

| | #91's prompt alone, no repair/retry (`agent_docs/2026-10-09-review-baseline/summary.json`, #91's own "after" run) | #91's prompt + this fix (combined batch) |
|---|---|---|
| Completed | 17/20 (85%) | **20/20 (100%)** |
| Failed | 3/20 (15%) | 0/20 |

Repair fired 4 times (log-verified: `grep -c "Repaired malformed LLM JSON output" sidecar-20run.log` → 4, matching the 4 runs whose result carries `"repaired": true`); retry fired once (log-verified: `grep -c "Review retry: parsing failed"` → 1), and that retry's regenerated attempt also needed a repair pass, so it both retried and repaired on its way to success. Timing: median 44.7s, range 32.8–218.1s — the 218s run is the one retry (a full second generation on top of the first); every other run falls in the 32.8–93.1s band this branch's sidecar/model pair otherwise produces. Same caveat as the 60-run batch: per-run repair/retry attribution comes from the log, not re-derived from the result content.

Run from source (`uv run python main.py --port 11439 --dev --data-dir …`) on this rebased branch, not a rebuilt PyInstaller binary — the distinguishing property for this measurement is going through the real `/review` job-queue endpoint (not a direct function call), which source and the packaged binary exercise identically; the binary itself was already confirmed to start standalone with `json-repair` bundled in the original PR evidence above. `OLLAMA_DEFAULT_MODEL=llama3.2` was set explicitly so this batch used the same model as the other two.

## How the runs were made

- Offline comparison: saved raw text from 16 failed attempts (captured during the production batch below) replayed against each parsing strategy with zero new model calls — purely a CPU-bound comparison of recovery strategies on the same inputs.
- Production batch: 60 reviews of the same fictional sample NDA, posted to `/review` on the compiled sidecar binary and polled to completion — the actual job-queue path, not a direct function call.
- Combined batch: 20 reviews of the same fictional sample NDA, posted to `/review` on this branch's sidecar (run from source, after the post-#91 rebase) and polled to completion.

All three used `llama3.2`. Harness scripts (`offline_repair_test.py`, `run_production_batch.py`, `run_combined_batch.py`) aren't committed here — this folder is the evidence they produced, not the tooling.
