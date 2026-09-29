# Premium payload contract (L0) — owner decision 2026-09-22 (D1, adopted)

The privacy wedge — questions of law go to the cloud, documents stay local — is a
routing-level guarantee, not marketing copy. This contract binds every premium call
site, present and future. It exists because `resolve_url` used to send *everything*
(including /review and /draft bodies) to the cloud in premium mode, which contradicts
the wedge outright. Since #41, `resolve_url` refuses every premium path ("requires
Elefant Premium") until premium routing is built against this contract.

## The rule

Premium mode may transmit to api.elefant.com ONLY:
1. **User-previewed excerpts** — text the user has explicitly selected or been shown
   verbatim in a consent surface before sending ("send these N clauses?").
2. **Questions of law** — model- or user-formulated queries, shown before sending.
3. **Account/entitlement traffic** — auth tokens, subscription state; never document
   content.

Premium mode may NEVER transmit:
- The full document body, silently or as a side effect of a local action.
- Chats, drafts, or review results produced in local/byok mode.
- Anything while the mode is ollama or byok (enforced by the two-branch routing rule
  in #41 and the sidecar mode enforcement in #52).

## Enforcement points

- Call-site review rule: any PR adding a premium call names the payload class (1-3)
  in its description; anything outside the table is a contract change — ask first.
- UI rule: classes 1-2 always render the exact outbound payload before send (the
  consent surface doubles as the wedge made visible — see W7 in the program plan).
- The confidentiality pill's premium copy references this contract.

Program context: agent_docs/2026-09-22-credible-delightful-local-first-plan.md (§2 L0).
