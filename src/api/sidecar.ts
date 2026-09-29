// ABOUTME: Typed wrappers around sidecar HTTP endpoints routed through Tauri IPC.
// ABOUTME: Each function calls apiClient() which invokes the Rust api_call command.

import { invoke } from "@tauri-apps/api/core";

import apiClient from "./client";
import type { ClarifyRequest } from "./models/clarifyRequest";
import type { ClarifyResponse } from "./models/clarifyResponse";
import type { DraftRequest } from "./models/draftRequest";
import type { JobCreatedResponse } from "./models/jobCreatedResponse";
import type { JobResultResponse } from "./models/jobResultResponse";
import type { ResearchRequest } from "./models/researchRequest";
import type { ResearchResponse } from "./models/researchResponse";
import type { ResearchResultResponse } from "./models/researchResultResponse";
import type { ReviewRequest } from "./models/reviewRequest";
import type { TranslateRequest } from "./models/translateRequest";
import type { TranslateResponse } from "./models/translateResponse";

// ---------------------------------------------------------------------------
// Chat CRUD & messaging
// ---------------------------------------------------------------------------

export type SidecarChat = { id: string };

export type SidecarMessage = {
  id: string;
  chat_id: string;
  role: "user" | "assistant";
  content: string;
  created_at: string;
};

export type SidecarChatDetail = SidecarChat & {
  title: string | null;
  created_at: string;
  messages: SidecarMessage[];
};

export async function createChat(title?: string): Promise<SidecarChat> {
  return apiClient<SidecarChat>("/chats", {
    method: "POST",
    body: JSON.stringify(title ? { title } : {}),
  });
}

export async function getChat(chatId: string): Promise<SidecarChatDetail> {
  return apiClient<SidecarChatDetail>(`/chats/${chatId}`, { method: "GET" });
}

export async function sendMessage(
  chatId: string,
  text: string,
  model?: string,
): Promise<SidecarMessage> {
  return apiClient<SidecarMessage>(`/chats/${chatId}/message`, {
    method: "POST",
    body: JSON.stringify({ text, ...(model ? { model } : {}) }),
  });
}

/**
 * Stream a chat message via SSE, calling onDelta for each text chunk.
 * Connects directly to the sidecar (bypassing Tauri IPC) because
 * api_call reads the full response as JSON and can't handle SSE.
 */
export async function streamMessage(
  chatId: string,
  text: string,
  onDelta: (chunk: string) => void,
  model?: string,
): Promise<string> {
  const status = await invoke<{ port: number | null }>("sidecar_status");
  if (!status.port) throw new Error("Sidecar is not running");

  // Fetched separately so the secret isn't carried in routine status payloads.
  const secret = await invoke<string>("sidecar_secret");

  const url = `http://127.0.0.1:${status.port}/chats/${chatId}/stream`;
  const response = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Rumble-Secret": secret,
    },
    body: JSON.stringify({ text, ...(model ? { model } : {}) }),
  });

  if (!response.ok) {
    const detail = await response.text().catch(() => "unknown error");
    throw new Error(`Stream request failed (${response.status}): ${detail}`);
  }

  const reader = response.body?.getReader();
  if (!reader) throw new Error("No response body for SSE stream");

  const decoder = new TextDecoder();
  let fullText = "";
  let buffer = "";

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });

    // Parse SSE lines from buffer
    const lines = buffer.split("\n");
    buffer = lines.pop() ?? "";

    for (const line of lines) {
      if (!line.startsWith("data:")) continue;
      const payload = line.slice(5).trim();
      if (!payload) continue;

      try {
        const event = JSON.parse(payload) as {
          type: string;
          value: string;
        };
        if (event.type === "delta") {
          fullText += event.value;
          onDelta(event.value);
        } else if (event.type === "done") {
          fullText = event.value;
        } else if (event.type === "error") {
          throw new Error(event.value);
        }
      } catch (e) {
        if (e instanceof SyntaxError) continue; // skip non-JSON lines
        throw e;
      }
    }
  }

  return fullText;
}

// ---------------------------------------------------------------------------
// Stateless AI endpoints
// ---------------------------------------------------------------------------

export async function clarify(
  payload: ClarifyRequest,
): Promise<ClarifyResponse> {
  return apiClient<ClarifyResponse>("/clarify", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function translate(
  payload: TranslateRequest,
): Promise<TranslateResponse> {
  return apiClient<TranslateResponse>("/translate", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

// ---------------------------------------------------------------------------
// Job endpoints (draft, review, research)
// ---------------------------------------------------------------------------

export async function createDraftJob(
  payload: DraftRequest,
): Promise<JobCreatedResponse> {
  return apiClient<JobCreatedResponse>("/draft", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function createReviewJob(
  payload: ReviewRequest,
): Promise<JobCreatedResponse> {
  return apiClient<JobCreatedResponse>("/review", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function createResearchJob(
  payload: ResearchRequest,
): Promise<ResearchResponse> {
  return apiClient<ResearchResponse>("/research", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

/**
 * Poll a draft or review job result by job ID.
 * The poll_url from JobCreatedResponse is `/{type}/{id}/result`.
 */
export async function pollJobResult(
  type: "draft" | "review",
  jobId: string,
): Promise<JobResultResponse> {
  return apiClient<JobResultResponse>(`/${type}/${jobId}/result`, {
    method: "GET",
  });
}

export async function pollResearchResult(
  jobId: string,
): Promise<ResearchResultResponse> {
  return apiClient<ResearchResultResponse>(`/research/${jobId}/result`, {
    method: "GET",
  });
}

// ---------------------------------------------------------------------------
// Polling helper — waits for a job to finish
// ---------------------------------------------------------------------------

const POLL_INTERVAL_MS = 1500;
const MAX_POLL_ATTEMPTS = 120; // 3 minutes max

export async function waitForJob(
  type: "draft" | "review",
  jobId: string,
): Promise<JobResultResponse> {
  for (let i = 0; i < MAX_POLL_ATTEMPTS; i++) {
    const result = await pollJobResult(type, jobId);
    if (result.status === "completed" || result.status === "failed") {
      return result;
    }
    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
  }
  throw new Error(`Job ${jobId} did not complete within polling timeout`);
}

export async function waitForResearch(
  jobId: string,
): Promise<ResearchResultResponse> {
  for (let i = 0; i < MAX_POLL_ATTEMPTS; i++) {
    const result = await pollResearchResult(jobId);
    if (result.status === "completed" || result.status === "failed") {
      return result;
    }
    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
  }
  throw new Error(
    `Research ${jobId} did not complete within polling timeout`,
  );
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

export type HealthResponse = { status: string; mode: string };
export type ReadyResponse = {
  status: string;
  mode: string;
  error?: string;
  checks?: Record<string, string>;
};
export type ModelsResponse = {
  models: { id: string; provider: string; name: string; default: boolean }[];
  error?: string | null;
};

export async function health(): Promise<HealthResponse> {
  return apiClient<HealthResponse>("/health", { method: "GET" });
}

export async function ready(): Promise<ReadyResponse> {
  return apiClient<ReadyResponse>("/ready", { method: "GET" });
}

export async function listModels(): Promise<ModelsResponse> {
  return apiClient<ModelsResponse>("/models", { method: "GET" });
}
