<script setup lang="ts">
import { computed, onMounted } from "vue";
import { currentUser } from "../../composables/user";
import { backendMode, CONFIDENTIALITY, confidentialityOf, loadBackendMode } from "../../composables/backendMode";

const emit = defineEmits<{
  (e: "toggle-sidebar"): void;
  (e: "toggle-palette"): void;
}>();

// Reads the shared mode, so a switch in Settings updates the pill without a
// remount. Starts unknown rather than local — until the mode is read,
// claiming confidentiality would be a guess.
const confidentiality = computed(() => CONFIDENTIALITY[confidentialityOf(backendMode.value)]);
const confidentialityLabel = computed(() => confidentiality.value.label);
const confidentialityMessage = computed(() => confidentiality.value.message);

onMounted(loadBackendMode);

const initials = computed(() =>
  currentUser.value.name
    .split(/\s+/)
    .map((w) => w[0])
    .join("")
    .toUpperCase()
    .slice(0, 2),
);
</script>

<template>
  <header
    class="flex items-center justify-between border-b border-[var(--primary-300)] bg-[var(--white)] px-4 py-3 shadow-sm"
  >
    <div class="flex items-center gap-4">
      <button
        class="md:hidden flex h-10 w-10 items-center justify-center rounded-md border border-[var(--primary-300)] bg-transparent text-[var(--primary-700)] transition hover:text-[var(--accent-500)]"
        @click="emit('toggle-sidebar')"
        aria-label="Toggle navigation"
      >
        ☰
      </button>
      <span
        class="hidden md:inline-flex text-xs uppercase tracking-[0.32em] text-[var(--primary-500)]"
        :title="confidentialityMessage"
      >
        {{ confidentialityLabel }}
      </span>
    </div>

    <div class="flex items-center gap-4">
      <button
        class="hidden items-center gap-2 rounded-md border border-[var(--primary-300)] bg-white px-3 py-2 text-sm text-[var(--primary-600)] shadow-sm transition hover:border-[var(--accent-500)] hover:text-[var(--accent-600)] md:flex"
        @click="emit('toggle-palette')"
        aria-label="Open shortcuts"
      >
        <span class="font-medium" aria-hidden="true">⌘ K</span>
        <span>Shortcuts</span>
      </button>
      <div
        class="flex items-center gap-3 rounded-full border border-[var(--primary-200)] bg-[var(--primary-50)] px-3 py-1.5 text-sm text-[var(--primary-600)]"
        aria-label="Current user"
      >
        <div class="flex h-8 w-8 items-center justify-center rounded-full bg-[var(--primary-200)] text-[var(--primary-700)]">
          {{ initials }}
        </div>
        <div class="text-left leading-tight">
          <div class="font-medium text-[var(--primary-700)]">{{ currentUser.name }}</div>
          <div v-if="currentUser.team" class="text-xs text-[var(--primary-500)]">Team · {{ currentUser.team }}</div>
        </div>
      </div>
    </div>
  </header>
</template>
