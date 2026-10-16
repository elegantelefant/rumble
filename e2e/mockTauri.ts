// ABOUTME: Opt-in Tauri IPC stub for e2e — call mockTauri(page, ...) before navigating, in tests
// ABOUTME: that need host-backed views (Document Review, Research, Translation) to do anything.
import type { Page } from "@playwright/test";
import type { BackendMode } from "../src/composables/backendMode";
import type { SidecarMessage } from "../src/api/sidecar";
import type { ChatCreateResponse } from "../src/api/models/chatCreateResponse";
import type { JobCreatedResponse } from "../src/api/models/jobCreatedResponse";
import type { JobResultResponse } from "../src/api/models/jobResultResponse";
import type { ModelInfo } from "../src/api/models/modelInfo";
import type { ModelListResponse } from "../src/api/models/modelListResponse";
import type { ReadyResponse } from "../src/api/models/readyResponse";
import type { ResearchResponse } from "../src/api/models/researchResponse";
import type { ResearchResultResponse } from "../src/api/models/researchResultResponse";
import type { TranslateResponse } from "../src/api/models/translateResponse";

// The public @tauri-apps/api types don't expose this — it's the internal bridge object the real
// Tauri runtime installs, and the one mocks (real or hand-rolled) must stand in for.
declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: (cmd: string, args?: Record<string, unknown>, options?: unknown) => Promise<unknown>;
      transformCallback: (callback: (data: unknown) => void, once?: boolean) => number;
    };
  }
}

const DEFAULT_MODELS: ModelInfo[] = [
  { id: "elefant-local", provider: "Local", name: "Ollama · Elefant Legal Blend", default: true },
];

export type MockTauriOptions = {
  /** What get_backend_mode resolves with. Defaults to "ollama". */
  backendMode?: BackendMode;
  /** What /models resolves with. [] reproduces "the sidecar has no models" (no host, unreachable Ollama, etc). */
  models?: ModelInfo[];
  /** What extract_document resolves with, for any uploaded file. */
  extractedText?: string;
  /** The review job's summary, once "completed". */
  reviewSummary?: string;
  /** The research job's result text, once "completed". */
  researchResult?: string;
  /** The draft job's generated text, once "completed". */
  draftResult?: string;
  /** translate()'s translated_text. */
  translatedText?: string;
  /** The assistant's reply streamed back by stream_message (one delta, then done). */
  streamReply?: string;
  /**
   * false reproduces "no host at all" for the keychain commands: both get_api_key and
   * delete_api_key reject, so removeSecret() can't confirm the key is gone and takes its
   * honest-failure path (error toast, secret stays listed) instead of removing it.
   */
  deleteApiKey?: boolean;
};

// What mockTauri actually hands into the page — every optional field resolved to its default, so
// the in-page callback below never has to repeat the ?? fallback.
type ResolvedMockTauriOptions = {
  backendMode: BackendMode;
  models: ModelInfo[];
  extractedText: string;
  reviewSummary: string;
  researchResult: string;
  draftResult: string;
  translatedText?: string;
  streamReply?: string;
  deleteApiKey?: boolean;
};

/**
 * Stubs window.__TAURI_INTERNALS__.invoke before the next navigation, so views that call Tauri
 * commands (DocumentReviewView, ResearchView, TranslationView, SettingsView) can run against plain
 * Chromium. Call this, then (re-)navigate — addInitScript only takes effect on navigations that
 * start after it's registered.
 *
 * Only the commands these views actually call are implemented; anything else rejects loudly so a
 * newly-reached, unmocked command fails a test instead of hanging.
 *
 * Canned responses are typed against the generated sidecar contract (src/api/models, from
 * sidecar-openapi.json) and the hand-written client's own types (src/api/sidecar.ts) — a shape
 * orval regenerates differently, or a client type that moves, fails this file's typecheck
 * (e2e/tsconfig.json, run by `pnpm build`) instead of failing silently at test time.
 */
export async function mockTauri(page: Page, options: MockTauriOptions = {}): Promise<void> {
  const resolved: ResolvedMockTauriOptions = {
    backendMode: options.backendMode ?? "ollama",
    models: options.models ?? DEFAULT_MODELS,
    extractedText: options.extractedText ?? "This is a mocked extracted document body for e2e.",
    reviewSummary: options.reviewSummary ?? "Mocked review summary: no material risks found.",
    researchResult: options.researchResult ?? "Mocked research result.",
    draftResult: options.draftResult ?? "Mocked draft text.",
    translatedText: options.translatedText,
    streamReply: options.streamReply,
    deleteApiKey: options.deleteApiKey,
  };

  await page.addInitScript((opts: ResolvedMockTauriOptions) => {
    function randomId(prefix: string): string {
      return `${prefix}-${Math.random().toString(36).slice(2)}`;
    }

    async function handleApiCall(args: { method: string; path: string; body: unknown }): Promise<unknown> {
      const { method, path, body } = args;
      const json: Record<string, unknown> = typeof body === "string" ? JSON.parse(body) : {};

      if (path === "/ready") {
        const res: ReadyResponse = { status: "ready" };
        return res;
      }
      if (path === "/models") {
        const res: ModelListResponse = { models: opts.models };
        return res;
      }
      if (path === "/review" && method === "POST") {
        const res: JobCreatedResponse = { job_id: randomId("review-job"), poll_url: "" };
        return res;
      }
      if (/^\/review\/[^/]+\/result$/.test(path)) {
        const res: JobResultResponse = {
          id: randomId("review-job"),
          status: "completed",
          result: { summary: opts.reviewSummary, issues: [] },
        };
        return res;
      }
      if (path === "/chats" && method === "POST") {
        const res: ChatCreateResponse = { id: randomId("chat") };
        return res;
      }
      if (/^\/chats\/[^/]+\/message$/.test(path)) {
        const res: SidecarMessage = {
          id: randomId("msg"),
          chat_id: "mock-chat",
          role: "assistant",
          content: "",
          created_at: new Date().toISOString(),
        };
        return res;
      }
      if (path === "/research" && method === "POST") {
        const res: ResearchResponse = { report_id: randomId("report"), status: "queued" };
        return res;
      }
      if (/^\/research\/[^/]+\/result$/.test(path)) {
        const res: ResearchResultResponse = {
          report_id: randomId("report"),
          status: "completed",
          result: opts.researchResult,
          sources: [],
        };
        return res;
      }
      if (path === "/draft" && method === "POST") {
        const res: JobCreatedResponse = { job_id: randomId("draft-job"), poll_url: "" };
        return res;
      }
      if (/^\/draft\/[^/]+\/result$/.test(path)) {
        const res: JobResultResponse = {
          id: randomId("draft-job"),
          status: "completed",
          result: { draft: opts.draftResult, warnings: [] },
        };
        return res;
      }
      if (path === "/translate" && method === "POST") {
        const res: TranslateResponse = {
          translated_text: opts.translatedText ?? `[translated] ${String(json.text ?? "")}`,
        };
        return res;
      }
      throw `e2e mockTauri: unmocked sidecar path ${method} ${path}`;
    }

    window.__TAURI_INTERNALS__ = {
      // Channel's constructor always calls this (core.js); its registration protocol is never
      // exercised here because stream_message below calls onEvent.onmessage() directly.
      transformCallback: () => Math.floor(Math.random() * 1e9),
      invoke: async (cmd: string, rawArgs?: Record<string, unknown>) => {
        const args = rawArgs ?? {};
        switch (cmd) {
          case "get_backend_mode":
            return opts.backendMode;
          case "api_call":
            return handleApiCall(args as { method: string; path: string; body: unknown });
          case "extract_document":
            return opts.extractedText;
          case "stream_message": {
            const text = opts.streamReply ?? "Mocked assistant reply.";
            const onEvent = args.onEvent as { onmessage: (e: { type: string; value: string }) => void };
            // The real host streams these after invoke() resolves, not before — deferring with
            // setTimeout keeps this mock from hiding ordering bugs in the view.
            setTimeout(() => onEvent.onmessage({ type: "delta", value: text }), 0);
            setTimeout(() => onEvent.onmessage({ type: "done", value: text }), 0);
            return null;
          }
          case "load_settings":
            return null;
          case "save_settings":
            return null;
          case "get_api_key":
            // Both this and delete_api_key need to be unavailable to reproduce the real no-host
            // failure: removeSecret() falls back to this call when delete_api_key rejects, and
            // treats a resolved `null` as "confirmed gone" — so delete_api_key alone rejecting
            // isn't enough to reproduce the honest-failure path; this one must reject too.
            if (opts.deleteApiKey === false) throw "e2e mockTauri: get_api_key not mocked";
            return null;
          case "store_api_key":
            return null;
          case "delete_api_key":
            if (opts.deleteApiKey === false) throw "e2e mockTauri: delete_api_key not mocked";
            return null;
          default:
            throw `e2e mockTauri: unmocked Tauri command ${cmd}`;
        }
      },
    };
  }, resolved);
}
