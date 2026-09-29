// ABOUTME: Shared model inventory composable for all views that need model selection.
// ABOUTME: Loads real models from the sidecar's /models endpoint, with a static fallback.
import { computed, ref, watch } from "vue";
import { listModels } from "../api/sidecar";
import { backendMode } from "./backendMode";

export type ModelOption = {
  id: string;
  label: string;
  provider: string;
  available: boolean;
};

// Used until /models responds, and as a fallback if the sidecar is unreachable
// in ollama mode only — offering a local model in byok or premium would be false.
const FALLBACK_MODELS: ModelOption[] = [
  { id: "elefant-local", label: "Ollama · Elefant Legal Blend", provider: "Local", available: true },
];

const models = ref<ModelOption[]>([...FALLBACK_MODELS]);
const loaded = ref(false);
const loading = ref(false);
// Bumped by invalidateModels, so a load that started before it can't land.
let listGeneration = 0;

/** Forgets the list, e.g. after a mode switch: the other mode's models must not be offered. */
export function invalidateModels() {
  listGeneration++;
  models.value = [];
  loaded.value = false;
  loading.value = false;
}

// A switch between two known modes invalidates; the first read (null → mode) doesn't.
watch(
  backendMode,
  (now, before) => {
    if (before !== null && now !== null && now !== before) invalidateModels();
  },
  { flush: "sync" },
);

export function useModels() {
  const availableModels = computed(() => models.value.filter((m) => m.available));

  async function loadModels(force = false) {
    if (loading.value || (loaded.value && !force)) return;
    const generation = listGeneration;
    loading.value = true;
    try {
      const response = await listModels();
      if (generation !== listGeneration) return;
      if (response.models.length) {
        models.value = response.models.map((m) => ({
          id: m.id,
          label: m.name,
          provider: m.provider,
          available: true,
        }));
      } else {
        models.value = [];
      }
      loaded.value = true;
    } catch (error) {
      console.error("Failed to load models from sidecar:", error);
      if (generation !== listGeneration) return;
      // Left unloaded either way, so the next load retries.
      models.value = backendMode.value === "ollama" ? [...FALLBACK_MODELS] : [];
    } finally {
      if (generation === listGeneration) loading.value = false;
    }
  }

  return { models, availableModels, loadModels, loading, loaded };
}
