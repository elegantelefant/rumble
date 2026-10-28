<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";

defineProps<{ version: string }>();
const emit = defineEmits<{ (e: "close"): void }>();
const panel = ref<HTMLElement | null>(null);
const closeButton = ref<HTMLButtonElement | null>(null);
const error = ref("");
const supportUrl = "mailto:ping@elefant.legal";
const reportUrl = "https://github.com/elegantelefant/rumble/issues/new";
let previousFocus: HTMLElement | null = null;

async function openExternal(url: string) {
  error.value = "";
  try {
    await openUrl(url);
  } catch {
    error.value = "Could not open the link. Please try again.";
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
  } else if (event.key === "Tab") {
    const controls = panel.value?.querySelectorAll<HTMLElement>("a, button");
    if (!controls?.length) return;
    const first = controls[0];
    const last = controls[controls.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
}

onMounted(() => {
  previousFocus = document.activeElement as HTMLElement | null;
  closeButton.value?.focus();
});

onBeforeUnmount(() => previousFocus?.focus());
</script>

<template>
  <div
    class="fixed inset-0 z-[999] flex items-center justify-center bg-black/40 p-6"
    @click.self="emit('close')"
    @keydown.stop="onKeydown"
  >
    <section
      ref="panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="help-title"
      class="w-full max-w-md space-y-4 rounded-xl bg-white p-6 shadow-xl"
    >
      <h2 id="help-title" class="text-xl font-semibold">Help</h2>
      <p>{{ version }}</p>
      <p>Support: <a :href="supportUrl" class="underline" @click.prevent="openExternal(supportUrl)">ping@elefant.legal</a></p>
      <p><a :href="reportUrl" class="underline" @click.prevent="openExternal(reportUrl)">Report a problem</a></p>
      <p v-if="error" role="alert">{{ error }}</p>
      <button ref="closeButton" type="button" class="btn-secondary" @click="emit('close')">Close</button>
    </section>
  </div>
</template>
