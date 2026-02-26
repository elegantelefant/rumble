<script setup lang="ts">
import { reactive, ref, inject } from "vue";

const toasts = inject<{ addToast: (message: string, type?: "success" | "error" | "info") => void }>("toast");

const templates = [
  { id: "employment", name: "Employment Agreement" },
  { id: "nda", name: "Non-Disclosure Agreement" },
  { id: "service", name: "Service Contract" },
];

const selectedTemplate = ref("employment");
const formState = reactive({
  employeeName: "",
  startDate: "",
  salary: "",
  position: "",
  terms: "",
});
const errors = reactive<Record<string, string>>({});
const isGenerating = ref(false);

function validateField(key: keyof typeof formState, label: string) {
  if (!formState[key]?.toString().trim()) {
    errors[key] = `${label} is required`;
  } else {
    delete errors[key];
  }
}

function validateForm() {
  validateField("employeeName", "Employee name");
  validateField("startDate", "Start date");
  validateField("salary", "Salary");
  validateField("position", "Position");
  return Object.keys(errors).length === 0;
}

function generateDraft() {
  if (!validateForm()) return;
  isGenerating.value = true;
  // TODO: invoke("draft_generate", { template: selectedTemplate.value, params: formState })
  setTimeout(() => {
    isGenerating.value = false;
    toasts?.addToast("Draft prepared. Review before sharing with clients.", "success");
  }, 1500);
}

function exportDraft(format: "word" | "pdf") {
  // TODO: invoke("draft_export", { format })
  toasts?.addToast(`Exported draft as ${format.toUpperCase()}.`, "info");
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
      <button class="btn-secondary">Manage Templates</button>
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
        <button class="btn-secondary mt-4 w-full">Browse Local Templates...</button>
      </aside>

      <section class="card space-y-4">
        <h2 class="caption-uppercase">Template: {{ selectedTemplate === "employment" ? "Employment Agreement" : "Selected" }}</h2>
        <form class="grid gap-4 md:grid-cols-2" @submit.prevent>
          <label class="space-y-1 text-sm">
            <span class="font-medium text-[var(--primary-700)]">Employee Name</span>
            <input
              v-model="formState.employeeName"
              class="input"
              placeholder="Full name"
              @blur="validateField('employeeName', 'Employee name')"
            />
            <p v-if="errors.employeeName" class="text-xs text-[var(--error)]">{{ errors.employeeName }}</p>
          </label>
          <label class="space-y-1 text-sm">
            <span class="font-medium text-[var(--primary-700)]">Start Date</span>
            <input
              v-model="formState.startDate"
              type="date"
              class="input"
              @blur="validateField('startDate', 'Start date')"
            />
            <p v-if="errors.startDate" class="text-xs text-[var(--error)]">{{ errors.startDate }}</p>
          </label>
          <label class="space-y-1 text-sm">
            <span class="font-medium text-[var(--primary-700)]">Salary</span>
            <input
              v-model="formState.salary"
              class="input"
              placeholder="$100,000"
              @blur="validateField('salary', 'Salary')"
            />
            <p v-if="errors.salary" class="text-xs text-[var(--error)]">{{ errors.salary }}</p>
          </label>
          <label class="space-y-1 text-sm">
            <span class="font-medium text-[var(--primary-700)]">Position</span>
            <input
              v-model="formState.position"
              class="input"
              placeholder="Role"
              @blur="validateField('position', 'Position')"
            />
            <p v-if="errors.position" class="text-xs text-[var(--error)]">{{ errors.position }}</p>
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

