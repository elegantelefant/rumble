<script setup lang="ts">
import { ref } from "vue";
import { useToast } from "../composables/toast";

const toasts = useToast();
const joined = ref(false);

function joinWaitlist() {
  if (joined.value) return;
  joined.value = true;
  toasts.addToast("You're on the Briefcases waitlist — we'll email you at launch.", "success");
}
</script>

<template>
  <div class="grid gap-6 lg:grid-cols-2">
    <section class="card space-y-4">
      <header>
        <h1 class="text-2xl font-semibold text-[var(--primary-900)]">Briefcases</h1>
        <p class="text-sm text-[var(--primary-600)]">
          Secure cloud backup for your legal workflows. Encrypted end-to-end with zero knowledge.
        </p>
      </header>
      <ul class="space-y-2 text-sm text-[var(--primary-600)]">
        <li>• Encrypted sync across devices</li>
        <li>• Team collaboration and shared workspaces</li>
        <li>• Priority support and monitoring</li>
      </ul>
      <div class="rounded-md border border-[var(--warning)] bg-yellow-50 px-3 py-2 text-sm text-[var(--warning)]">
        Premium Feature — $10/month after launch
      </div>
      <button class="btn-primary w-full" :disabled="joined" @click="joinWaitlist">
        {{ joined ? "You're on the list" : "Join Waitlist" }}
      </button>
    </section>

    <section class="card space-y-3">
      <h2 class="text-lg font-semibold text-[var(--primary-800)]">Local Briefcases</h2>
      <p class="text-sm text-[var(--primary-600)]">
        Local-only mode is always available. Create folders to organise matters and share within your team.
      </p>
      <div class="rounded-md border border-dashed border-[var(--primary-400)] bg-[var(--primary-200)] px-4 py-8 text-center text-sm text-[var(--primary-600)]">
        Coming soon: manage local briefcases here.
      </div>
    </section>
  </div>
</template>
