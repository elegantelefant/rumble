// ABOUTME: Installs a stand-in Tauri host before the app loads, answering invoke() at the IPC
// ABOUTME: boundary, so e2e drives the real views and sidecar client against canned sidecar replies.
import type { Page } from "@playwright/test";

export type FakeHost = {
  /** api_call replies keyed "METHOD /path". A missing route rejects with a string, as Tauri does. */
  routes?: Record<string, unknown>;
  /** What extract_document returns for any file. */
  extractedText?: string;
  /** Chunks stream_message delivers, in order, before it reports the joined text as done. */
  streamedReply?: string[];
};

export const LOCAL_MODEL = { id: "llama3.2", provider: "ollama", name: "llama3.2", default: true };

const READY_ROUTES = {
  "GET /ready": { status: "ready", mode: "ollama" },
  "GET /health": { status: "ok", mode: "ollama" },
  "GET /models": { models: [LOCAL_MODEL] },
};

export function installFakeHost(page: Page, host: FakeHost = {}) {
  const config = { ...host, routes: { ...READY_ROUTES, ...host.routes } };
  return page.addInitScript((host: FakeHost & { routes: Record<string, unknown> }) => {
    const callbacks = new Map<number, (message: unknown) => void>();
    let nextCallback = 1;
    const commands: Record<string, (args: Record<string, unknown>) => unknown> = {
      get_backend_mode: () => "ollama",
      sidecar_status: () => ({ running: true, state: "running", error: null }),
      load_settings: () => null,
      data_dir: () => "/fake/rumble-data",
      get_api_key: () => null,
      "plugin:app|version": () => "0.0.0-e2e",
      api_call: ({ method, path }) => {
        const key = `${method} ${path}`;
        if (!(key in host.routes)) throw `fake host has no route for ${key}`;
        return host.routes[key];
      },
      extract_document: () => host.extractedText ?? "",
      stream_message: ({ onEvent }) => {
        const send = callbacks.get((onEvent as { id: number }).id)!;
        const chunks = host.streamedReply ?? [];
        chunks.forEach((value, index) => send({ index, message: { type: "delta", value } }));
        send({ index: chunks.length, message: { type: "done", value: chunks.join("") } });
        return null;
      },
    };
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      transformCallback(callback: (message: unknown) => void) {
        const id = nextCallback++;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id: number) {
        callbacks.delete(id);
      },
      async invoke(cmd: string, args: Record<string, unknown>) {
        const command = commands[cmd];
        if (!command) throw `fake host has no command ${cmd}`;
        return command(args ?? {});
      },
      convertFileSrc: (path: string) => path,
      metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    };
  }, config);
}

export const RESEARCH_ANSWER = "Personal data may be kept only as long as its purpose requires.";

export function installResearchHost(page: Page) {
  return installFakeHost(page, {
    routes: {
      "POST /research": { report_id: "research-1", status: "queued" },
      "GET /research/research-1/result": { report_id: "research-1", status: "completed", result: RESEARCH_ANSWER, sources: [] },
      "POST /translate": { translated_text: "Ceci est un test" },
    },
  });
}
