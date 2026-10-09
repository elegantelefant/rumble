// ABOUTME: The one shared view of the host's backend mode, and the privacy copy each mode earns.
// ABOUTME: TopBar, Settings and DocumentReviewView read it here so a mode switch updates all of them at once.
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export type BackendMode = "ollama" | "byok" | "premium";
export type ConfidentialityState = "local" | "byok" | "hybrid" | "unknown";

// Each backend mode gets its own state. Collapsing byok into "local" would
// claim confidentiality for requests that go to a hosted provider.
//
// `notice` is the short, standalone sentence views use for inline privacy
// captions (file upload areas, form footers, the sidebar tagline) — kept
// separate from `message` (the pill tooltip / Settings dropdown wording,
// which names what's stored where) so each reads naturally in its own spot.
export const CONFIDENTIALITY: Record<ConfidentialityState, { label: string; message: string; notice: string }> = {
  local: {
    label: "Local & Confidential",
    message: "Chats and drafting stay on this device.",
    notice: "Your data stays on this device.",
  },
  byok: {
    label: "Direct to Provider",
    message:
      "Requests go to your chosen provider using your API key. Chats are stored locally.",
    notice: "Your data goes to your chosen provider using your API key.",
  },
  hybrid: {
    label: "Hybrid",
    message: "Chats retained locally; remote agents may assist on request.",
    notice: "Chats stay local; remote agents may assist on request.",
  },
  unknown: {
    label: "Mode unavailable",
    message: "Could not determine where requests are sent.",
    notice: "Could not determine where your data goes.",
  },
};

// Where local data lives (settings, chats, job history) doesn't depend on
// the backend mode — every mode still stores its records here — so these
// belong beside the map rather than duplicated across CONFIDENTIALITY's
// per-mode entries. Whole sentences, like CONFIDENTIALITY's: a view renders
// one of these as-is rather than composing a claim around a bare noun.
export const STORAGE_NOTICES = {
  savedToast: "Settings saved on this device.",
  providersHint:
    "Remote providers process prompts on their servers. Chats, review and drafting jobs, and these settings are kept on this device.",
  workspaceData:
    "Chats, messages and document review, draft and research jobs, including the document text, are kept in a database in this device's local app data folder, not in the workspace path below. Jobs are deleted the next time Rumble starts after they turn 30 days old; chats stay until you delete all local data. Backups of this device copy the database too.",
};

export const MODE_TO_STATE: Record<BackendMode, ConfidentialityState> = {
  ollama: "local",
  byok: "byok",
  premium: "hybrid",
};

// null until read, and after a failed read — never a guess.
export const backendMode = ref<BackendMode | null>(null);

// Only the newest read may land: a read that started before a switch
// completed must not overwrite the switched mode.
let latestRead = 0;

export function confidentialityOf(mode: BackendMode | null): ConfidentialityState {
  return (mode && MODE_TO_STATE[mode]) ?? "unknown";
}

export async function loadBackendMode(): Promise<void> {
  const read = ++latestRead;
  try {
    const mode = await invoke<BackendMode>("get_backend_mode");
    if (read === latestRead) backendMode.value = mode;
  } catch (error) {
    console.error("Failed to read backend mode:", error);
    if (read === latestRead) backendMode.value = null;
  }
}

/**
 * Switches the host. Resolves true when the mode changed and the sidecar is
 * restarting, false when the host was already in that mode. The shared mode
 * changes only once the host confirms; on failure it re-reads the host's mode
 * and rethrows its string.
 */
export async function setBackendMode(mode: BackendMode): Promise<boolean> {
  try {
    const changed = await invoke<boolean>("set_backend_mode", { mode });
    latestRead++;
    backendMode.value = mode;
    return changed;
  } catch (error) {
    await loadBackendMode();
    throw typeof error === "string" ? error : String(error);
  }
}
