<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { createDraftJob, waitForJob } from "../api/sidecar";
import { useToast } from "../composables/toast";

const toasts = useToast();

type FieldConfig = { key: string; label: string; type?: string; placeholder?: string };

const templateFields: Record<string, FieldConfig[]> = {
  employment: [
    { key: "employeeName", label: "Employee Name", placeholder: "Full name" },
    { key: "startDate", label: "Start Date", type: "date" },
    { key: "salary", label: "Salary", placeholder: "$100,000" },
    { key: "position", label: "Position", placeholder: "Role" },
  ],
  nda: [
    { key: "disclosingParty", label: "Disclosing Party", placeholder: "Company or person" },
    { key: "receivingParty", label: "Receiving Party", placeholder: "Company or person" },
    { key: "effectiveDate", label: "Effective Date", type: "date" },
    { key: "duration", label: "Duration", placeholder: "e.g. 2 years" },
  ],
  service: [
    { key: "serviceProvider", label: "Service Provider", placeholder: "Provider name" },
    { key: "clientName", label: "Client Name", placeholder: "Client name" },
    { key: "startDate", label: "Start Date", type: "date" },
    { key: "scopeOfWork", label: "Scope of Work", placeholder: "Brief description" },
  ],
};

const templates = [
  { id: "employment", name: "Employment Agreement" },
  { id: "nda", name: "Non-Disclosure Agreement" },
  { id: "service", name: "Service Contract" },
];

const selectedTemplate = ref("employment");
const formState = reactive<Record<string, string>>({});
const errors = reactive<Record<string, string>>({});
const isGenerating = ref(false);
const draftResult = ref("");
const draftWarnings = ref<string[]>([]);

const activeFields = computed(() => templateFields[selectedTemplate.value] ?? []);
const activeTemplateName = computed(
  () => templates.find((t) => t.id === selectedTemplate.value)?.name ?? "Selected",
);

watch(selectedTemplate, () => {
  Object.keys(formState).forEach((k) => delete formState[k]);
  Object.keys(errors).forEach((k) => delete errors[k]);
  formState.terms = "";
});

function sentenceCase(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1).toLowerCase();
}

function validateField(key: string, label: string) {
  if (!formState[key]?.toString().trim()) {
    errors[key] = `${sentenceCase(label)} is required`;
  } else {
    delete errors[key];
  }
}

function validateForm() {
  for (const field of activeFields.value) {
    validateField(field.key, field.label);
  }
  return Object.keys(errors).length === 0;
}

async function generateDraft() {
  if (!validateForm()) return;
  isGenerating.value = true;
  draftResult.value = "";
  draftWarnings.value = [];
  try {
    const fieldSummary = activeFields.value
      .map((f) => `${f.label}: ${formState[f.key] ?? ""}`)
      .filter((line) => !line.endsWith(": "))
      .join("\n");

    const prompt = [
      `Draft a ${activeTemplateName.value} with the following details:`,
      fieldSummary,
      formState.terms?.trim() ? `\nAdditional terms: ${formState.terms.trim()}` : "",
    ].filter(Boolean).join("\n");

    const jobResponse = await createDraftJob({
      prompt,
      document_type: selectedTemplate.value,
    });

    const result = await waitForJob("draft", jobResponse.job_id);

    if (result.status === "failed") {
      throw new Error("Draft generation failed — the AI could not produce a draft.");
    }

    // The server's own contract doesn't guarantee these types (#49) -- a
    // wrong-typed `warnings` would pass a plain `?? []` null guard, and
    // iterating a string in the template's v-for renders one bullet per
    // character instead of failing visibly.
    const payload = result.result as { draft?: unknown; warnings?: unknown } | undefined;
    draftResult.value = typeof payload?.draft === "string" ? payload.draft : "";
    draftWarnings.value = Array.isArray(payload?.warnings) ? payload.warnings : [];

    if (draftResult.value) {
      toasts.addToast("Draft prepared. Review before sharing with clients.", "success");
    } else {
      toasts.addToast("Draft completed but returned no content.", "info");
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to generate draft.";
    toasts.addToast(message, "error");
  } finally {
    isGenerating.value = false;
  }
}

async function exportDraft(format: "word" | "pdf") {
  if (!draftResult.value) {
    toasts.addToast("Generate a draft before exporting.", "info");
    return;
  }

  if (format === "pdf") {
    toasts.addToast("PDF export is coming soon.", "info");
    return;
  }

  try {
    const wrote = await invoke<boolean>("export_draft_docx", { text: draftResult.value });
    if (wrote) {
      toasts.addToast("Draft exported as Word document.", "success");
    }
  } catch (error) {
    // Tauri rejects with the raw Err(String) payload, not an Error object, so
    // `instanceof Error` never matches and the specific Rust message is lost.
    const message = typeof error === "string" ? error : String(error);
    toasts.addToast(message, "error");
  }
}
</script>

<template>
  <div class="space-y-6">
    <header class="flex items-center justify-between">
      <div>
        <h1 class="h1">Document Draft</h1>
        <p class="body-muted">Generate drafts from your local templates.</p>
      </div>
      <button class="btn-secondary" type="button" @click="toasts.addToast('Template management coming soon.', 'info')">Manage Templates</button>
    </header>

    <div class="grid gap-6 lg:grid-cols-[2fr,3fr]">
      <aside class="card">
        <h2 class="caption-uppercase">Template Library</h2>
        <div class="mt-3 space-y-2">
          <button
            v-for="template in templates"
            :key="template.id"
            class="w-full rounded-md px-3 py-2 text-left text-sm transition"
            :class="[
              selectedTemplate === template.id
                ? 'bg-[var(--primary-200)] border border-[var(--accent-500)]'
                : 'border border-transparent hover:bg-[var(--primary-100)]',
            ]"
            @click="selectedTemplate = template.id"
          >
            {{ template.name }}
          </button>
        </div>
        <button class="btn-secondary mt-4 w-full" type="button" @click="toasts.addToast('Template browsing coming soon.', 'info')">Browse Local Templates...</button>
      </aside>

      <section class="card space-y-4">
        <h2 class="caption-uppercase">Template: {{ activeTemplateName }}</h2>
        <form class="grid gap-4 md:grid-cols-2" @submit.prevent="generateDraft">
          <label v-for="field in activeFields" :key="field.key" class="space-y-1 text-sm">
            <span class="font-medium text-[var(--primary-700)]">{{ field.label }}</span>
            <input
              v-model="formState[field.key]"
              :type="field.type ?? 'text'"
              class="input"
              :placeholder="field.placeholder"
              @blur="validateField(field.key, field.label)"
            />
            <p v-if="errors[field.key]" class="text-xs text-[var(--error)]">{{ errors[field.key] }}</p>
          </label>
        </form>

        <div class="space-y-2">
          <span class="text-sm font-medium text-[var(--primary-700)]">Additional Terms</span>
          <textarea
            v-model="formState.terms"
            class="input h-32 resize-none"
            placeholder="Enter additional clauses or notes"
          />
          <p class="text-xs text-[var(--primary-500)]">Sensitive data stays local; nothing leaves your machine.</p>
        </div>

        <div class="flex flex-wrap gap-3 pt-2">
          <button class="btn-primary" :disabled="isGenerating" @click="generateDraft">
            <span v-if="!isGenerating">Generate Draft</span>
            <span v-else class="flex items-center gap-2"><span class="spinner"></span>Preparing…</span>
          </button>
          <button class="btn-secondary" @click="exportDraft('word')">Export to Word</button>
          <button class="btn-secondary" @click="exportDraft('pdf')">Export to PDF</button>
        </div>

        <div v-if="draftWarnings.length" class="rounded-md border border-yellow-300 bg-yellow-50 p-3 text-sm text-yellow-800">
          <p class="font-medium">Warnings:</p>
          <ul class="mt-1 list-inside list-disc">
            <li v-for="(w, i) in draftWarnings" :key="i">{{ w }}</li>
          </ul>
        </div>

        <div v-if="draftResult" class="space-y-2">
          <h3 class="caption-uppercase">Generated Draft</h3>
          <pre class="whitespace-pre-wrap rounded-md border border-[var(--primary-200)] bg-[var(--primary-50)] p-4 text-sm leading-relaxed">{{ draftResult }}</pre>
        </div>
      </section>
    </div>
  </div>
</template>

