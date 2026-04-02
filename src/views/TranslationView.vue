<script setup lang="ts">
defineOptions({ name: "TranslationView" });
import { computed, ref, watch } from "vue";
import { backendRegistry } from "../modules/backend/backendClient";
import { translate } from "../api/sidecar";
import { useModels } from "../composables/models";
import { useToast } from "../composables/toast";
import { generateId } from "../utils/ids";

type TranslationJob = {
  id: string;
  sourceLanguage: string;
  targetLanguage: string;
  inputText: string;
  outputText: string;
  status: "draft" | "complete";
  createdAt: string;
  lastUpdated: string;
  model: string;
};

const sourceLanguage = ref("auto");
const targetLanguage = ref("es");
const sourceText = ref("");
const translatedText = ref("");
const selectedModel = ref("elefant-local");
const showApiDocs = ref(false);
const isTranslating = ref(false);

const { models: modelOptions } = useModels();

const jobs = ref<TranslationJob[]>([
  {
    id: generateId(),
    sourceLanguage: "fr",
    targetLanguage: "en",
    inputText: "Veuillez confirmer la politique de conservation des données pour les dossiers clients.",
    outputText:
      "Please confirm the data retention policy for client records.",
    status: "complete",
    createdAt: new Date(Date.now() - 1000 * 60 * 45).toISOString(),
    lastUpdated: new Date(Date.now() - 1000 * 60 * 45).toISOString(),
    model: "elefant-local",
  },
]);

const activeJobId = ref(jobs.value[0]?.id ?? null);

const activeJob = computed(() =>
  activeJobId.value ? jobs.value.find((job) => job.id === activeJobId.value) ?? null : null,
);

function updateEditorFromJob(job: TranslationJob | null) {
  if (!job) return;
  sourceLanguage.value = job.sourceLanguage;
  targetLanguage.value = job.targetLanguage;
  sourceText.value = job.inputText;
  translatedText.value = job.outputText;
  selectedModel.value = job.model;
}

watch(activeJob, (job) => {
  updateEditorFromJob(job ?? null);
}, { immediate: true });

function activateJob(job: TranslationJob) {
  if (sourceText.value.trim() && sourceText.value !== job.inputText) {
    if (!confirm("You have unsaved text. Load this history entry and discard changes?")) return;
  }
  activeJobId.value = job.id;
  updateEditorFromJob(job);
}

const translationDocs = computed(() =>
  backendRegistry.value.filter((doc) => doc.command.includes("translation")),
);

const toast = useToast();

async function runTranslation() {
  const input = sourceText.value.trim();
  if (!input) return;
  if (sourceLanguage.value !== "auto" && sourceLanguage.value === targetLanguage.value) {
    toast.addToast("Source and target languages are the same.", "error");
    return;
  }
  isTranslating.value = true;
  try {
    const response = await translate({
      text: input,
      target_lang: targetLanguage.value,
    });
    const output = response.translated_text;

    const newJob: TranslationJob = {
      id: generateId(),
      sourceLanguage: sourceLanguage.value,
      targetLanguage: targetLanguage.value,
      inputText: input,
      outputText: output,
      status: "complete",
      createdAt: new Date().toISOString(),
      lastUpdated: new Date().toISOString(),
      model: selectedModel.value,
    };

    jobs.value.unshift(newJob);
    activeJobId.value = newJob.id;
    translatedText.value = output;
    toast.addToast("Translation complete.", "success");
  } catch (error) {
    const message = error instanceof Error ? error.message : "Translation failed. Check sidecar logs.";
    console.error("Translation error:", error);
    toast.addToast(message, "error");
  } finally {
    isTranslating.value = false;
  }
}

const dashboardNotes = [
  "History keeps translations per matter. Attach jobs to briefcases from the job menu.",
  "Hosted models require API keys saved in Settings. Local models run entirely on-device.",
  "Download bilingual outputs for external counsel review or certified translation requests.",
];
</script>

<template>
  <div class="space-y-6">
    <header class="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
      <div>
        <h1 class="h1">Translation</h1>
        <p class="body-muted">
          Draft-quality translations for legal review workflows. Verify with certified professionals before filing.
        </p>
      </div>
      <div class="badge-trust border-[var(--warning)] bg-[#fef7e7] text-[var(--warning)]">
        Draft Quality • Verify before filing
      </div>
    </header>

    <button class="btn-secondary text-xs" type="button" @click="showApiDocs = !showApiDocs">
      {{ showApiDocs ? "Hide API surface" : "API surface" }}
    </button>

    <section v-if="showApiDocs" class="card space-y-3 text-xs text-[var(--primary-600)]">
      <header class="flex items-center justify-between">
        <h2 class="text-sm font-semibold text-[var(--primary-800)]">Backend expectations</h2>
        <span v-if="!translationDocs.length" class="text-[var(--primary-500)]">No translation-specific commands documented.</span>
      </header>
      <ul class="space-y-2">
        <li v-for="doc in translationDocs" :key="doc.command" class="rounded border border-[var(--primary-200)] bg-white p-3">
          <div class="text-sm font-semibold text-[var(--primary-800)]">{{ doc.command }}</div>
          <p class="mt-1 font-medium text-[var(--primary-700)]">{{ doc.description }}</p>
          <pre class="mt-2 overflow-auto rounded bg-[var(--primary-100)] p-2 text-[10px]">{{ JSON.stringify(doc.expectedPayload, null, 2) }}</pre>
          <p class="mt-2 text-[var(--primary-500)]">{{ doc.notes }}</p>
        </li>
      </ul>
    </section>

    <div class="grid gap-6 xl:grid-cols-[320px,1fr]">
      <aside class="card space-y-4">
        <div>
          <h2 class="text-sm font-semibold uppercase tracking-wide text-[var(--primary-500)]">
            Translation history
          </h2>
          <p class="text-xs text-[var(--primary-500)]">Resume or export prior jobs for consistency across matters.</p>
        </div>
        <ul class="space-y-2">
          <li v-for="job in jobs" :key="job.id">
            <button
              type="button"
              class="group flex w-full flex-col rounded-lg border border-transparent bg-white px-3 py-2 text-left transition hover:border-[var(--accent-400)]"
              :class="activeJob?.id === job.id ? 'border-[var(--accent-500)] shadow-sm' : ''"
              @click="activateJob(job)"
            >
              <div class="flex items-center justify-between text-sm font-semibold text-[var(--primary-800)]">
                <span>{{ job.sourceLanguage.toUpperCase() }} → {{ job.targetLanguage.toUpperCase() }}</span>
                <span class="text-xs text-[var(--primary-500)]">{{ new Date(job.lastUpdated).toLocaleString() }}</span>
              </div>
              <p class="mt-1 text-xs text-[var(--primary-600)]">
                {{ job.inputText.slice(0, 90) }}{{ job.inputText.length > 90 ? "..." : "" }}
              </p>
            </button>
          </li>
        </ul>

        <section class="rounded-lg border border-[var(--primary-200)] bg-[var(--primary-50)] p-3 text-xs text-[var(--primary-600)]">
          <h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-[var(--primary-500)]">
            Workflow tips
          </h3>
          <ul class="space-y-2">
            <li v-for="note in dashboardNotes" :key="note">{{ note }}</li>
          </ul>
        </section>
      </aside>

      <section class="card flex h-full flex-col space-y-4">
        <div class="grid gap-4 md:grid-cols-3">
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Source language
            <select v-model="sourceLanguage" class="input mt-1">
              <option value="auto">Auto-detect</option>
              <option value="en">English</option>
              <option value="fr">French</option>
              <option value="de">German</option>
              <option value="es">Spanish</option>
              <option value="zh">Chinese</option>
            </select>
          </label>
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Target language
            <select v-model="targetLanguage" class="input mt-1">
              <option value="en">English</option>
              <option value="es">Spanish</option>
              <option value="fr">French</option>
              <option value="de">German</option>
              <option value="zh">Chinese</option>
            </select>
          </label>
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Model
            <select v-model="selectedModel" class="input mt-1">
              <option
                v-for="option in modelOptions"
                :key="option.id"
                :value="option.id"
                :disabled="!option.available"
              >
                {{ option.label }} {{ option.available ? "" : "(add API key in Settings)" }}
              </option>
            </select>
          </label>
        </div>

        <div class="grid gap-4 md:grid-cols-2">
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Source text
            <textarea
              v-model="sourceText"
              class="input mt-1 h-64 resize-none"
              placeholder="Paste or type the passage you need translated..."
            />
          </label>
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Translation
            <textarea v-model="translatedText" class="input mt-1 h-64 resize-none" readonly />
          </label>
        </div>

        <div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <button class="btn-primary md:w-auto" type="button" @click="runTranslation" :disabled="isTranslating">
            <span v-if="isTranslating" class="flex items-center gap-2"><span class="spinner"></span>Translating...</span>
            <span v-else>Translate</span>
          </button>
          <div class="rounded-md border border-[var(--info)] bg-blue-50 px-3 py-2 text-xs text-[var(--info)]">
            Translation notes: terminology varies across jurisdictions. Engage a sworn translator for filings.
          </div>
        </div>
      </section>
    </div>
  </div>
</template>
