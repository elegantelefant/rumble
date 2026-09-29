# Network

Every way Elefant Rumble, or something it depends on, can open a connection off this machine —
as `main` behaves today. Each row cites the code it was read from; anything not yet true is marked
**pending** with its issue. For how rumble protects the local API, see [SECURITY.md](SECURITY.md).

The claim this document supports is **"offline after setup"** — never "offline". System and
environment proxies are ignored for all host traffic (PR #76, `no_proxy()`) and by the sidecar's
clients (PR #66, `trust_env=False`) — loopback traffic cannot transit a configured proxy
([#73], fixed). The exception is BYOK provider traffic, which deliberately honours proxy env
vars and `SSL_CERT_FILE` (documented in `llm.py`). Installing Ollama
and pulling a model need the network; after that, chat, draft and review are meant to run with
outbound traffic blocked. [Proving it](#proving-it) says how to check that on your own machine.

## Modes, as the code has them

The host has three modes — `ollama` (shown as *local*), `byok`, `premium` — and starts in `ollama` on
every launch. Settings → Providers switches it through `set_backend_mode`; BYOK is selectable only
in dev builds and Premium is shown as unavailable, so a packaged build stays in `ollama` mode. The
sidecar takes its mode only from its spawn environment (`RUMBLE_BACKEND_MODE`, failing closed to
`ollama`, PR [#66]), and a mode switch respawns it. The host sets `BYOK_API_KEY` to the keychain's
`byok_openai` key in `byok` mode only; in every other mode it sets it empty, overwriting any key
inherited from the environment rumble was launched in ([#35]).
<!-- src-tauri/src/lib.rs:477-534,570-609; src-tauri/sidecar/services/mode.py:28-40; src-tauri/sidecar/services/llm.py:77-91; src/views/SettingsView.vue -->

## Egress initiators

| Initiator | Mode | Trigger | Destination · payload | Off-switch |
|---|---|---|---|---|
| **Host router** (`resolve_url`)<!-- src-tauri/src/lib.rs:28-29,58-77,238-268 --> | None. `ollama`/`byok`: every path goes to the sidecar on `127.0.0.1`, except the cloud contract (`/api/v1`, `/api/v1/…`), which fails with "This feature requires Elefant Premium." `premium`: every path fails the same way until premium routing is built against the [payload contract] | A webview `api_call` | Loopback only: `resolve_url` never returns a non-loopback URL. The keychain `auth_token` is read and attached as a Bearer header only for a non-sidecar URL, of which there are none today ([#72], fixed) | Nothing — enforced in code. **Pending**: premium routing, gated on the [payload contract] |
| **Sidecar → Ollama**<!-- src-tauri/sidecar/services/llm.py:77-91; src-tauri/sidecar/routes/health.py:21-38,41-68; src-tauri/src/lib.rs:513-534 --> | `ollama`, the packaged default (and `premium`, whose sidecar runs as `ollama`) | `/ready`, `/models`, model resolution (`GET /api/tags`); chat, draft, review, research, clarify, translate (chat completions) — `/health` itself does not touch Ollama | `OLLAMA_BASE_URL`, default `http://localhost:11434` — loopback · prompts including document text. Non-loopback `OLLAMA_BASE_URL` is refused and cloud/remote models are rejected per request ([#53], fixed by PR [#66]) | Nothing — enforced in code; `/ready` reports the refusal reason |
| **Sidecar → BYOK provider**<!-- src-tauri/sidecar/services/llm.py:77-84; src-tauri/src/lib.rs:513-534,570-578 --> | `byok` only — the sidecar branches on `RUMBLE_BACKEND_MODE`, never on whether a key is present (PR [#66]) | The same LLM routes as above | OpenAI's API, pinned by explicit base_url ([#74], fixed by PR [#66]) · prompts including document text, and the key. The OpenAI key saved in Settings reaches the sidecar only in `byok` mode ([#35]) | Stay in local mode. BYOK is not selectable in packaged builds (`BYOK_SELECTABLE` in `src/views/SettingsView.vue`) |
| **Webview** (CSP)<!-- src-tauri/tauri.conf.json:21; src/api/sidecar.ts:72-86 --> | All modes — the CSP is static | Page `fetch`. The one direct fetch today is chat streaming to `http://127.0.0.1:{port}` ([#55]) | `connect-src 'self' http://127.0.0.1 https://api.elefant.com` — **the cloud host is allowed in every mode** (flagged, [#41]-adjacent; [plan] §2 L3: tighten per mode). No webview code fetches it today. Note: a CSP host source without a port matches only the scheme's default port, so `http://127.0.0.1` may not cover the sidecar's random port either — unverified, [#55]-adjacent | Edit `csp` in `tauri.conf.json`. **Pending**: per-mode CSP |
| **`tauri-plugin-opener`**<!-- src-tauri/src/lib.rs:640; src-tauri/capabilities/default.json:8 --> | All | None today: the plugin is registered and granted `opener:default`, but nothing in `src/` or `lib.rs` calls it | Would hand a URL to the system browser; the browser, not rumble, makes the request | Remove the plugin and its capability |
| **Ollama install**<!-- outside rumble; instructed at src/views/SetupView.vue:103 --> | Setup | The user installs Ollama | ollama.com · the installer download | Install once, then block ([Offline model install](#offline-model-install)) |
| **`ollama pull`**<!-- outside rumble; instructed at src/views/SetupView.vue:158 --> | Setup | The user pulls a model | registry.ollama.ai (and its download CDN) · model name out, weights in | [Offline model install](#offline-model-install) |
| **Ollama's updater**<!-- outside rumble --> | Any — Ollama runs independently of rumble | The Ollama desktop app checks ollama.com for updates | ollama.com · version check and download | Ollama's, not rumble's: block at the firewall, or run a package-managed `ollama serve` without the desktop app |
| **Ollama cloud models**<!-- src-tauri/sidecar/services/llm.py:40-46; see #53 --> | `ollama` | A model whose name ends `:cloud`/`-cloud` is selected — or picked as the "first pulled model" fallback | ollama.com · prompts including document text, **run remotely while the UI says local** | Refused in code per request — `remote_host` flag + normalized name check ([#53], fixed by PR [#66]); `OLLAMA_NO_CLOUD=1` on the daemon as defence in depth |
| **macOS Gatekeeper**<!-- .github/workflows/build.yml (no signing step) --> | First launch | **Forward-looking**: builds are currently unsigned and unnotarized, so there is nothing to verify. A signed, notarized build may be checked with Apple on first launch | Apple · the app's code signature identity | Launch once before blocking egress (setup). **Pending**: signing, [plan] §3 R4 |
| **Auto-updater**<!-- src-tauri/Cargo.toml (no tauri-plugin-updater) --> | — | **Not shipped**: no updater plugin is in the build | — | **Pending**: [plan] §3 R4 / Phase 2. Updates are a manual download today |

The OS and its own services (DNS, time sync, Windows' WebView2 runtime updates, telemetry) sit
outside rumble and outside this table; a whole-machine block covers them too.

### Loopback only (not egress)

- Host → sidecar on `127.0.0.1:{random port}`, with the shared secret and never the premium Bearer token ([#72], fixed).<!-- src-tauri/src/lib.rs:238-268 -->
- Sidecar binds `127.0.0.1` only.<!-- src-tauri/sidecar/main.py:52 -->
- Settings' sync "Test connection" is a mock and makes no request.<!-- src/views/SettingsView.vue:264-283; src/modules/backend/backendClient.ts:114-120 -->

## Offline model install

For firms whose machines never reach the internet. Both routes are Ollama's, not rumble's; check
them against your Ollama version.

1. **Copy a model store.** On a connected machine, `ollama pull <model>`, then copy
   `~/.ollama/models/` (both `blobs/` and `manifests/`) to the same path on the offline machine and
   restart Ollama. `ollama list` should show the model.
2. **Build from a checksummed GGUF.** Obtain the `.gguf` file through your normal software intake,
   verify it (`shasum -a 256 model.gguf` against the publisher's hash), then on the offline machine:

   ```sh
   printf 'FROM ./model.gguf\n' > Modelfile
   ollama create my-model -f Modelfile
   ```

Set `OLLAMA_NO_CLOUD=1` on the Ollama daemon either way.

## Proving it

"Provable" here means **deny-by-default**: block all outbound traffic from the whole machine, then
use the packaged app for chat, draft and review. If everything works, nothing needed the network.
Watching connections cannot prove absence; a block can.

[`scripts/egress-smoke.sh`](scripts/egress-smoke.sh) prints the macOS `pf` procedure for the block
(it needs `sudo` and is never run for you), and has an unprivileged `observe` mode that samples a
running app's and Ollama's sockets and reports any non-loopback peer. Sampling can miss
short-lived connections; the block is the real proof. On other platforms, an offline VM with no
network adapter is the equivalent.

[#35]: https://github.com/elegantelefant/rumble/issues/35
[#72]: https://github.com/elegantelefant/rumble/issues/72
[#73]: https://github.com/elegantelefant/rumble/issues/73
[#74]: https://github.com/elegantelefant/rumble/issues/74
[#40]: https://github.com/elegantelefant/rumble/issues/40
[#41]: https://github.com/elegantelefant/rumble/issues/41
[#52]: https://github.com/elegantelefant/rumble/issues/52
[#53]: https://github.com/elegantelefant/rumble/issues/53
[#55]: https://github.com/elegantelefant/rumble/issues/55
[#60]: https://github.com/elegantelefant/rumble/issues/60
[#66]: https://github.com/elegantelefant/rumble/pull/66
[plan]: agent_docs/2026-09-22-credible-delightful-local-first-plan.md
[payload contract]: agent_docs/2026-09-22-premium-payload-contract.md
