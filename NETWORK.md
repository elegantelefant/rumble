# Network

Every way Elefant Rumble, or something it depends on, can open a connection off this machine —
as `main` behaves today. Each row cites the code it was read from; anything not yet true is marked
**pending** with its issue. For how rumble protects the local API, see [SECURITY.md](SECURITY.md).

The claim this document supports is **"offline after setup"** — never "offline". Installing Ollama
and pulling a model need the network; after that, chat, draft and review are meant to run with
outbound traffic blocked. [Proving it](#proving-it) says how to check that on your own machine.

## Modes, as the code has them

The host has three modes — `ollama` (shown as *local*), `byok`, `premium` — and starts in `ollama`.
Nothing in the UI calls `set_backend_mode` yet, so a packaged build stays in `ollama` mode
(pending [#40]). Separately, the sidecar picks its provider from its own environment, not from the
host's mode: `BYOK_API_KEY` present → OpenAI, absent → Ollama (pending [#52], fix in PR [#66]). The
host never passes `BYOK_API_KEY` ([#35]), so the key can only arrive by inheritance from the
environment rumble was launched in.
<!-- src-tauri/src/lib.rs:63-67,342-358,644; src-tauri/sidecar/services/llm.py:61-68; src-tauri/sidecar/routes/health.py:14-18; src-tauri/src/lib.rs:411-425 -->

## Egress initiators

| Initiator | Mode | Trigger | Destination · payload | Off-switch |
|---|---|---|---|---|
| **Host router** (`resolve_url`)<!-- src-tauri/src/lib.rs:23,28-57,87-106,192-198,235-258 --> | `premium`: every path. `ollama`/`byok`: any path matching `CLOUD_ONLY_PREFIXES` (`/billing`, `/auth`, `/documents`, `/search`, … 28 unversioned prefixes) | A webview `api_call`. No current call site hits a cloud prefix (`src/api/sidecar.ts`), and `premium` is unreachable from the UI ([#40]) | `https://api.elefant.com` · the request body as the UI sent it, plus the keychain `auth_token` as a Bearer header if one is stored. That host has no working TLS endpoint today ([#60]) | None in code. **Pending [#41]**: `ollama`/`byok` return only 127.0.0.1 sidecar URLs and `Err` for everything else |
| **Sidecar → Ollama**<!-- src-tauri/sidecar/services/llm.py:18-20,25-47,66-68; src-tauri/sidecar/routes/health.py:11,35-36,59-60 --> | Whenever `BYOK_API_KEY` is absent (the packaged default) | `/health`, `/models`, model resolution (`GET /api/tags`); chat, draft, review, research, clarify, translate (chat completions) | `OLLAMA_BASE_URL`, default `http://localhost:11434` — loopback · prompts including document text. Leaves the machine if `OLLAMA_BASE_URL` in the inherited environment points elsewhere (unchecked), or if the model is an Ollama cloud model (below) | Leave `OLLAMA_BASE_URL` unset. **Pending [#53]** (PR [#66]): refuse non-loopback `OLLAMA_BASE_URL` and cloud models in local mode |
| **Sidecar → BYOK provider**<!-- src-tauri/sidecar/services/llm.py:61-64 --> | Whenever `BYOK_API_KEY` is in the sidecar's environment — **regardless of host mode** ([#52]) | The same LLM routes as above | OpenAI's API (the `OpenAIProvider` default) · prompts including document text, and the key. Keys saved in Settings go to the keychain and are never read by the sidecar ([#35]) | Don't launch rumble from an environment that sets `BYOK_API_KEY`. **Pending [#52]** (PR [#66]): provider follows the mode |
| **Webview** (CSP)<!-- src-tauri/tauri.conf.json:21; src/api/sidecar.ts:72-86 --> | All modes — the CSP is static | Page `fetch`. The one direct fetch today is chat streaming to `http://127.0.0.1:{port}` ([#55]) | `connect-src 'self' http://127.0.0.1 https://api.elefant.com` — **the cloud host is allowed in every mode** (flagged, [#41]-adjacent; [plan] §2 L3: tighten per mode). No webview code fetches it today. Note: a CSP host source without a port matches only the scheme's default port, so `http://127.0.0.1` may not cover the sidecar's random port either — unverified, [#55]-adjacent | Edit `csp` in `tauri.conf.json`. **Pending**: per-mode CSP |
| **`tauri-plugin-opener`**<!-- src-tauri/src/lib.rs:640; src-tauri/capabilities/default.json:8 --> | All | None today: the plugin is registered and granted `opener:default`, but nothing in `src/` or `lib.rs` calls it | Would hand a URL to the system browser; the browser, not rumble, makes the request | Remove the plugin and its capability |
| **Ollama install**<!-- outside rumble; instructed at src/views/SetupView.vue:103 --> | Setup | The user installs Ollama | ollama.com · the installer download | Install once, then block ([Offline model install](#offline-model-install)) |
| **`ollama pull`**<!-- outside rumble; instructed at src/views/SetupView.vue:158 --> | Setup | The user pulls a model | registry.ollama.ai (and its download CDN) · model name out, weights in | [Offline model install](#offline-model-install) |
| **Ollama's updater**<!-- outside rumble --> | Any — Ollama runs independently of rumble | The Ollama desktop app checks ollama.com for updates | ollama.com · version check and download | Ollama's, not rumble's: block at the firewall, or run a package-managed `ollama serve` without the desktop app |
| **Ollama cloud models**<!-- src-tauri/sidecar/services/llm.py:40-46; see #53 --> | `ollama` | A model whose name ends `:cloud`/`-cloud` is selected — or picked as the "first pulled model" fallback | ollama.com · prompts including document text, **run remotely while the UI says local** | `OLLAMA_NO_CLOUD=1` on the Ollama daemon (per [#53]). **Pending [#53]** (PR [#66]): rumble filters and refuses them |
| **macOS Gatekeeper**<!-- .github/workflows/build.yml (no signing step) --> | First launch | **Forward-looking**: builds are currently unsigned and unnotarized, so there is nothing to verify. A signed, notarized build may be checked with Apple on first launch | Apple · the app's code signature identity | Launch once before blocking egress (setup). **Pending**: signing, [plan] §3 R4 |
| **Auto-updater**<!-- src-tauri/Cargo.toml (no tauri-plugin-updater) --> | — | **Not shipped**: no updater plugin is in the build | — | **Pending**: [plan] §3 R4 / Phase 2. Updates are a manual download today |

The OS and its own services (DNS, time sync, Windows' WebView2 runtime updates, telemetry) sit
outside rumble and outside this table; a whole-machine block covers them too.

### Loopback only (not egress)

- Host → sidecar on `127.0.0.1:{random port}`, with the shared secret.<!-- src-tauri/src/lib.rs:364-374,376-396,398-425 -->
- Sidecar binds `127.0.0.1` only.<!-- src-tauri/sidecar/main.py:52 -->
- Settings' sync "Test connection" is a mock and makes no request.<!-- src/views/SettingsView.vue:204-223; src/modules/backend/backendClient.ts:114-120 -->

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
[#40]: https://github.com/elegantelefant/rumble/issues/40
[#41]: https://github.com/elegantelefant/rumble/issues/41
[#52]: https://github.com/elegantelefant/rumble/issues/52
[#53]: https://github.com/elegantelefant/rumble/issues/53
[#55]: https://github.com/elegantelefant/rumble/issues/55
[#60]: https://github.com/elegantelefant/rumble/issues/60
[#66]: https://github.com/elegantelefant/rumble/pull/66
[plan]: agent_docs/2026-09-22-credible-delightful-local-first-plan.md
