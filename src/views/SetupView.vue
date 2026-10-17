<script setup lang="ts">
// ABOUTME: First-launch setup page — checks Ollama status and guides the user.
// ABOUTME: Accessible at /setup, auto-redirected to on first launch if Ollama isn't ready.

import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import { health, ready, listModels } from "../api/sidecar";
import { setupVerified } from "../router";
// Setup only ever configures Ollama — it runs before any other mode is
// reachable — so it reads the local state directly rather than the current
// (possibly still-unread) backend mode.
import { CONFIDENTIALITY } from "../composables/backendMode";

type SetupState = "checking" | "no-ollama" | "no-models" | "ready";

const router = useRouter();
const state = ref<SetupState>("checking");
const models = ref<{ id: string; name: string }[]>([]);
const errorDetail = ref("");

async function checkStatus() {
  state.value = "checking";
  errorDetail.value = "";

  try {
    await health();
  } catch {
    state.value = "no-ollama";
    errorDetail.value = "Cannot reach Ollama. Make sure it is installed and running.";
    return;
  }

  try {
    const readyRes = await ready();
    if (readyRes.status !== "ready") {
      // Ollama answered but has no usable local model: ask for a pull, not an install.
      state.value = readyRes.checks?.models === "none" ? "no-models" : "no-ollama";
      // The sidecar's reason, e.g. a refused non-loopback OLLAMA_BASE_URL, not a guess that Ollama is down.
      errorDetail.value = readyRes.error
        ? `Ollama is not ready: ${readyRes.error}`
        : "Ollama is not responding. Make sure it is running.";
      return;
    }
  } catch {
    state.value = "no-ollama";
    errorDetail.value = "Ollama is not responding. Make sure it is running.";
    return;
  }

  try {
    const modelsRes = await listModels();
    if (!modelsRes.models || modelsRes.models.length === 0) {
      state.value = "no-models";
      return;
    }
    models.value = modelsRes.models;
    state.value = "ready";
  } catch {
    state.value = "no-models";
  }
}

function proceed() {
  setupVerified.value = true;
  router.push("/review");
}

onMounted(checkStatus);
</script>

<template>
  <div class="flex min-h-screen items-center justify-center bg-[var(--primary-50)] p-8">
    <div class="w-full max-w-lg space-y-8">

      <!-- Header -->
      <div class="text-center">
        <h1 class="text-3xl font-bold text-[var(--primary-900)]">Welcome to Rumble</h1>
        <p class="mt-2 text-[var(--primary-600)]">
          Local AI for legal work. {{ CONFIDENTIALITY.local.notice }}
        </p>
      </div>

      <!-- Checking state -->
      <div
        v-if="state === 'checking'"
        class="rounded-xl border border-[var(--primary-200)] bg-white p-8 text-center shadow-sm"
      >
        <div class="mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-[var(--primary-200)] border-t-[var(--accent-500)]" />
        <p class="text-[var(--primary-600)]">Checking your setup...</p>
      </div>

      <!-- Ollama not installed / not running -->
      <div
        v-else-if="state === 'no-ollama'"
        class="rounded-xl border border-[var(--primary-200)] bg-white p-8 shadow-sm"
      >
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold text-[var(--primary-900)]">Install Ollama</h2>
            <p class="mt-1 text-sm text-[var(--primary-500)]">
              Rumble uses Ollama to run AI models locally on your machine.
            </p>
          </div>

          <ol class="space-y-4 text-sm text-[var(--primary-700)]">
            <li class="flex gap-3">
              <span class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[var(--accent-100)] text-xs font-bold text-[var(--accent-700)]">1</span>
              <div>
                <p class="font-medium">Download Ollama</p>
                <p class="text-[var(--primary-500)]">
                  Visit <span class="font-mono text-[var(--accent-600)]">ollama.com</span> and install it for your platform.
                </p>
              </div>
            </li>
            <li class="flex gap-3">
              <span class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[var(--accent-100)] text-xs font-bold text-[var(--accent-700)]">2</span>
              <div>
                <p class="font-medium">Start Ollama</p>
                <p class="text-[var(--primary-500)]">
                  Open the Ollama app. It runs in the background — you should see its icon in your menu bar or system tray.
                </p>
              </div>
            </li>
            <li class="flex gap-3">
              <span class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[var(--accent-100)] text-xs font-bold text-[var(--accent-700)]">3</span>
              <div>
                <p class="font-medium">Come back here</p>
                <p class="text-[var(--primary-500)]">
                  Once Ollama is running, click the button below.
                </p>
              </div>
            </li>
          </ol>

          <p v-if="errorDetail" class="rounded-lg bg-red-50 px-4 py-2 text-sm text-red-700">
            {{ errorDetail }}
          </p>

          <button
            class="w-full rounded-lg bg-[var(--accent-500)] px-4 py-3 font-medium text-white transition hover:bg-[var(--accent-600)]"
            @click="checkStatus"
          >
            Check again
          </button>
        </div>
      </div>

      <!-- Ollama running but no models -->
      <div
        v-else-if="state === 'no-models'"
        class="rounded-xl border border-[var(--primary-200)] bg-white p-8 shadow-sm"
      >
        <div class="space-y-6">
          <div>
            <h2 class="text-xl font-semibold text-[var(--primary-900)]">Pull a model</h2>
            <p class="mt-1 text-sm text-[var(--primary-500)]">
              Ollama is running, but you need at least one AI model. We recommend <span class="font-mono font-medium">llama3.2</span> — it's fast, capable, and only 2 GB.
            </p>
          </div>

          <div class="rounded-lg border border-[var(--primary-200)] bg-[var(--primary-50)] p-4">
            <p class="mb-2 text-xs font-medium uppercase tracking-wide text-[var(--primary-500)]">
              Open a terminal and run:
            </p>
            <code class="block rounded bg-[var(--primary-900)] px-4 py-3 font-mono text-sm text-[var(--primary-100)]">
              ollama pull llama3.2
            </code>
            <p class="mt-2 text-xs text-[var(--primary-500)]">
              This downloads ~2 GB. It only needs to happen once.
            </p>
          </div>

          <p v-if="errorDetail" class="rounded-lg bg-red-50 px-4 py-2 text-sm text-red-700">
            {{ errorDetail }}
          </p>

          <button
            class="w-full rounded-lg bg-[var(--accent-500)] px-4 py-3 font-medium text-white transition hover:bg-[var(--accent-600)]"
            @click="checkStatus"
          >
            I've pulled a model — check again
          </button>
        </div>
      </div>

      <!-- Ready -->
      <div
        v-else-if="state === 'ready'"
        class="rounded-xl border border-[var(--primary-200)] bg-white p-8 shadow-sm"
      >
        <div class="space-y-6">
          <div class="text-center">
            <div class="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-full bg-green-100">
              <span class="text-2xl">&#10003;</span>
            </div>
            <h2 class="text-xl font-semibold text-[var(--primary-900)]">You're all set</h2>
            <p class="mt-1 text-sm text-[var(--primary-500)]">
              Ollama is running with {{ models.length }} model{{ models.length === 1 ? '' : 's' }} available.
            </p>
          </div>

          <ul class="space-y-1">
            <li
              v-for="model in models"
              :key="model.id"
              class="flex items-center gap-2 rounded px-3 py-1.5 text-sm text-[var(--primary-700)]"
            >
              <span class="h-1.5 w-1.5 rounded-full bg-green-500" />
              {{ model.name }}
            </li>
          </ul>

          <button
            class="w-full rounded-lg bg-[var(--accent-500)] px-4 py-3 font-medium text-white transition hover:bg-[var(--accent-600)]"
            @click="proceed"
          >
            Get started
          </button>
        </div>
      </div>

      <!-- Footer link -->
      <p class="text-center text-xs text-[var(--primary-400)]">
        You can return to this page anytime from Settings.
        <button
          v-if="state === 'no-ollama' || state === 'no-models'"
          class="ml-1 underline"
          @click="proceed"
        >
          Skip for now
        </button>
      </p>

    </div>
  </div>
</template>
