// ABOUTME: Typed wrappers around sidecar HTTP endpoints routed through Tauri IPC.
// ABOUTME: Each function calls apiClient() which invokes the Rust api_call command.

import { Channel, invoke } from "@tauri-apps/api/core";

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

/** One event of a chat stream, as the host's stream_message command sends it. */
type StreamEvent = { type: "delta" | "done" | "error"; value: string };

/**
 * Stream a chat reply, calling onDelta for each text chunk in order, and
 * resolve with the complete text.
 *
 * The host's stream_message command makes the sidecar SSE request itself,
 * attaching the shared secret host-side, and relays events over a Tauri IPC
 * channel — so the secret never reaches webview JavaScript and there is no
 * cross-origin fetch (#55). The promise settles on the channel's terminal
 * event, never on invoke resolving: Tauri orders channel messages among
 * themselves, not against the command's own response. Rejects with a string,
 * as invoke does.
 */
export function streamMessage(
  chatId: string,
  text: string,
  onDelta: (chunk: string) => void,
  model?: string,
): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const onEvent = new Channel<StreamEvent>();
    onEvent.onmessage = (event) => {
      if (event.type === "delta") onDelta(event.value);
      else if (event.type === "done") resolve(event.value);
      else if (event.type === "error") reject(event.value);
    };
    invoke("stream_message", { chatId, text, model: model ?? null, onEvent }).catch(reject);
  });
}

// ---------------------------------------------------------------------------
// Document extraction
// ---------------------------------------------------------------------------

const EXTRACT_FILENAME_HEADER = "X-Rumble-Filename";

// Must equal MAX_UPLOAD_MB in src-tauri/sidecar/routes/extract.py, which
// rejects larger uploads with a 413; a sidecar test holds the two in step.
export const MAX_UPLOAD_MB = 50;
const MAX_UPLOAD_BYTES = MAX_UPLOAD_MB * 1024 * 1024;

/**
 * Extract text from an uploaded file via the host's extract_document command,
 * which posts the bytes to the sidecar's /extract endpoint as multipart form
 * data. Supports PDF, DOCX, and plain text; rejects with the sidecar's
 * human-readable message for anything else (scanned PDF, unsupported type).
 *
 * The filename travels in a header, and header values must be Latin-1, so a
 * non-ASCII name (e.g. "合同.pdf") is percent-encoded here; the host decodes it.
 *
 * A file over the upload limit is refused before its bytes are read, rejecting
 * with a string as the host's commands do.
 */
export async function extractDocument(file: File): Promise<string> {
  if (file.size > MAX_UPLOAD_BYTES) {
    throw `${file.name} is larger than the ${MAX_UPLOAD_MB} MB upload limit.`;
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  return invoke<string>("extract_document", bytes, {
    headers: { [EXTRACT_FILENAME_HEADER]: encodeURIComponent(file.name) },
  });
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
