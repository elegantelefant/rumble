// ABOUTME: Documents the backend command surface the frontend expects.
// ABOUTME: Also holds the two remaining mocks — sync test and evals run.
import { ref } from "vue";

export type BackendStatus = "idle" | "running" | "ready" | "error";

export type EvalsRunResponse = {
  benchmarkId: string;
  status: "queued" | "completed";
  startedAt: string;
};

export type ApiSurfaceDocumentation = {
  command: string;
  description: string;
  expectedPayload: Record<string, unknown>;
  notes: string;
};

export const backendRegistry = ref<ApiSurfaceDocumentation[]>([
  {
    command: "documents/register_files",
    description: "Register uploaded documents for review.",
    expectedPayload: {
      files: [
        {
          id: "string uuid",
          name: "string",
          sizeBytes: "number",
          customPrompt: "string | null",
        },
      ],
    },
    notes:
      "Backend should persist metadata per session, queue initial analysis, and respond with session identifiers.",
  },
  {
    command: "documents/initial_review",
    description: "Trigger initial outline for a session and return summary + opening assistant message.",
    expectedPayload: { sessionId: "string uuid" },
    notes:
      "Response should include summary text, citations, and any structured issue list. Use streaming if available.",
  },
  {
    command: "documents/chat",
    description: "Send a follow-up question for a session and return assistant reply with citations.",
    expectedPayload: { sessionId: "string uuid", question: "string" },
    notes:
      "Expect streaming partials; front-end currently assumes full response. Include citations as array of strings referencing document + page.",
  },
  {
    command: "research/run_query",
    description: "Kick off a research plan for a thread.",
    expectedPayload: { threadId: "string uuid", prompt: "string", model: "string" },
    notes: "Return instructions and citations. Should persist to allow resume.",
  },
  {
    command: "translation/run",
    description: "Translate text from source to target language.",
    expectedPayload: {
      sourceLanguage: "string",
      targetLanguage: "string",
      text: "string",
      model: "string",
    },
    notes: "Return translated string and quality/confidence metadata.",
  },
  {
    command: "evals/run",
    description: "Queue a benchmark evaluation.",
    expectedPayload: { benchmarkKey: "string", modelIds: ["string"], weightings: "Record<string, number>" },
    notes: "Respond with queued status and benchmark id. Progress should be subscribable.",
  },
  {
    command: "settings/save",
    description: "Persist provider secrets and local configuration.",
    expectedPayload: {
      secrets: [
        {
          provider: "string",
          apiKey: "string | null",
          scope: "global | drafting | research",
          notes: "string",
        },
      ],
      local: {
        host: "string",
        port: "string",
        model: "string",
      },
    },
    notes: "Provide validation errors if provider requires key but missing.",
  },
  {
    command: "sync/test",
    description: "Ping sync server health endpoint.",
    expectedPayload: { url: "string" },
    notes: "Respond with status ok/failed and optional diagnostic message.",
  },
]);

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export async function mockEvalsRun(_benchmarkKey: string, _models: string[]): Promise<EvalsRunResponse> {
  await delay(500);
  return {
    benchmarkId: `benchmark-${Date.now()}`,
    status: "completed",
    startedAt: new Date().toISOString(),
  };
}

export async function mockTestSync(url: string): Promise<{ ok: boolean; message?: string }> {
  await delay(180);
  return {
    ok: true,
    message: `Mocked connection successful to ${url}`,
  };
}
