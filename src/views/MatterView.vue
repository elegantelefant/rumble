<!-- ABOUTME: One matter: its files with numbered versions and authors, Open in Word, and the fork banner. -->
<!-- ABOUTME: Also joining: the invite link, the safety number both sides confirm, and the second-admin prompt. -->
<script setup lang="ts">
defineOptions({ name: "MatterView" });
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import ForkBanner from "../components/ForkBanner.vue";
import {
  addFile,
  approveRequest,
  combineFile,
  confirmNumber,
  createInvite,
  errorText,
  getMatter,
  makeAdmin,
  openFile,
  resolveFork,
  withdrawInvite,
  type FileView,
  type MatterDetail,
  type VersionView,
} from "../api/sync";
import { useToast } from "../composables/toast";

const props = defineProps<{ id: string }>();
const { addToast } = useToast();
const detail = ref<MatterDetail | null>(null);
const busy = ref(false);
const invitee = ref("");
const inviteLink = ref("");
const invitedName = ref("");
const dismissed = ref<string[]>([]);

const waiting = computed(() => detail.value?.matter.standing === "waiting");
const admin = computed(() => detail.value?.matter.standing === "admin");
const requests = computed(() => detail.value?.requests.filter((r) => !dismissed.value.includes(r.device)) ?? []);
const waitingFor = computed(() => detail.value?.matter.waitingFor ?? "an admin");

async function load() {
  try {
    detail.value = await getMatter(props.id);
  } catch (error) {
    addToast(errorText(error), "error");
  }
}

/** Saves made in Word are recorded when Rumble looks again, so look again on return to the window. */
onMounted(() => {
  void load();
  window.addEventListener("focus", load);
});
onBeforeUnmount(() => window.removeEventListener("focus", load));

async function run(step: () => Promise<void>) {
  busy.value = true;
  try {
    await step();
  } catch (error) {
    addToast(errorText(error), "error");
  } finally {
    busy.value = false;
    await load();
  }
}

const isWord = (file: FileView) => file.name.toLowerCase().endsWith(".docx");
const openLabel = (file: FileView) => (!file.openable ? "Show in folder" : isWord(file) ? "Open in Word" : "Open");
const forked = (file: FileView) => file.versions.filter((v) => v.head).length > 1;
const savedAt = (v: VersionView) => new Date(v.savedAtMs).toLocaleString();

const open = (file: FileView) =>
  run(async () => {
    if (await openFile(props.id, file.id)) {
      addToast(isWord(file) ? "Press Save in Word to share your changes." : "Press Save to share your changes.", "info");
    }
  });

const combine = (file: FileView) =>
  run(async () => {
    const opened = await combineFile(props.id, file.id);
    addToast(
      opened
        ? "Both versions are open. Bring the changes across, then press Save in yours."
        : "Both versions are shown in their folder. Bring the changes across, then save yours.",
      "info",
    );
  });

const use = (file: FileView, version: VersionView) =>
  run(async () => {
    await resolveFork(props.id, file.id, version.id);
    addToast("Saved as the next version. The other stays in the history.", "success");
  });

const add = () =>
  run(async () => {
    await addFile(props.id);
  });

const invite = () =>
  run(async () => {
    invitedName.value = invitee.value.trim();
    inviteLink.value = await createInvite(props.id, invitedName.value);
  });

const copyLink = () =>
  run(async () => {
    await navigator.clipboard.writeText(inviteLink.value);
    addToast("Invite link copied.", "success");
  });

const confirm = (number: string) =>
  run(async () => {
    await confirmNumber(props.id, number);
  });

const withdraw = (invite: string) =>
  run(async () => {
    await withdrawInvite(props.id, invite);
    addToast("Invite withdrawn. Send a fresh one to the person you meant.", "success");
  });

const approve = (device: string, number: string) =>
  run(async () => {
    await approveRequest(props.id, device, number);
  });

const promote = (device: string) =>
  run(async () => {
    await makeAdmin(props.id, device);
  });
</script>

<template>
  <div v-if="detail" class="space-y-6">
    <header>
      <RouterLink to="/matters" class="text-xs text-[var(--primary-500)]">Matters</RouterLink>
      <h1 class="h1">{{ detail.matter.name }}</h1>
      <p class="body-muted">{{ detail.matter.folder }}</p>
    </header>

    <section v-if="waiting" class="card space-y-3 text-sm text-[var(--primary-700)]">
      <p class="font-semibold text-[var(--primary-800)]">Waiting for {{ waitingFor }} to let you in.</p>
      <p>
        {{ waitingFor }} will see your request the next time Rumble is open on their laptop. When you talk, read this
        number to each other:
      </p>
      <p class="font-mono text-base tracking-wide text-[var(--primary-800)]">{{ detail.safetyNumber }}</p>
      <template v-if="!detail.confirmed && detail.safetyNumber">
        <button class="btn-primary" type="button" :disabled="busy" @click="confirm(detail.safetyNumber)">Numbers match</button>
        <p class="text-xs text-[var(--primary-500)]">Tap this only once you have both read out the same number.</p>
      </template>
      <p v-else-if="detail.confirmed">Thanks. This matter opens here once {{ waitingFor }} lets you in.</p>
    </section>

    <template v-else>
      <section v-for="invite in detail.contested" :key="invite" class="card space-y-3 text-sm text-[var(--primary-700)]">
        <p class="font-semibold text-[var(--primary-800)]">Two people answered this invite.</p>
        <p>Neither can be let in. Withdraw it, then send a fresh invite to the person you meant.</p>
        <button class="btn-secondary" type="button" :disabled="busy" @click="withdraw(invite)">Withdraw this invite</button>
      </section>

      <section v-if="detail.notRecorded.length" class="card text-sm text-[var(--primary-700)]" role="status">
        Your saved changes to {{ detail.notRecorded.join(", ") }} couldn't be added as a new version yet. They are kept
        in your working copy; Rumble tries again each time you come back.
      </section>

      <section v-for="request in requests" :key="request.device" class="card space-y-3 text-sm text-[var(--primary-700)]">
        <p class="font-semibold text-[var(--primary-800)]">{{ request.name }} is waiting to join {{ detail.matter.name }}.</p>
        <p>Call {{ request.name }} on a number you already know, and check you both see the same number.</p>
        <p class="font-mono text-base tracking-wide text-[var(--primary-800)]">{{ request.safetyNumber }}</p>
        <div class="flex flex-wrap gap-2">
          <button
            class="btn-primary"
            type="button"
            :disabled="busy || !request.safetyNumber"
            @click="approve(request.device, request.safetyNumber ?? '')"
          >
            Numbers match: let {{ request.name }} in
          </button>
          <button class="btn-secondary" type="button" @click="dismissed.push(request.device)">Not now</button>
        </div>
      </section>

      <section class="card space-y-4">
        <div class="flex items-center justify-between">
          <h2 class="text-sm font-semibold text-[var(--primary-800)]">Files</h2>
          <button class="btn-secondary" type="button" :disabled="busy" @click="add">Add file</button>
        </div>
        <p v-if="!detail.files.length" class="text-sm text-[var(--primary-500)]">No files yet.</p>
        <article v-for="file in detail.files" :key="file.id" class="space-y-2 border-t border-[var(--primary-200)] pt-3">
          <div class="flex items-center justify-between gap-3">
            <span class="font-medium text-[var(--primary-800)]">{{ file.name }}</span>
            <button v-if="!forked(file)" class="btn-secondary" type="button" :disabled="busy" @click="open(file)">
              {{ openLabel(file) }}
            </button>
          </div>
          <ForkBanner v-if="forked(file)" :file="file" :busy="busy" @combine="combine(file)" @use="use(file, $event)" />
          <ul class="space-y-1 text-xs text-[var(--primary-600)]">
            <li v-for="v in file.versions" :key="v.id">v{{ v.label }} · {{ v.mine ? "You" : v.author }} · {{ savedAt(v) }}</li>
          </ul>
        </article>
      </section>

      <section class="card space-y-3 text-sm text-[var(--primary-700)]">
        <h2 class="text-sm font-semibold text-[var(--primary-800)]">People</h2>
        <ul class="space-y-1">
          <li v-for="m in detail.members" :key="m.device" class="flex items-center gap-2">
            <span>{{ m.mine ? `${m.name} (you)` : m.name }}</span>
            <span v-if="m.admin" class="text-xs text-[var(--primary-500)]">admin</span>
            <button
              v-else-if="admin && detail.needsSecondAdmin"
              class="btn-secondary text-xs"
              type="button"
              :disabled="busy"
              @click="promote(m.device)"
            >
              Make {{ m.name }} an admin
            </button>
          </li>
        </ul>
        <p v-if="detail.needsSecondAdmin">
          Make a colleague a second admin before inviting anyone else. If this laptop is lost, nobody can add or remove
          people in this matter.
        </p>
        <template v-else-if="admin">
          <label class="block">
            Invite someone
            <input v-model="invitee" name="invitee" class="input mt-1 w-full" type="text" placeholder="Their name" />
          </label>
          <button class="btn-secondary" type="button" :disabled="busy || !invitee.trim()" @click="invite">Create invite link</button>
          <template v-if="inviteLink">
            <textarea name="invite-link" class="input w-full font-mono text-xs" rows="3" readonly :value="inviteLink" />
            <button class="btn-secondary text-xs" type="button" @click="copyLink">Copy link</button>
            <p class="text-xs text-[var(--primary-500)]">
              Send this to {{ invitedName }}. It opens nothing by itself: they ask to join, then you compare a number on a call
              before they're in.
            </p>
          </template>
        </template>
      </section>
    </template>
  </div>
</template>
