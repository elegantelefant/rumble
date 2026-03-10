<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import BrandLogo from "../components/BrandLogo.vue";

const passphrase = ref("");
const remember = ref(true);
const loading = ref(false);
const error = ref<string | null>(null);

const router = useRouter();

async function handleSubmit() {
  if (!passphrase.value.trim()) {
    error.value = "Passphrase required";
    return;
  }

  error.value = null;
  loading.value = true;

  try {
    // TODO: invoke("auth_login", { passphrase: passphrase.value, remember: remember.value })
    await new Promise((resolve) => setTimeout(resolve, 500));
    router.push("/review");
  } catch (err) {
    error.value = `Login failed: ${err}`;
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="min-h-screen bg-[var(--primary-100)] flex items-center justify-center px-4 py-12">
    <div class="card w-full max-w-md space-y-6">
      <div class="flex flex-col items-center space-y-3 text-center">
        <BrandLogo />
        <div class="text-xs uppercase tracking-[0.32em] text-[var(--primary-500)]">Secure access</div>
        <div class="body-muted">Enter Rumble by Elefant — all data stays on your machine.</div>
      </div>

      <form class="space-y-4" @submit.prevent="handleSubmit">
        <div class="space-y-2">
          <label class="text-sm font-medium text-[var(--primary-700)]" for="passphrase">
            Passphrase
          </label>
          <input
            id="passphrase"
            v-model="passphrase"
            class="input"
            type="password"
            placeholder="Enter your passphrase"
            autofocus
          />
        </div>

        <label class="flex items-center gap-2 text-sm text-[var(--primary-600)]">
          <input type="checkbox" v-model="remember" class="h-4 w-4 rounded border-[var(--primary-400)]" />
          Remember for 30 days
        </label>

        <button type="submit" class="btn-primary w-full" :disabled="loading">
          <span v-if="!loading">Enter Rumble</span>
          <span v-else class="loading">Authenticating...</span>
        </button>
      </form>

      <div v-if="error" class="rounded-md border border-[var(--error)] bg-red-50 px-3 py-2 text-sm text-[var(--error)]">
        {{ error }}
      </div>

      <button class="w-full text-sm text-[var(--accent-600)] hover:text-[var(--accent-500)]">
        First time? Generate phrase
      </button>
    </div>
  </div>
</template>

