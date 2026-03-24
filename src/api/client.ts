// ABOUTME: Tauri IPC bridge mutator for Orval-generated API composables.
// ABOUTME: Routes all API calls through invoke('api_call') instead of HTTP fetch.
import { invoke } from "@tauri-apps/api/core";

export const apiClient = async <T>(
  url: string,
  config: RequestInit,
): Promise<T> => {
  const method = config.method ?? "GET";

  // Parse body from RequestInit if present
  let body: unknown = null;
  if (config.body) {
    if (typeof config.body === "string") {
      try {
        body = JSON.parse(config.body);
      } catch {
        body = config.body;
      }
    } else {
      body = config.body;
    }
  }

  // Extract query params from URL if present
  let path = url;
  let params: Record<string, string> | null = null;
  const qIndex = url.indexOf("?");
  if (qIndex !== -1) {
    path = url.slice(0, qIndex);
    const searchParams = new URLSearchParams(url.slice(qIndex + 1));
    params = Object.fromEntries(searchParams.entries());
  }

  return invoke<T>("api_call", {
    method,
    path,
    body: body ?? null,
    params: params ?? null,
  });
};

export default apiClient;
