<script setup lang="ts">
import { computed, ref } from "vue";
import {
  backendRegistry,
  mockDocumentChat,
  mockInitialReview,
  mockRegisterReview,
} from "../modules/backend/backendClient";
import { useToast } from "../composables/toast";

const toasts = useToast();

type ReviewStatus = "idle" | "running" | "ready";

type UploadedFile = {
  id: string;
  name: string;
  size: number;
  lastReviewedAt: string;
  prompt?: string;
};

type SessionMessage = {
  id: string;
  role: "assistant" | "user";
  content: string;
  timestamp: string;
  citations?: string[];
};

type ReviewSession = {
  file: UploadedFile;
  messages: SessionMessage[];
  summary: string;
  reviewStatus: ReviewStatus;
};

const fileInputRef = ref<HTMLInputElement | null>(null);
const customPrompt = ref("");
const question = ref("");
const syncingCount = ref(0);
const isSyncingBackend = computed(() => syncingCount.value > 0);
const sendingCount = ref(0);
const isSending = computed(() => sendingCount.value > 0);
const showApiDocs = ref(false);

function generateId() {
  return Math.random().toString(36).slice(2, 10);
}

function formatTimestamp(date = new Date()) {
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

const initialSessionId = generateId();

const sessions = ref<Record<string, ReviewSession>>({
  [initialSessionId]: {
    file: {
      id: initialSessionId,
      name: "Contract_2024.pdf",
      size: 2.3,
      lastReviewedAt: new Date().toISOString(),
    },
    reviewStatus: "ready",
    summary:
      "Initial review prepared for Contract_2024.pdf. Key focus areas: obligations, deadlines, and risk language.",
    messages: [
      {
        id: generateId(),
        role: "assistant",
        content: "How can I help you analyze Contract_2024.pdf?",
        timestamp: formatTimestamp(),
      },
      {
        id: generateId(),
        role: "user",
        content: "Highlight the key renewal obligations.",
        timestamp: formatTimestamp(),
      },
      {
        id: generateId(),
        role: "assistant",
        content:
          "Renewal clauses flagged: • Notice deadline 60 days prior to term end • Automatic renewal unless written notice • Pricing escalation tied to CPI adjustments.",
        timestamp: formatTimestamp(),
        citations: ["Contract_2024.pdf · Section 4"],
      },
    ],
  },
});

const activeSessionId = ref<string | null>(initialSessionId);

const orderedSessions = computed(() =>
  Object.values(sessions.value).sort((a, b) =>
    a.file.lastReviewedAt < b.file.lastReviewedAt ? 1 : -1,
  ),
);

const activeSession = computed(() =>
  activeSessionId.value ? sessions.value[activeSessionId.value] ?? null : null,
);

const activeMessages = computed(() => activeSession.value?.messages ?? []);

function triggerFilePicker() {
  fileInputRef.value?.click();
}

function registerFile(file: File) {
  const id = generateId();
  const details: UploadedFile = {
    id,
    name: file.name,
    size: +(file.size / (1024 * 1024)).toFixed(2),
    lastReviewedAt: new Date().toISOString(),
    prompt: customPrompt.value.trim() || undefined,
  };

  sessions.value[id] = {
    file: details,
    reviewStatus: "idle",
    summary: "",
    messages: [],
  };

  customPrompt.value = "";
  openSession(id);
  queueInitialReview(details);
}

function handleFiles(files: FileList | null) {
  if (!files) return;
  Array.from(files)
    .filter((file) => /\.(pdf|docx|txt)$/i.test(file.name))
    .forEach(registerFile);
}

function handleInputChange(event: Event) {
  const input = event.target as HTMLInputElement | null;
  handleFiles(input?.files ?? null);
}

function handleDrop(event: DragEvent) {
  event.preventDefault();
  handleFiles(event.dataTransfer?.files ?? null);
}

function beginInitialReview(session: ReviewSession) {
  if (session.reviewStatus === "ready" || session.reviewStatus === "running") return;

  session.messages.push({
    id: generateId(),
    role: "assistant",
    content: `Starting initial review for ${session.file.name}.`,
    timestamp: formatTimestamp(),
  });

  session.reviewStatus = "running";
  session.messages.push({
    id: generateId(),
    role: "assistant",
    content: "Preparing initial summary using backend.",
    timestamp: formatTimestamp(),
  });
}

function openSession(id: string) {
  activeSessionId.value = id;
  const session = sessions.value[id];
  if (!session) return;
  beginInitialReview(session);
}

async function queueInitialReview(file: UploadedFile) {
  try {
    syncingCount.value++;
    const results = await mockRegisterReview([
      {
        id: file.id,
        name: file.name,
        sizeBytes: Math.round(file.size * 1024 * 1024),
        customPrompt: file.prompt ?? null,
      },
    ]);
    if (!results.length) {
      toasts.addToast("File could not be registered for review.", "error");
      return;
    }
    const summary = results[0];
    const session = sessions.value[file.id];
    if (!session) return;
    session.summary = summary.summary;
    const detailed = await mockInitialReview(file.id);
    session.messages.push(...detailed.messages);
    session.summary = detailed.summary;
    session.reviewStatus = "ready";
    session.file.lastReviewedAt = new Date().toISOString();
  } catch (error) {
    console.error(error);
    const session = sessions.value[file.id];
    if (session) {
      session.reviewStatus = "idle";
      session.messages = [];
    }
    toasts.addToast("Failed to start initial review. Please try again.", "error");
  } finally {
    syncingCount.value--;
  }
}

async function askQuestion() {
  const session = activeSession.value;
  if (!session || !question.value.trim() || isSending.value) return;

  sendingCount.value++;
  const now = formatTimestamp();
  const content = question.value;
  question.value = "";

  session.messages.push({
    id: generateId(),
    role: "user",
    content,
    timestamp: now,
  });

  try {
    const response = await mockDocumentChat(session.file.id, content);
    session.messages.push(response);
    session.file.lastReviewedAt = new Date().toISOString();
  } catch (error) {
    console.error(error);
    toasts.addToast("Failed to get a response. Please try again.", "error");
  } finally {
    sendingCount.value--;
  }
}

const workflowSteps = [
  {
    title: "Drop or browse for documents",
    detail: "Upload one or more files and include an optional custom prompt for the first pass.",
  },
  {
    title: "Initial review auto-starts",
    detail:
      "Selecting an active document triggers a quick outline and risk triage so you see actionable context immediately.",
  },
  {
    title: "Chat in the workspace",
    detail:
      "Follow-up questions stay threaded with the document. Responses cite page-level references.",
  },
  {
    title: "Return via dashboard",
    detail:
      "All sessions appear in the active documents list. Jump back in anytime to continue the same chat.",
  },
];
</script>

<template>
  <div class="space-y-6">
    <header class="flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
      <div class="space-y-1">
        <h1 class="h1">Document Review</h1>
        <p class="body-muted">Review and chat with your documents.</p>
      </div>
      <div class="flex items-start gap-3 md:items-center">
        <span class="badge-trust">Local-only • Encrypted</span>
        <button
          class="btn-secondary flex items-center gap-2 text-xs"
          type="button"
          @click="() => (showApiDocs = !showApiDocs)"
        >
          API surface
        </button>
      </div>
    </header>

    <section v-if="showApiDocs" class="card space-y-3 text-xs text-[var(--primary-600)]">
      <header class="flex items-center justify-between">
        <h2 class="text-sm font-semibold text-[var(--primary-800)]">Backend expectations</h2>
        <button class="btn-secondary text-xs" type="button" @click="showApiDocs = false">Hide</button>
      </header>
      <ul class="space-y-2">
        <li v-for="doc in backendRegistry" :key="doc.command" class="rounded border border-[var(--primary-200)] bg-white p-3">
          <div class="text-sm font-semibold text-[var(--primary-800)]">{{ doc.command }}</div>
          <p class="mt-1 font-medium text-[var(--primary-700)]">{{ doc.description }}</p>
          <pre class="mt-2 overflow-auto rounded bg-[var(--primary-100)] p-2 text-[10px]">{{ JSON.stringify(doc.expectedPayload, null, 2) }}</pre>
          <p class="mt-2 text-[var(--primary-500)]">{{ doc.notes }}</p>
        </li>
      </ul>
    </section>

    <div class="grid gap-6 xl:grid-cols-[340px,1fr]">
      <div class="space-y-6">
        <section class="card space-y-4">
          <header class="space-y-1">
            <h2 class="text-base font-semibold text-[var(--primary-800)]">Start a new review</h2>
            <p class="text-sm text-[var(--primary-600)]">
              Drop files or browse from disk, then tailor the first-pass analysis with your own prompt.
            </p>
          </header>

          <div
            class="flex flex-col gap-4 rounded-lg border-2 border-dashed border-[var(--primary-400)] bg-[var(--primary-200)] p-4 text-center transition hover:border-[var(--accent-500)] hover:bg-[color:color-mix(in_srgb,var(--primary-200)_40%,white)]"
            @dragover.prevent
            @drop="handleDrop"
          >
            <div class="text-base font-medium">Drop files here or browse</div>
            <p class="text-xs text-[var(--primary-600)]">PDF, DOCX, TXT supported. Files never leave this device.</p>
            <div class="flex flex-wrap items-center justify-center gap-3">
              <button class="btn-primary" type="button" @click="triggerFilePicker">Browse Files</button>
              <input
                ref="fileInputRef"
                type="file"
                class="hidden"
                accept=".pdf,.docx,.txt"
                multiple
                @change="handleInputChange"
              />
            </div>

            <label class="text-left text-sm text-[var(--primary-700)]">
              Custom prompt (optional)
              <textarea
                v-model="customPrompt"
                class="input mt-1 min-h-[80px]"
                placeholder="e.g. Focus on indemnity, renewal, and data handling clauses."
              />
            </label>
          </div>

          <p class="text-xs text-[var(--primary-500)]">
            The prompt is stored with each session so summaries and follow-up answers stay aligned with your priorities.
          </p>
        </section>

        <section class="card space-y-3">
          <header class="flex items-center justify-between">
            <div>
              <h2 class="text-sm font-semibold uppercase tracking-wide text-[var(--primary-500)]">
                Active document sessions
              </h2>
              <p class="text-xs text-[var(--primary-500)]">Select a session to resume a chat or trigger an initial review.</p>
            </div>
          </header>

          <ul class="space-y-2">
            <li v-for="session in orderedSessions" :key="session.file.id">
              <button
                type="button"
                class="group flex w-full flex-col rounded-lg border border-[var(--primary-300)] bg-white px-3 py-2 text-left transition hover:border-[var(--accent-400)]"
                :class="activeSession?.file.id === session.file.id ? 'border-[var(--accent-500)] shadow-sm' : ''"
                @click="openSession(session.file.id)"
              >
                <div class="flex items-center justify-between">
                  <span class="text-sm font-semibold text-[var(--primary-800)]">{{ session.file.name }}</span>
                  <span class="text-xs text-[var(--primary-500)]">{{ session.file.size }} MB</span>
                </div>
                <div class="mt-1 flex flex-wrap items-center gap-2 text-xs text-[var(--primary-500)]">
                  <span
                    class="rounded-full px-2 py-0.5 font-semibold"
                    :class="[
                      session.reviewStatus === 'ready'
                        ? 'bg-[var(--primary-100)] text-[var(--primary-600)]'
                        : session.reviewStatus === 'running'
                          ? 'bg-[var(--accent-100)] text-[var(--accent-600)]'
                          : 'bg-[var(--primary-900)] text-white',
                    ]"
                  >
                    {{
                      session.reviewStatus === "ready"
                        ? "Summary ready"
                        : session.reviewStatus === "running"
                          ? "Reviewing"
                          : "Awaiting review"
                    }}
                  </span>
                  <span>Last touched {{ new Date(session.file.lastReviewedAt).toLocaleString() }}</span>
                </div>
              </button>
            </li>
          </ul>
        </section>

        <section class="card space-y-3">
          <h2 class="text-sm font-semibold uppercase tracking-wide text-[var(--primary-500)]">Suggested workflow</h2>
          <ol class="space-y-2 text-sm text-[var(--primary-600)]">
            <li
              v-for="(step, index) in workflowSteps"
              :key="step.title"
              class="flex gap-3 rounded-md border border-[var(--primary-200)] bg-[var(--primary-100)] p-3"
            >
              <span class="flex h-6 w-6 items-center justify-center rounded-full bg-[var(--accent-500)] text-xs font-semibold text-white">
                {{ index + 1 }}
              </span>
              <div>
                <div class="font-semibold text-[var(--primary-700)]">{{ step.title }}</div>
                <p class="text-xs text-[var(--primary-500)]">{{ step.detail }}</p>
              </div>
            </li>
          </ol>
        </section>
      </div>

      <section class="card flex h-full flex-col space-y-4">
        <div v-if="activeSession" class="flex flex-col space-y-4">
          <header class="flex flex-col gap-2 border-b border-[var(--primary-200)] pb-3 md:flex-row md:items-center md:justify-between">
            <div>
              <h2 class="text-lg font-semibold text-[var(--primary-800)]">
                {{ activeSession.file.name }}
              </h2>
              <p class="text-xs text-[var(--primary-500)]">
                {{ activeSession.file.size }} MB • Session since {{ new Date(activeSession.file.lastReviewedAt).toLocaleDateString() }}
              </p>
            </div>
            <div class="flex flex-wrap items-center gap-3">
              <span
                class="rounded-full px-3 py-1 text-xs font-semibold uppercase"
                :class="[
                  activeSession.reviewStatus === 'ready'
                    ? 'bg-[var(--primary-100)] text-[var(--primary-600)]'
                    : activeSession.reviewStatus === 'running'
                      ? 'bg-[var(--accent-100)] text-[var(--accent-700)]'
                      : 'bg-[var(--primary-900)] text-white',
                ]"
              >
                {{
                  activeSession.reviewStatus === "ready"
                    ? "Initial review complete"
                    : activeSession.reviewStatus === "running"
                      ? "Review in progress"
                      : "Awaiting review"
                }}
              </span>
              <span v-if="isSyncingBackend" class="flex items-center gap-2 text-xs text-[var(--primary-500)]">
                <span class="spinner"></span> Syncing with backend...
              </span>
              <button class="btn-secondary text-xs" type="button">
                Export Session
              </button>
            </div>
          </header>

          <section class="rounded-lg border border-[var(--primary-200)] bg-[var(--primary-50)] p-4 text-sm text-[var(--primary-700)]">
            <h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-[var(--primary-500)]">
              Initial review summary
            </h3>
            <p v-if="activeSession.summary">{{ activeSession.summary }}</p>
            <p v-else class="text-[var(--primary-500)]">
              Select a document to trigger its first-pass review and populate this summary.
            </p>
          </section>

          <div class="flex-1 space-y-3 overflow-auto pr-1">
            <div
              v-for="message in activeMessages"
              :key="message.id"
              :class="message.role === 'user' ? 'message-user' : 'message-assistant'"
            >
              <div class="mb-2 flex items-center justify-between text-xs text-[var(--primary-500)]">
                <span>{{ message.role === "user" ? "You" : "Elefant Assistant" }}</span>
                <span>{{ message.timestamp }}</span>
              </div>
              <p>{{ message.content }}</p>
              <div v-if="message.citations?.length" class="mt-2 flex flex-wrap gap-2">
                <span v-for="citation in message.citations" :key="citation" class="chip">
                  {{ citation }}
                </span>
              </div>
            </div>
          </div>

          <form class="flex gap-2" @submit.prevent="askQuestion">
            <label class="sr-only" for="document-question">Ask a follow-up</label>
            <input
              id="document-question"
              v-model="question"
              class="input flex-1"
              placeholder="Ask about obligations, timelines, or definitions..."
            />
            <button type="submit" class="btn-primary" :disabled="isSending">Send</button>
          </form>
        </div>

        <div v-else class="flex flex-1 flex-col items-center justify-center space-y-3 text-center text-sm text-[var(--primary-600)]">
          <p>Select a document session from the left to view its summary and conversation.</p>
          <p>Upload a new file or choose an existing session to resume the chat where you left off.</p>
        </div>
      </section>
    </div>
  </div>
</template>
