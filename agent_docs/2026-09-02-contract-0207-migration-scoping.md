# Contract 0.1.0 → 0.207.0 migration scoping (second half of #33)

Method: structural diff of the endpoints rumble actually calls (from `src/api/sidecar.ts`
and `src-tauri/sidecar/routes/*`) between the vendored `openapi.json` (elefant-api 0.1.0)
and birepo `origin/dev` `backend/openapi.json` (0.207.0), with `$ref`s resolved and
descriptions stripped. Prefix note: 0.4.0+ moved everything except `/health` and `/ready`
under `/api/v1`.

## Verdict

This is a migration project, not a spec refresh. A wholesale swap of `openapi.json` +
regeneration would delete models the sidecar implements and change wire formats it emits.

## Endpoint inventory (rumble-relevant)

**Removed in 0.207.0 — no replacement exists:**

| path | rumble impact |
|---|---|
| `/translate`, `/translate/document` | TranslationView's entire backend surface |
| `/summarise/chat`, `/summarise/document`, `/summarise/search` | implemented in sidecar `routes/ai.py` |
| `/research/{id}/result` | polled by `waitForResearch` |
| `/chats/{id}/briefcase-context` (+`/refresh`) | not yet used by rumble |

The only `summar*` survivors in 0.207.0 are search/briefcase summaries — different
features, not replacements.

**Prefix-only (safe):** `/health`, `/ready`, `/draft/{id}/result`, `/review/{id}/result`.

**Shape changed (15):** `/chats` (all sub-routes incl. `/message`, `/stream`, `/messages`),
`/chat_title/generate`, `/clarify`, `/draft`, `/jobs` (all, plus new `PATCH /jobs/{job_id}`),
`/models`, `/research`, `/research/{id}`, `/review`.

## Nature of the shape changes (sampled)

- **Casing convention flipped to camelCase** on the wire: `briefcase_doc_ids` →
  `briefcaseDocIds`, `briefcase_facets` → `briefcaseFacets`. Every sidecar Pydantic model
  and handler that serializes snake_case is affected. (The closed PR #1 anticipated this:
  its `generate:models` added `--snake-case-field --no-alias`.)
- **Semantic renames**: `ChatMessageRequest.learning_mode: bool` → `learnLevel` (typed enum),
  plus new optional `deepResearch`, `contentBlocks`, `fileIds`, `jurisdiction`.
- **New required fields**: `ModelInfo.displayName` (sidecar `/models` must emit it).
- **Auth surface changed**: `X-Org-Id` header parameters dropped from operations.

## The real decision before any code

The vendored spec serves double duty: it types the **local sidecar** (which rumble fully
controls) and the **cloud API** (which moved on without us). Two ways forward:

1. **Track the full contract** (current design): sidecar stays a drop-in shape-compatible
   subset of elefant-api. Cost now: camelCase migration of the whole sidecar, re-adding
   `/translate`+`/summarise` is impossible (cloud dropped them) so those views either go
   sidecar-only off-contract or get cut. Benefit: premium mode is a pure URL swap.
2. **Split contracts**: a small rumble-sidecar spec (owned here, keeps translate/summarise,
   snake_case, stable) + consume `@elefant/api-contract` only for the cloud calls premium
   mode actually makes. Cost: two contracts; benefit: local features stop being hostage to
   cloud product decisions, and the 0.207 migration shrinks to the premium call sites.

Recommendation: option 2 fits how the product actually behaves (TranslationView and
summarise exist only locally now), but it changes the hybrid-architecture story in the
README — decide before scheduling the work.

## Phasing (whichever option)

1. Decide contract ownership (above).
2. `/api/v1` prefix + `/models` `displayName` (small, unblocks nothing but reduces drift).
3. camelCase wire migration (or scope it to premium call sites under option 2).
4. Reconcile removed surfaces (translate/summarise/research-result) per the decision.
5. Refresh vendored spec(s), regenerate, let the drift gate (#37) hold the line.

## Decision (2026-09-03)

Purpose settles ownership: **the elefant-api contract describes the subscription-gated
cloud API — the thing the funnel sells into. The local sidecar is the simple, free tier
and owns its own shapes.**

Consequences:

- The vendored `openapi.json` here is the **rumble sidecar's own spec** from now on. It is
  frozen at the current shapes (0.1.0 lineage), drift-gated by CI, and deliberately does
  NOT track birepo. Translate/summarise stay — they're local features.
- No camelCase migration, no endpoint removals, no wholesale regeneration.
- When a premium surface makes real cloud calls (briefcases, billing, cloud review), that
  call site consumes `@elefant/api-contract` (birepo publishes it) at whatever version is
  current — scoped to those calls only.
- README's "hybrid cloud/sidecar, one contract" story overstates interchangeability now;
  soften it when premium work starts.
