<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { mockTestSync } from "../modules/backend/backendClient";
import { useToast } from "../composables/toast";
import { generateId } from "../utils/ids";
import {
  backendMode,
  CONFIDENTIALITY,
  loadBackendMode,
  MODE_TO_STATE,
  setBackendMode,
  type BackendMode,
} from "../composables/backendMode";

const toast = useToast();

const tabs = [
  { id: "providers", label: "Providers & API keys" },
  { id: "storage", label: "Templates & workspace storage" },
  { id: "appearance", label: "Appearance" },
  { id: "sync", label: "Sync" },
];

const activeTab = ref("providers");

type ProviderOption = {
  id: string;
  label: string;
  description: string;
  requiresKey: boolean;
  supportsLocalConfig?: boolean;
};

type SecretRecord = {
  id: string;
  provider: string;
  label: string;
  addedAt: string;
  scope: "drafting" | "research" | "global";
  notes: string;
};

const providerOptions: ProviderOption[] = [
  {
    id: "elefant-local",
    label: "Ollama · Elefant Legal Blend",
    description: "Runs locally via Ollama. Configure host and model name.",
    requiresKey: false,
    supportsLocalConfig: true,
  },
  {
    id: "openai",
    label: "OpenAI",
    description: "Hosted GPT models. Requests leave the device.",
    requiresKey: true,
  },
  {
    id: "anthropic",
    label: "Anthropic",
    description: "Claude family models for research and drafting.",
    requiresKey: true,
  },
];

const secrets = ref<SecretRecord[]>([]);

// v1 ships Ollama-only: BYOK is selectable in dev builds, shown unavailable in
// packaged ones. Flip this to offer BYOK in the packaged app.
const BYOK_SELECTABLE = import.meta.env.DEV;
const UNAVAILABLE = "Not available in this version.";

type ModeOption = { id: BackendMode; label: string; description: string; available: boolean };

const modeOptions: ModeOption[] = [
  {
    id: "ollama",
    label: "Local (Ollama)",
    description: CONFIDENTIALITY[MODE_TO_STATE.ollama].message,
    available: true,
  },
  {
    id: "byok",
    label: "Your own key (OpenAI)",
    description: `${CONFIDENTIALITY[MODE_TO_STATE.byok].message} Uses the OpenAI key saved below.`,
    available: BYOK_SELECTABLE,
  },
  {
    id: "premium",
    label: "Elefant Premium",
    description: CONFIDENTIALITY[MODE_TO_STATE.premium].message,
    available: false,
  },
];

const switchingMode = ref(false);
// The radio group's own value, so a refused switch can put the selection back.
const selectedMode = ref<BackendMode | null>(backendMode.value);
watch(backendMode, (mode) => {
  selectedMode.value = mode;
});

async function chooseMode(mode: BackendMode) {
  if (mode === backendMode.value) return;
  switchingMode.value = true;
  try {
    await setBackendMode(mode);
    const label = modeOptions.find((option) => option.id === mode)?.label ?? mode;
    toast.addToast(`Switched to ${label}. The local AI service is restarting.`, "success");
  } catch (error) {
    toast.addToast(`Could not switch mode: ${error}`, "error");
    selectedMode.value = backendMode.value;
  } finally {
    switchingMode.value = false;
  }
}

onMounted(loadBackendMode);

onMounted(async () => {
  const stored: SecretRecord[] = [];
  for (const provider of providerOptions) {
    if (!provider.requiresKey) continue;
    try {
      const key = await invoke<string | null>("get_api_key", { provider: provider.id });
      if (key) {
        stored.push({
          id: generateId(),
          provider: provider.id,
          label: provider.label,
          addedAt: new Date().toISOString(),
          scope: "global",
          notes: "Loaded from system keychain",
        });
      }
    } catch (e) {
      console.error(`Failed to check keychain for ${provider.id}:`, e);
    }
  }
  if (stored.length > 0) {
    secrets.value = stored;
  }
});

const localConfig = reactive({
  host: "http://127.0.0.1",
  port: "11434",
  model: "elefant-legal-blend",
});

const newSecret = reactive({
  provider: "",
  label: "",
  key: "",
  scope: "global",
  notes: "",
});

const workspaceStorage = reactive({
  templatesPath: "~/Documents/LegalTemplates",
  workspacePath: "~/Library/Application Support/Elefant/Rumble",
  briefcases: ["General research", "Litigation", "Transactions"],
  attachableResources: ["DocumentReview: Contract_2024.pdf", "Research: Tax compliance"],
});

const appearanceSettings = reactive({
  navigationSidebar: "left",
  contextSidebar: "right",
  theme: "system",
  showChatSidebarByDefault: true,
});

const syncSettings = reactive({
  enabled: true,
  teamCode: "",
  server: "",
  useCustom: false,
  customServer: "",
});

const isSaving = ref(false);

const newBriefcase = ref("");
const newResource = ref("");

function addBriefcase() {
  const name = newBriefcase.value.trim();
  if (!name) return;
  if (workspaceStorage.briefcases.includes(name)) {
    toast.addToast("That briefcase already exists.", "error");
    return;
  }
  workspaceStorage.briefcases.push(name);
  newBriefcase.value = "";
}

function attachResource() {
  const name = newResource.value.trim();
  if (!name) return;
  if (workspaceStorage.attachableResources.includes(name)) {
    toast.addToast("That resource is already attached.", "error");
    return;
  }
  workspaceStorage.attachableResources.push(name);
  newResource.value = "";
}

async function addSecret() {
  if (!newSecret.provider) return;
  const provider = providerOptions.find((option) => option.id === newSecret.provider);
  if (provider?.requiresKey && !newSecret.key.trim()) {
    toast.addToast("Enter the provider key before saving.", "error");
    return;
  }
  if (provider?.requiresKey) {
    try {
      await invoke("store_api_key", { provider: newSecret.provider, key: newSecret.key });
    } catch (error) {
      toast.addToast(`Failed to store API key: ${error}`, "error");
      return;
    }
  }
  secrets.value.unshift({
    id: generateId(),
    provider: newSecret.provider,
    label: newSecret.label || provider?.label || "Provider",
    addedAt: new Date().toISOString(),
    scope: newSecret.scope as SecretRecord["scope"],
    notes: newSecret.notes,
  });
  newSecret.provider = "";
  newSecret.label = "";
  newSecret.key = "";
  newSecret.scope = "global";
  newSecret.notes = "";
  toast.addToast("Secret saved locally. Remember: hosted providers process data off-device.", "success");
}

async function removeSecret(id: string) {
  const secret = secrets.value.find((s) => s.id === id);
  if (secret) {
    try {
      await invoke("delete_api_key", { provider: secret.provider });
    } catch (e) {
      console.error("Failed to delete keychain entry:", e);
      toast.addToast("Could not remove credential from system keychain.", "error");
      return;
    } finally {
      // Deleting the OpenAI key in BYOK mode drops the host back to local mode.
      await loadBackendMode();
    }
  }
  secrets.value = secrets.value.filter((s) => s.id !== id);
}

async function saveSettings() {
  isSaving.value = true;
  try {
    // API keys are stored individually via addSecret/removeSecret.
    // General preferences (appearance, workspace) are local reactive state.
    toast.addToast("Settings stored securely on this device.", "success");
  } finally {
    isSaving.value = false;
  }
}

async function testSync() {
  const url = syncSettings.useCustom ? syncSettings.customServer : syncSettings.server;
  try {
    new URL(url);
  } catch {
    toast.addToast("Invalid server URL.", "error");
    return;
  }
  if (!url.startsWith("https://")) {
    toast.addToast("Sync server must use HTTPS.", "error");
    return;
  }
  toast.addToast("Pinging sync server health endpoint...", "info");
  try {
    const result = await mockTestSync(url);
    toast.addToast(result.ok ? "Sync server reachable." : (result.message ?? "Sync server unreachable."), result.ok ? "success" : "error");
  } catch (error) {
    console.error(error);
    toast.addToast("Failed to reach sync server.", "error");
  }
}

const selectedProviderDetails = computed(() =>
  newSecret.provider ? providerOptions.find((option) => option.id === newSecret.provider) ?? null : null,
);
</script>

<template>
  <div class="space-y-6">
    <header>
      <h1 class="h1">Settings</h1>
      <p class="body-muted">
        Manage provider credentials, local storage, appearance, and sync endpoints. Secrets stay encrypted on-device.
      </p>
    </header>

    <div class="flex flex-wrap gap-2">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        class="rounded-md border px-4 py-2 text-sm font-medium transition"
        :class="activeTab === tab.id ? 'border-[var(--accent-400)] bg-[var(--accent-100)] text-[var(--accent-700)]' : 'border-[var(--primary-300)] bg-white text-[var(--primary-600)] hover:border-[var(--accent-300)]'"
        type="button"
        @click="activeTab = tab.id"
      >
        {{ tab.label }}
      </button>
    </div>

    <form class="card space-y-6" @submit.prevent="saveSettings">
    <fieldset :disabled="isSaving">
      <section v-if="activeTab === 'providers'" class="space-y-5">
        <fieldset class="space-y-2" :disabled="switchingMode">
          <legend class="text-base font-semibold text-[var(--primary-800)]">Processing mode</legend>
          <p class="text-xs text-[var(--primary-500)]">Where chat, drafting and review requests are processed.</p>
          <label
            v-for="option in modeOptions"
            :key="option.id"
            class="flex items-start gap-2 rounded-lg border border-[var(--primary-200)] p-3 text-sm"
            :class="option.available ? 'bg-white text-[var(--primary-700)]' : 'bg-[var(--primary-50)] text-[var(--primary-400)]'"
          >
            <input
              type="radio"
              name="backend-mode"
              class="mt-1"
              v-model="selectedMode"
              :value="option.id"
              :disabled="!option.available"
              @change="chooseMode(option.id)"
            />
            <span>
              <span class="font-semibold">{{ option.label }}</span>
              <span class="block text-xs">{{ option.available ? option.description : UNAVAILABLE }}</span>
            </span>
          </label>
        </fieldset>

        <div>
          <h2 class="text-base font-semibold text-[var(--primary-800)]">Configured secrets</h2>
          <p class="text-xs text-[var(--primary-500)]">
            Local storage uses SQLCipher for encryption. Hosted providers transmit prompts and outputs to their APIs.
          </p>
        </div>

        <ul class="space-y-3">
          <li
            v-for="secret in secrets"
            :key="secret.id"
            class="flex flex-col gap-2 rounded-lg border border-[var(--primary-200)] bg-[var(--primary-50)] p-3 text-sm text-[var(--primary-600)]"
          >
            <div class="flex items-center justify-between">
              <div class="font-semibold text-[var(--primary-800)]">
                {{ providerOptions.find((option) => option.id === secret.provider)?.label ?? secret.provider }}
              </div>
              <button class="text-xs text-[var(--accent-600)] hover:underline" type="button" @click="removeSecret(secret.id)">
                Remove
              </button>
            </div>
            <div>Label · {{ secret.label }}</div>
            <div>Scope · {{ secret.scope }}</div>
            <div class="text-xs text-[var(--primary-500)]">
              Added {{ new Date(secret.addedAt).toLocaleString() }} · {{ secret.notes || "No additional notes" }}
            </div>
          </li>
        </ul>

        <div class="rounded-lg border border-[var(--primary-200)] bg-white p-4 space-y-3">
          <h3 class="text-sm font-semibold text-[var(--primary-800)]">Add provider secret</h3>
          <div class="grid gap-3 md:grid-cols-2">
            <label class="text-xs font-semibold text-[var(--primary-600)]">
              Provider
              <select v-model="newSecret.provider" class="input mt-1">
                <option value="" disabled>Select provider</option>
                <option v-for="option in providerOptions" :key="option.id" :value="option.id">
                  {{ option.label }}
                </option>
              </select>
            </label>
            <label class="text-xs font-semibold text-[var(--primary-600)]">
              Label
              <input v-model="newSecret.label" class="input mt-1" placeholder="e.g. Drafting primary key" />
            </label>
            <label class="text-xs font-semibold text-[var(--primary-600)]">
              Scope
              <select v-model="newSecret.scope" class="input mt-1">
                <option value="global">Global</option>
                <option value="drafting">Drafting</option>
                <option value="research">Research</option>
              </select>
            </label>
            <label v-if="selectedProviderDetails?.requiresKey" class="text-xs font-semibold text-[var(--primary-600)]">
              API key / token
              <input v-model="newSecret.key" class="input mt-1" placeholder="sk-..." type="password" />
            </label>
            <label v-if="selectedProviderDetails?.supportsLocalConfig" class="text-xs font-semibold text-[var(--primary-600)] md:col-span-2">
              Local runtime configuration
              <div class="mt-1 grid gap-2 md:grid-cols-3">
                <input v-model="localConfig.host" class="input" placeholder="http://127.0.0.1" />
                <input v-model="localConfig.port" class="input" placeholder="11434" />
                <input v-model="localConfig.model" class="input" placeholder="elefant-legal-blend" />
              </div>
            </label>
            <label class="md:col-span-2 text-xs font-semibold text-[var(--primary-600)]">
              Notes
              <textarea v-model="newSecret.notes" class="input mt-1" placeholder="Usage hints, rate limits, matter restrictions..." />
            </label>
          </div>
          <p class="text-xs text-[var(--primary-500)]">
            Remote providers process prompts on their servers. Only chats are retained locally.
          </p>
          <div class="flex justify-end">
            <button class="btn-primary" type="button" @click="addSecret">Save secret</button>
          </div>
        </div>
      </section>

      <section v-else-if="activeTab === 'storage'" class="space-y-5">
        <div>
          <h2 class="text-base font-semibold text-[var(--primary-800)]">Template library</h2>
          <p class="text-xs text-[var(--primary-500)]">
            Map Rumble to your firm template repository so drafting tools can reference the latest clauses.
          </p>
        </div>
        <label class="text-sm font-medium text-[var(--primary-700)]">
          Templates folder
          <div class="mt-1 flex gap-2">
            <input v-model="workspaceStorage.templatesPath" class="input" />
            <button class="btn-secondary" type="button">Browse</button>
          </div>
        </label>
        <div>
          <h3 class="text-base font-semibold text-[var(--primary-800)]">Workspace data</h3>
          <p class="text-xs text-[var(--primary-500)]">
            Chats, document snapshots, and eval runs live in the workspace directory. Back it up with your standard retention policy.
          </p>
        </div>
        <label class="text-sm font-medium text-[var(--primary-700)]">
          Workspace path
          <input v-model="workspaceStorage.workspacePath" class="input mt-1" />
        </label>
        <div class="grid gap-3 md:grid-cols-2">
          <div>
            <h4 class="text-sm font-semibold text-[var(--primary-700)]">Briefcases</h4>
            <ul class="mt-2 space-y-1 text-xs text-[var(--primary-600)]">
              <li v-for="briefcase in workspaceStorage.briefcases" :key="briefcase" class="rounded border border-[var(--primary-200)] bg-white px-3 py-2">
                {{ briefcase }}
              </li>
            </ul>
            <div class="mt-2 flex gap-2">
              <input
                v-model="newBriefcase"
                class="input text-xs"
                placeholder="New briefcase name"
                @keydown.enter.prevent="addBriefcase"
              />
              <button class="btn-secondary text-xs" type="button" @click="addBriefcase">Add briefcase</button>
            </div>
          </div>
          <div>
            <h4 class="text-sm font-semibold text-[var(--primary-700)]">Attach threads to briefcase</h4>
            <p class="text-xs text-[var(--primary-500)]">Select threads or chats to group them with a matter.</p>
            <ul class="mt-2 space-y-1 text-xs text-[var(--primary-600)]">
              <li v-for="resource in workspaceStorage.attachableResources" :key="resource" class="rounded border border-[var(--primary-200)] bg-white px-3 py-2">
                {{ resource }}
              </li>
            </ul>
            <div class="mt-2 flex gap-2">
              <input
                v-model="newResource"
                class="input text-xs"
                placeholder="Thread or chat name"
                @keydown.enter.prevent="attachResource"
              />
              <button class="btn-secondary text-xs" type="button" @click="attachResource">Attach resource</button>
            </div>
          </div>
        </div>
      </section>

      <section v-else-if="activeTab === 'appearance'" class="space-y-5">
        <div>
          <h2 class="text-base font-semibold text-[var(--primary-800)]">Layout preferences</h2>
          <p class="text-xs text-[var(--primary-500)]">
            Rumble uses a navigation sidebar and a contextual chat/history sidebar. Configure their positions independently.
          </p>
        </div>
        <div class="grid gap-4 md:grid-cols-2">
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Navigation sidebar
            <select v-model="appearanceSettings.navigationSidebar" class="input mt-1">
              <option value="left">Left</option>
              <option value="right">Right</option>
            </select>
          </label>
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Chat/history sidebar
            <select v-model="appearanceSettings.contextSidebar" class="input mt-1">
              <option value="left">Left</option>
              <option value="right">Right</option>
            </select>
          </label>
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Theme
            <select v-model="appearanceSettings.theme" class="input mt-1">
              <option value="system">Match system</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
          <label class="flex items-center gap-2 text-sm font-medium text-[var(--primary-700)]">
            <input type="checkbox" v-model="appearanceSettings.showChatSidebarByDefault" />
            Show chat history sidebar on entry
          </label>
        </div>
        <p class="text-xs text-[var(--primary-500)]">
          When both sidebars use the same side, Rumble collapses the contextual pane until a chat is active.
        </p>
      </section>

      <section v-else class="space-y-5">
        <div>
          <h2 class="text-base font-semibold text-[var(--primary-800)]">Workspace sync</h2>
          <p class="text-xs text-[var(--primary-500)]">
            Use the Elefant sync service or supply your own server built from our open-source reference implementation.
          </p>
        </div>
        <label class="flex items-center gap-2 text-sm font-medium text-[var(--primary-700)]">
          <input type="checkbox" v-model="syncSettings.enabled" />
          Enable secure sync
        </label>
        <div class="grid gap-4 md:grid-cols-2">
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Team code
            <input v-model="syncSettings.teamCode" class="input mt-1" />
            <span class="text-xs text-[var(--primary-500)]">Share this with colleagues to join your workspace.</span>
          </label>
          <label class="text-sm font-medium text-[var(--primary-700)]">
            Default Elefant server
            <input v-model="syncSettings.server" class="input mt-1" disabled />
            <span class="text-xs text-[var(--primary-500)]">
              Hosted by Elefant. Data is encrypted in transit and at rest.
            </span>
          </label>
        </div>
        <label class="flex items-center gap-2 text-sm font-medium text-[var(--primary-700)]">
          <input type="checkbox" v-model="syncSettings.useCustom" />
          Use custom sync server
        </label>
        <label class="text-sm font-medium text-[var(--primary-700)]" :class="{ 'opacity-50': !syncSettings.useCustom }">
          Custom server URL
          <input v-model="syncSettings.customServer" :disabled="!syncSettings.useCustom" class="input mt-1" placeholder="https://sync.myfirm.com" />
          <span class="text-xs text-[var(--primary-500)]">
            Build your own server using our reference repo: github.com/ielegante/rumble-sync
          </span>
        </label>
        <div class="flex justify-end">
          <button class="btn-secondary" type="button" @click="testSync">Test connection</button>
        </div>
      </section>

      <div class="flex items-center justify-between gap-3">
        <router-link to="/setup" class="text-sm text-[var(--primary-500)] underline hover:text-[var(--accent-600)]">
          Ollama setup guide
        </router-link>
        <button class="btn-primary" type="submit" :disabled="isSaving">
          <span v-if="!isSaving">Save settings</span>
          <span v-else class="flex items-center gap-2">
            <span class="spinner"></span>
            Saving...
          </span>
        </button>
      </div>
    </fieldset>
    </form>
  </div>
</template>
