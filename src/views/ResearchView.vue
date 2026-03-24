<script setup lang="ts">
import { computed, ref } from "vue";
import { backendRegistry, mockResearchRun } from "../modules/backend/backendClient";
import { useToast } from "../composables/toast";

type Message = {
  id: string;
  role: "assistant" | "user";
  content: string;
  timestamp: string;
  citations?: string[];
};

type ResearchThread = {
  id: string;
  title: string;
  status: "draft" | "running" | "complete";
  createdAt: string;
  lastUpdated: string;
  summary: string;
  model: string;
  messages: Message[];
};

const toasts = useToast();

function generateId() {
  return Math.random().toString(36).slice(2, 10);
}

function formatTime(date = new Date()) {
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

const modelInventory = ref([
  { id: "gpt-4.1-mini", label: "GPT-4.1 mini", provider: "OpenAI", available: false },
  { id: "sonnet-3.5", label: "Claude 3.5 Sonnet", provider: "Anthropic", available: false },
  { id: "elefant-local", label: "Ollama · Elefant Legal Blend", provider: "Local", available: true },
]);

const threads = ref<ResearchThread[]>([
  {
    id: generateId(),
    title: "Tax compliance for SaaS contracts",
    status: "complete",
    createdAt: new Date(Date.now() - 1000 * 60 * 60 * 6).toISOString(),
    lastUpdated: new Date(Date.now() - 1000 * 60 * 25).toISOString(),
    summary:
      "Mapped nexus triggers for five jurisdictions and highlighted withholding obligations for digital services.",
    model: "elefant-local",
    messages: [
      {
        id: generateId(),
        role: "assistant",
        content: "Ask a question to expand this research thread.",
        timestamp: formatTime(),
      },
    ],
  },
  {
    id: generateId(),
    title: "GDPR data retention checklist",
    status: "running",
    createdAt: new Date(Date.now() - 1000 * 60 * 15).toISOString(),
    lastUpdated: new Date(Date.now() - 1000 * 60 * 5).toISOString(),
    summary: "Compiling supervisory guidance and recent enforcement actions involving data retention.",
    model: "elefant-local",
    messages: [
      {
        id: generateId(),
        role: "assistant",
        content: "Working through the supervisory decisions and ENISA recommendations.",
        timestamp: formatTime(),
      },
    ],
  },
]);

const activeThreadId = ref(threads.value[0]?.id ?? null);
const prompt = ref("");
const selectedModel = ref(modelInventory.value.find((m) => m.available)?.id ?? "");
const isResearching = ref(false);
const showApiDocs = ref(false);
const researchDocs = computed(() =>
  backendRegistry.value.filter((doc) => doc.command.includes("research")),
);

const activeThread = computed(() =>
  activeThreadId.value ? threads.value.find((thread) => thread.id === activeThreadId.value) ?? null : null,
);

const messages = computed(() => activeThread.value?.messages ?? []);

// Model defaults to first available; no runtime re-check needed

function startNewThread() {
  const newThread: ResearchThread = {
    id: generateId(),
    title: "Untitled research thread",
    status: "draft",
    createdAt: new Date().toISOString(),
    lastUpdated: new Date().toISOString(),
    summary: "No findings yet. Submit a question to begin the research pass.",
    model: selectedModel.value || "elefant-local",
    messages: [
      {
        id: generateId(),
        role: "assistant",
        content: "Ready whenever you are. What would you like me to investigate?",
        timestamp: formatTime(),
      },
    ],
  };
  threads.value.unshift(newThread);
  activeThreadId.value = newThread.id;
}

async function submitPrompt() {
  const thread = activeThread.value;
  if (!thread || !prompt.value.trim() || isResearching.value) return;

  if (!selectedModel.value) {
    toasts?.addToast("Add an API key in Settings to unlock hosted research models.", "error");
    return;
  }

  isResearching.value = true;
  const content = prompt.value.trim();
  prompt.value = "";

  thread.messages.push({
    id: generateId(),
    role: "user",
    content,
    timestamp: formatTime(),
  });
  thread.status = "running";
  thread.lastUpdated = new Date().toISOString();
  thread.summary = "Research underway. Results will include citations and suggested follow-ups.";

  try {
    toasts?.addToast("Research request sent to backend.", "info");
    const response = await mockResearchRun(thread.id, content);
    thread.messages.push({
      id: generateId(),
      role: "assistant",
      content: response.answer,
      timestamp: formatTime(),
      citations: response.citations,
    });
    thread.status = "complete";
    thread.summary = response.answer.slice(0, 180) + (response.answer.length > 180 ? "..." : "");
  } catch (error) {
    console.error(error);
    toasts?.addToast("Mock backend failed to return research results.", "error");
    thread.status = "draft";
  } finally {
    thread.lastUpdated = new Date().toISOString();
    isResearching.value = false;
  }
}

function activateThread(id: string) {
  activeThreadId.value = id;
}

const workflowNotes = [
  {
    title: "Draft the request",
    description:
      "Frame the issue, key jurisdictions, and any known authorities. Longer prompts generate better first passes.",
  },
  {
    title: "Review generated memo",
    description:
      "Each response cites authority. Mark gaps or ask follow-up questions to iterate inside the same thread.",
  },
  {
    title: "Return via history",
    description:
      "The left-hand dashboard keeps a running audit log. Resume any matter by choosing its saved thread.",
  },
];
</script>

<template>
  <div class="space-y-6">
    <header class="flex flex-col gap-2">
      <h1 class="h1">Research Assistant</h1>
      <p class="body-muted">
        Targeted research plans with citations, grounded on your local matter library.
      </p>
      <button class="btn-secondary self-start text-xs" type="button" @click="showApiDocs = !showApiDocs">
        {{ showApiDocs ? "Hide API surface" : "API surface" }}
      </button>
    </header>

    <section v-if="showApiDocs" class="card space-y-3 text-xs text-[var(--primary-600)]">
      <header class="flex items-center justify-between">
        <h2 class="text-sm font-semibold text-[var(--primary-800)]">Backend expectations</h2>
        <span v-if="!researchDocs.length" class="text-[var(--primary-500)]">No research-specific commands documented.</span>
      </header>
      <ul class="space-y-2">
        <li v-for="doc in researchDocs" :key="doc.command" class="rounded border border-[var(--primary-200)] bg-white p-3">
          <div class="text-sm font-semibold text-[var(--primary-800)]">{{ doc.command }}</div>
          <p class="mt-1 font-medium text-[var(--primary-700)]">{{ doc.description }}</p>
          <pre class="mt-2 overflow-auto rounded bg-[var(--primary-100)] p-2 text-[10px]">{{ JSON.stringify(doc.expectedPayload, null, 2) }}</pre>
          <p class="mt-2 text-[var(--primary-500)]">{{ doc.notes }}</p>
        </li>
      </ul>
    </section>

    <div class="grid gap-6 xl:grid-cols-[320px,1fr]">
      <aside class="card flex flex-col gap-5">
        <div class="flex items-center justify-between">
          <div>
            <h2 class="text-sm font-semibold uppercase tracking-wide text-[var(--primary-500)]">
              Research threads
            </h2>
            <p class="text-xs text-[var(--primary-500)]">Organised by matter so you can resume without losing context.</p>
          </div>
          <button class="btn-secondary text-xs" type="button" @click="startNewThread">New Thread</button>
        </div>

        <ul class="space-y-2 overflow-auto pr-1">
          <li v-for="thread in threads" :key="thread.id">
            <button
              type="button"
              class="group flex w-full flex-col rounded-lg border border-transparent bg-white px-3 py-2 text-left transition hover:border-[var(--accent-400)]"
              :class="activeThread?.id === thread.id ? 'border-[var(--accent-500)] shadow-sm' : ''"
              @click="activateThread(thread.id)"
            >
              <div class="flex items-center justify-between">
                <span class="text-sm font-semibold text-[var(--primary-800)]">{{ thread.title }}</span>
                <span
                  class="rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase"
                  :class="[
                    thread.status === 'complete'
                      ? 'bg-[var(--primary-100)] text-[var(--primary-600)]'
                      : thread.status === 'running'
                        ? 'bg-[var(--accent-100)] text-[var(--accent-700)]'
                        : 'bg-[var(--primary-900)] text-white',
                  ]"
                >
                  {{
                    thread.status === "complete"
                      ? "Complete"
                      : thread.status === "running"
                        ? "In Progress"
                        : "Draft"
                  }}
                </span>
              </div>
              <div class="mt-1 text-xs text-[var(--primary-500)]">
                Updated {{ new Date(thread.lastUpdated).toLocaleString() }}
              </div>
              <p class="mt-2 text-xs text-[var(--primary-600)]">{{ thread.summary }}</p>
            </button>
          </li>
        </ul>

        <section class="rounded-lg border border-[var(--primary-200)] bg-[var(--primary-50)] p-3 text-xs text-[var(--primary-600)]">
          <h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-[var(--primary-500)]">
            Common workflow
          </h3>
          <ul class="space-y-2">
            <li v-for="note in workflowNotes" :key="note.title">
              <div class="font-semibold text-[var(--primary-700)]">{{ note.title }}</div>
              <p>{{ note.description }}</p>
            </li>
          </ul>
        </section>
      </aside>

      <section class="card flex h-full flex-col space-y-5">
        <header class="space-y-3">
          <div class="flex flex-wrap items-center gap-3">
            <label class="text-sm font-medium text-[var(--primary-700)]">
              Model
              <select class="input mt-1" v-model="selectedModel">
                <option value="" disabled>Select a model (configure in Settings)</option>
                <option
                  v-for="option in modelInventory"
                  :key="option.id"
                  :value="option.id"
                  :disabled="!option.available"
                >
                  {{ option.label }} · {{ option.provider }} {{ option.available ? "" : "(add API key)" }}
                </option>
              </select>
            </label>
            <span class="text-xs text-[var(--primary-500)]">
              Hosted models unlock after you store provider API keys in Settings.
            </span>
          </div>
          <div>
            <label class="sr-only" for="research-question">Research question</label>
            <textarea
              id="research-question"
              v-model="prompt"
              class="input min-h-[140px]"
              placeholder="Draft a question for the research team. Include jurisdictions, facts, desired deliverable, and any exclusions."
            />
          </div>
          <div class="flex items-center justify-between">
            <button class="btn-primary" type="button" :disabled="isResearching" @click="submitPrompt">Start Research</button>
            <span class="text-xs text-[var(--primary-500)]">
              Threads keep your transcripts and citations together for each matter.
            </span>
          </div>
        </header>

        <div class="flex-1 space-y-3 overflow-auto pr-1">
          <div
            v-for="message in messages"
            :key="message.id"
            :class="message.role === 'user' ? 'message-user' : 'message-assistant'"
          >
            <div class="mb-1 flex items-center justify-between text-[10px] uppercase tracking-wide text-[var(--primary-500)]">
              <span>{{ message.role === "user" ? "You" : "Research agent" }}</span>
              <span>{{ message.timestamp }}</span>
            </div>
            <p class="text-sm">{{ message.content }}</p>
            <div v-if="message.citations?.length" class="mt-2 flex flex-wrap gap-2 text-[0.65rem]">
              <span v-for="cite in message.citations" :key="cite" class="chip">
                {{ cite }}
              </span>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>
