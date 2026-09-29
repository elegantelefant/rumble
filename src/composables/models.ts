// ABOUTME: Shared model inventory composable for all views that need model selection.
// ABOUTME: Loads real models from the sidecar's /models endpoint, with a static fallback.
import { computed, ref } from "vue";
import { listModels } from "../api/sidecar";

export type ModelOption = {
  id: string;
  label: string;
  provider: string;
  available: boolean;
};

// Used until /models responds, and as a fallback if the sidecar is unreachable.
const FALLBACK_MODELS: ModelOption[] = [
  { id: "elefant-local", label: "Ollama · Elefant Legal Blend", provider: "Local", available: true },
];

const models = ref<ModelOption[]>([...FALLBACK_MODELS]);
const loaded = ref(false);
const loading = ref(false);

/** Forgets the list, e.g. after a mode switch: the other mode's models must not be offered. */
export function invalidateModels() {
  models.value = [];
  loaded.value = false;
}

export function useModels() {
  const availableModels = computed(() => models.value.filter((m) => m.available));

  async function loadModels(force = false) {
    if (loading.value || (loaded.value && !force)) return;
    loading.value = true;
    try {
      const response = await listModels();
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
      models.value = [...FALLBACK_MODELS];
    } finally {
      loading.value = false;
    }
  }

  return { models, availableModels, loadModels, loading, loaded };
}
