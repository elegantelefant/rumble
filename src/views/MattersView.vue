<!-- ABOUTME: The list of matters this launch of Rumble has open, plus starting a matter or joining with an invite link. -->
<!-- ABOUTME: A matter is a briefcase, as on the web app: versioned files kept in a folder the firm already syncs. -->
<script setup lang="ts">
defineOptions({ name: "MattersView" });
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { Briefcase } from "lucide-vue-next";
import { createMatter, errorText, joinMatter, listMatters, type MatterSummary } from "../api/sync";
import { useToast } from "../composables/toast";

const router = useRouter();
const { addToast } = useToast();
const matters = ref<MatterSummary[]>([]);
const yourName = ref("");
const joinLink = ref("");
const busy = ref(false);

async function load() {
  try {
    matters.value = await listMatters();
  } catch (error) {
    addToast(errorText(error), "error");
  }
}
onMounted(load);

async function open(step: () => Promise<MatterSummary | null>) {
  busy.value = true;
  try {
    const matter = await step();
    if (matter) router.push(`/matters/${matter.id}`);
  } catch (error) {
    addToast(errorText(error), "error");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="space-y-6">
    <header>
      <h1 class="h1">Matters</h1>
      <p class="body-muted">
        A matter holds versioned files, shared through a folder your firm already syncs, such as OneDrive, SharePoint
        or Google Drive. Rumble never contacts those services itself.
      </p>
    </header>

    <p class="card text-sm text-[var(--primary-600)]">
      Team sync is a preview. Rumble forgets its sync key when it quits, so matters you start or join now won't open
      after a restart.
    </p>

    <section class="card space-y-3">
      <h2 class="text-sm font-semibold text-[var(--primary-800)]">Your matters</h2>
      <p v-if="!matters.length" class="text-sm text-[var(--primary-500)]">No matters yet.</p>
      <ul v-else class="divide-y divide-[var(--primary-200)]">
        <li v-for="matter in matters" :key="matter.id">
          <button class="flex w-full items-center gap-3 py-2 text-left" type="button" @click="router.push(`/matters/${matter.id}`)">
            <Briefcase class="h-4 w-4 text-[var(--primary-500)]" />
            <span class="font-medium text-[var(--primary-800)]">{{ matter.name }}</span>
            <span v-if="matter.standing === 'waiting'" class="text-xs text-[var(--primary-500)]">
              Waiting for {{ matter.waitingFor ?? "an admin" }}
            </span>
          </button>
        </li>
      </ul>
    </section>

    <section class="card space-y-3">
      <label class="block text-sm text-[var(--primary-700)]">
        Your name, as colleagues will see it
        <input v-model="yourName" name="your-name" class="input mt-1 w-full" type="text" />
      </label>
      <div class="flex flex-wrap items-center gap-3">
        <button class="btn-primary" type="button" :disabled="busy || !yourName.trim()" @click="open(() => createMatter(yourName.trim()))">
          New matter
        </button>
        <span class="text-xs text-[var(--primary-500)]">You'll pick the synced folder to keep it in.</span>
      </div>
      <label class="block text-sm text-[var(--primary-700)]">
        Invite link from a colleague
        <textarea v-model="joinLink" name="join-link" class="input mt-1 w-full font-mono text-xs" rows="3" />
      </label>
      <button
        class="btn-secondary"
        type="button"
        :disabled="busy || !yourName.trim() || !joinLink.trim()"
        @click="open(() => joinMatter(joinLink.trim(), yourName.trim()))"
      >
        Join
      </button>
      <p class="text-xs text-[var(--primary-500)]">
        You'll pick the matter's folder, which your firm has already shared with you. The link opens nothing by itself.
      </p>
    </section>
  </div>
</template>
