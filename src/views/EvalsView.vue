<script setup lang="ts">
import { computed, ref } from "vue";
import { backendRegistry, mockEvalsRun } from "../modules/backend/backendClient";
import { useToast } from "../composables/toast";

type ModelOption = {
  id: string;
  label: string;
  provider: string;
  available: boolean;
  selected: boolean;
};

type BenchmarkRun = {
  id: string;
  name: string;
  dataset: string;
  ranAt: string;
  models: string[];
  notes: string;
  results: Record<string, number>;
};

const rubricMetrics = [
  "Accuracy",
  "Evidence of source checking",
  "Conciseness",
  "Comprehensiveness of sources",
  "Legal reasoning",
  "Risk management",
];

function generateId() {
  return Math.random().toString(36).slice(2, 10);
}

const models = ref<ModelOption[]>([
  { id: "gpt-4.1-mini", label: "GPT-4.1 mini", provider: "OpenAI", available: false, selected: false },
  { id: "claude-3.5-sonnet", label: "Claude 3.5 Sonnet", provider: "Anthropic", available: false, selected: false },
  { id: "elefant-ollama", label: "Ollama · Elefant Legal Blend", provider: "Local", available: true, selected: true },
]);

const benchmarkHistory = ref<BenchmarkRun[]>([
  {
    id: generateId(),
    name: "Document review task",
    dataset: "10 commercial contracts · 54 targeted questions",
    ranAt: new Date(Date.now() - 1000 * 60 * 60 * 6).toISOString(),
    models: ["elefant-ollama"],
    notes: "Baseline run for Q1 contract set.",
    results: {
      Accuracy: 0.78,
      "Evidence of source checking": 0.64,
      Conciseness: 0.71,
      "Comprehensiveness of sources": 0.69,
      "Legal reasoning": 0.74,
      "Risk management": 0.6,
    },
  },
  {
    id: generateId(),
    name: "Clause drafting scenario",
    dataset: "Non-compete templates · 20 prompts",
    ranAt: new Date(Date.now() - 1000 * 60 * 60 * 28).toISOString(),
    models: ["elefant-ollama"],
    notes: "Evaluated drafting tone and citation behavior.",
    results: {
      Accuracy: 0.82,
      "Evidence of source checking": 0.58,
      Conciseness: 0.75,
      "Comprehensiveness of sources": 0.62,
      "Legal reasoning": 0.71,
      "Risk management": 0.65,
    },
  },
]);

const isRunning = ref(false);
const toast = useToast();
const showApiDocs = ref(false);
const evalDocs = computed(() =>
  backendRegistry.value.filter((doc) => doc.command.includes("eval")),
);

const customBenchmark = ref({
  name: "",
  description: "",
  prompts: "",
  judgeCount: 4,
  weighting: rubricMetrics.map(() => 1),
});

const selectedModels = computed(() => models.value.filter((model) => model.selected));

function toggleModel(model: ModelOption) {
  if (!model.available) return;
  model.selected = !model.selected;
}

function runBenchmark(label: string) {
  if (!selectedModels.value.length) {
    toast.addToast("Select at least one model before running benchmarks.", "error");
    return;
  }
  isRunning.value = true;
  const readable = label.replace(/-/g, " ");
  toast.addToast(`Running ${readable} benchmark...`, "info");
  // TODO: invoke("evals_run", { label, models: selectedModels.value.map((m) => m.id) })
  mockEvalsRun(label, selectedModels.value.map((m) => m.id))
    .then((response) => {
      benchmarkHistory.value.unshift({
        id: response.benchmarkId,
        name: readable,
        dataset: `Template benchmark (${label})`,
        ranAt: response.startedAt,
        models: selectedModels.value.map((m) => m.id),
        notes: "Queued via quick-run trigger.",
        results: rubricMetrics.reduce<Record<string, number>>((acc, metric) => {
          acc[metric] = 0;
          return acc;
        }, {}),
      });
      toast.addToast(`${readable} benchmark completed. Review the results below.`, "success");
    })
    .catch(() => {
      toast.addToast("Mock backend failed to queue benchmark.", "error");
    })
    .finally(() => {
      isRunning.value = false;
    });
}

function scheduleCustomBenchmark() {
  if (!customBenchmark.value.name.trim() || !customBenchmark.value.prompts.trim()) {
    return;
  }
  const run: BenchmarkRun = {
    id: generateId(),
    name: customBenchmark.value.name,
    dataset: customBenchmark.value.description || "Custom benchmark",
    ranAt: new Date().toISOString(),
    models: selectedModels.value.map((m) => m.id),
    notes: "Queued via UI. Judges: " + customBenchmark.value.judgeCount,
    results: rubricMetrics.reduce<Record<string, number>>((acc, metric) => {
      acc[metric] = 0;
      return acc;
    }, {}),
  };
  benchmarkHistory.value.unshift(run);
  customBenchmark.value = {
    name: "",
    description: "",
    prompts: "",
    judgeCount: 4,
    weighting: rubricMetrics.map(() => 1),
  };
}
</script>

<template>
  <div class="space-y-6">
    <header class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
      <div>
        <h1 class="h1">Model Evaluation Benchmarks</h1>
        <p class="body-muted">
          Compare connected models on curated legal tasks and keep an audit log of historical scores.
        </p>
      </div>
      <div class="max-w-sm text-xs text-[var(--primary-500)] text-right md:text-left">
        Export results with timestamps for regulatory records. Hosted models require API keys saved in Settings.
      </div>
    </header>

    <button class="btn-secondary text-xs" type="button" @click="showApiDocs = !showApiDocs">
      {{ showApiDocs ? "Hide API surface" : "API surface" }}
    </button>

    <section v-if="showApiDocs" class="card space-y-3 text-xs text-[var(--primary-600)]">
      <header class="flex items-center justify-between">
        <h2 class="text-sm font-semibold text-[var(--primary-800)]">Backend expectations</h2>
        <span v-if="!evalDocs.length" class="text-[var(--primary-500)]">No evaluation commands documented.</span>
      </header>
      <ul class="space-y-2">
        <li v-for="doc in evalDocs" :key="doc.command" class="rounded border border-[var(--primary-200)] bg-white p-3">
          <div class="text-sm font-semibold text-[var(--primary-800)]">{{ doc.command }}</div>
          <p class="mt-1 font-medium text-[var(--primary-700)]">{{ doc.description }}</p>
          <pre class="mt-2 overflow-auto rounded bg-[var(--primary-100)] p-2 text-[10px]">{{ JSON.stringify(doc.expectedPayload, null, 2) }}</pre>
          <p class="mt-2 text-[var(--primary-500)]">{{ doc.notes }}</p>
        </li>
      </ul>
    </section>

    <section class="card space-y-5">
      <div>
        <h2 class="text-sm font-semibold uppercase tracking-wide text-[var(--primary-500)]">
          Models available for evaluation
        </h2>
        <p class="text-xs text-[var(--primary-500)]">
          Toggle the models you want to benchmark. Unavailable entries indicate missing credentials.
        </p>
      </div>
      <div class="grid gap-3 md:grid-cols-3">
        <button
          v-for="model in models"
          :key="model.id"
          type="button"
          class="flex flex-col rounded-lg border px-3 py-3 text-left transition"
          :class="[
            model.available
              ? model.selected
                ? 'border-[var(--accent-500)] bg-[color:color-mix(in_srgb,var(--accent-100)_60%,white)]'
                : 'border-[var(--primary-300)] bg-white hover:border-[var(--accent-400)]'
              : 'border-dashed border-[var(--primary-300)] bg-[var(--primary-100)] opacity-70 cursor-not-allowed',
          ]"
          @click="toggleModel(model)"
        >
          <div class="flex items-center justify-between text-sm font-semibold text-[var(--primary-800)]">
            <span>{{ model.label }}</span>
            <span v-if="!model.available" class="text-[10px] uppercase tracking-wide text-[var(--primary-500)]">
              Add API key
            </span>
          </div>
          <div class="mt-1 text-xs text-[var(--primary-500)]">Provider: {{ model.provider }}</div>
          <div v-if="model.available" class="mt-2 text-xs text-[var(--primary-600)]">
            {{ model.selected ? "Included in next run" : "Tap to include in next run" }}
          </div>
          <div v-else class="mt-2 text-xs text-[var(--primary-500)]">
            Configure credentials under Settings → Secrets.
          </div>
        </button>
      </div>
    </section>

    <div class="grid gap-6 xl:grid-cols-[360px,1fr]">
      <aside class="card space-y-4">
        <div class="flex items-center justify-between">
          <h2 class="text-sm font-semibold uppercase tracking-wide text-[var(--primary-500)]">
            Benchmark history
          </h2>
          <button class="btn-secondary text-xs" type="button">Export CSV</button>
        </div>
        <ul class="space-y-3">
          <li v-for="run in benchmarkHistory" :key="run.id" class="rounded-lg border border-[var(--primary-200)] bg-white p-3 text-xs text-[var(--primary-600)]">
            <div class="flex items-center justify-between text-sm font-semibold text-[var(--primary-800)]">
              <span>{{ run.name }}</span>
              <span>{{ new Date(run.ranAt).toLocaleDateString() }}</span>
            </div>
            <div class="mt-1 text-[var(--primary-500)]">{{ run.dataset }}</div>
            <div class="mt-1">
              Models:
              <span class="font-medium text-[var(--primary-700)]">
                {{ run.models.length ? run.models.join(", ") : "None selected" }}
              </span>
            </div>
            <div class="mt-1 text-[var(--primary-500)]">
              {{ run.notes }}
            </div>
          </li>
        </ul>
      </aside>

      <section class="card space-y-6">
        <div class="space-y-3">
          <h2 class="text-base font-semibold text-[var(--primary-800)]">Standard rubric breakdown</h2>
          <div class="grid gap-3 md:grid-cols-2">
            <div
              v-for="metric in rubricMetrics"
              :key="metric"
              class="rounded-lg border border-[var(--primary-200)] bg-[var(--primary-50)] px-3 py-2 text-sm text-[var(--primary-700)]"
            >
              {{ metric }}
            </div>
          </div>
          <p class="text-xs text-[var(--primary-500)]">
            Scores are averaged across four LM judges by default. Adjust judge count in the custom benchmark form.
          </p>
        </div>

        <section class="space-y-3">
          <div class="flex items-center justify-between">
            <h3 class="text-sm font-semibold text-[var(--primary-800)]">Quick run templates</h3>
            <span class="text-xs text-[var(--primary-500)]">Results will be timestamped and saved above.</span>
          </div>
          <div class="grid gap-4 md:grid-cols-3">
            <div class="rounded-lg border border-[var(--primary-200)] bg-white p-4">
              <h4 class="text-sm font-semibold text-[var(--primary-800)]">Document review</h4>
              <p class="mt-1 text-xs text-[var(--primary-500)]">50 Q&A pairs across contracts and policies.</p>
              <button class="btn-primary mt-3 w-full" type="button" @click="runBenchmark('document-review')">
                {{ isRunning ? 'Running...' : 'Run benchmark' }}
              </button>
            </div>
            <div class="rounded-lg border border-[var(--primary-200)] bg-white p-4">
              <h4 class="text-sm font-semibold text-[var(--primary-800)]">Clause drafting</h4>
              <p class="mt-1 text-xs text-[var(--primary-500)]">20 drafting prompts with redlines for comparison.</p>
              <button class="btn-primary mt-3 w-full" type="button" @click="runBenchmark('clause-drafting')">
                {{ isRunning ? 'Running...' : 'Run benchmark' }}
              </button>
            </div>
            <div class="rounded-lg border border-[var(--primary-200)] bg-white p-4">
              <h4 class="text-sm font-semibold text-[var(--primary-800)]">Research memo</h4>
              <p class="mt-1 text-xs text-[var(--primary-500)]">Short-form memos graded on sourcing and reasoning.</p>
              <button class="btn-primary mt-3 w-full" type="button" @click="runBenchmark('research-memo')">
                {{ isRunning ? 'Running...' : 'Run benchmark' }}
              </button>
            </div>
          </div>
        </section>

        <section class="rounded-lg border border-[var(--primary-200)] bg-[var(--primary-50)] p-4 space-y-3">
          <div>
            <h3 class="text-sm font-semibold text-[var(--primary-800)]">Custom benchmark</h3>
            <p class="text-xs text-[var(--primary-500)]">
              Upload test prompts, expected answers, and provide context. Four LM judges score each metric.
            </p>
          </div>
          <div class="grid gap-3 md:grid-cols-2">
            <label class="text-xs font-semibold text-[var(--primary-600)]">
              Benchmark name
              <input v-model="customBenchmark.name" class="input mt-1" placeholder="e.g. Employment handbook QA" />
            </label>
            <label class="text-xs font-semibold text-[var(--primary-600)]">
              Judge count
              <input
                v-model.number="customBenchmark.judgeCount"
                class="input mt-1"
                min="1"
                type="number"
              />
            </label>
            <label class="md:col-span-2 text-xs font-semibold text-[var(--primary-600)]">
              Dataset / notes
              <textarea
                v-model="customBenchmark.description"
                class="input mt-1"
                placeholder="Describe the benchmark scope, matters covered, or reference datasets."
              />
            </label>
            <label class="md:col-span-2 text-xs font-semibold text-[var(--primary-600)]">
              Test prompts and expected answers
              <textarea
                v-model="customBenchmark.prompts"
                class="input mt-1 min-h-[120px]"
                placeholder="Provide prompts and ideal answers in JSON or CSV format."
              />
            </label>
          </div>
          <div class="rounded-lg border border-[var(--primary-200)] bg-white p-3 text-xs text-[var(--primary-600)]">
            <div class="flex items-center justify-between">
              <span class="font-semibold text-[var(--primary-700)]">Metric weighting</span>
              <span>Adjust to emphasise specific qualities.</span>
            </div>
            <div class="mt-3 grid gap-3 md:grid-cols-3">
              <label v-for="(metric, index) in rubricMetrics" :key="metric" class="flex flex-col gap-1">
                <span>{{ metric }}</span>
                <input
                  v-model.number="customBenchmark.weighting[index]"
                  class="input"
                  min="0"
                  step="0.1"
                  type="number"
                />
              </label>
            </div>
          </div>
          <div class="flex items-center justify-between">
            <button class="btn-secondary" type="button">Attach to briefcase</button>
            <button class="btn-primary" type="button" @click="scheduleCustomBenchmark">Queue benchmark</button>
          </div>
        </section>
      </section>
    </div>
  </div>
</template>
