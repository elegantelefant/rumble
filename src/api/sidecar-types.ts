// ABOUTME: snake_case request/response types for the local sidecar's own routes.
// ABOUTME: The sidecar serves snake_case wire, distinct from the camelCase cloud
// ABOUTME: orval models, so its FE client (sidecar.ts) owns these types here.

export interface ClarifyRequest {
  ask: string;
  context?: Record<string, unknown>;
  document_type?: string;
  drafting_style?: string;
}

export interface ClarifyResponse {
  clarified_ask: string;
  questions?: string[];
}

export interface DraftRequest {
  prompt: string;
  context?: Record<string, unknown>;
  document_type?: string;
  drafting_style?: string;
  parties?: Record<string, unknown>[];
  document_terms?: Record<string, unknown>;
  model?: string;
  publish_formats?: string[];
}

export interface ReviewRequest {
  text: string;
  instructions?: string;
  context?: Record<string, unknown>;
}

export interface ResearchRequest {
  question: string;
  constraints?: Record<string, unknown>;
  guidelines?: string;
  primary_source?: string;
  model?: string;
}

export interface TranslateRequest {
  text: string;
  target_lang: string;
}

export interface TranslateResponse {
  translated_text: string;
}

export type JobStatus =
  | "queued"
  | "running"
  | "completed"
  | "failed"
  | "cancelled";

export interface JobCreatedResponse {
  job_id: string;
  status?: "queued";
  poll_url: string;
}

export interface JobResultResponse {
  id: string;
  status: JobStatus;
  result?: Record<string, unknown> | null;
}

export interface ResearchResponse {
  report_id: string;
  status?: JobStatus;
}

export interface ResearchResultSource {
  id: string;
  title?: string;
  url?: string;
}

export interface ResearchResultResponse {
  report_id: string;
  status: JobStatus;
  result?: string | null;
  sources?: ResearchResultSource[] | null;
}
