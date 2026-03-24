<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
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
  try {
    // TODO: invoke("draft_generate", { template: selectedTemplate.value, params: formState })
    await new Promise((resolve) => setTimeout(resolve, 1500));
    toasts.addToast("Draft prepared. Review before sharing with clients.", "success");
  } catch (error) {
    console.error(error);
    toasts.addToast("Failed to generate draft. Please try again.", "error");
  } finally {
    isGenerating.value = false;
  }
}

function exportDraft(format: "word" | "pdf") {
  // TODO: invoke("draft_export", { format })
  toasts.addToast(`Exported draft as ${format.toUpperCase()}.`, "info");
  void format;
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
      </section>
    </div>
  </div>
</template>

