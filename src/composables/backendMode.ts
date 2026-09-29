// ABOUTME: The one shared view of the host's backend mode, and the privacy copy each mode earns.
// ABOUTME: TopBar, Settings and DocumentReviewView read it here so a mode switch updates all of them at once.
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export type BackendMode = "ollama" | "byok" | "premium";
export type ConfidentialityState = "local" | "byok" | "hybrid" | "unknown";

// Each backend mode gets its own state. Collapsing byok into "local" would
// claim confidentiality for requests that go to a hosted provider.
export const CONFIDENTIALITY: Record<ConfidentialityState, { label: string; message: string }> = {
  local: {
    label: "Local & Confidential",
    message: "Chats and drafting stay on this device.",
  },
  byok: {
    label: "Direct to Provider",
    message:
      "Requests go to your chosen provider using your API key. Chats are stored locally.",
  },
  hybrid: {
    label: "Hybrid",
    message: "Chats retained locally; remote agents may assist on request.",
  },
  unknown: {
    label: "Mode unavailable",
    message: "Could not determine where requests are sent.",
  },
};

export const MODE_TO_STATE: Record<BackendMode, ConfidentialityState> = {
  ollama: "local",
  byok: "byok",
  premium: "hybrid",
};

// null until read, and after a failed read — never a guess.
export const backendMode = ref<BackendMode | null>(null);

export function confidentialityOf(mode: BackendMode | null): ConfidentialityState {
  return (mode && MODE_TO_STATE[mode]) ?? "unknown";
}

export async function loadBackendMode(): Promise<void> {
  try {
    backendMode.value = await invoke<BackendMode>("get_backend_mode");
  } catch (error) {
    console.error("Failed to read backend mode:", error);
    backendMode.value = null;
  }
}

/** Switches the host (which respawns the sidecar); on failure re-reads the host's mode and rethrows its string. */
export async function setBackendMode(mode: BackendMode): Promise<void> {
  try {
    await invoke("set_backend_mode", { mode });
    backendMode.value = mode;
  } catch (error) {
    await loadBackendMode();
    throw typeof error === "string" ? error : String(error);
  }
}
