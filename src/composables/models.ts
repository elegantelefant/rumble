// ABOUTME: Shared model inventory composable for all views that need model selection.
// ABOUTME: Single source of truth for available models and provider availability state.
import { computed, ref } from "vue";

export type ModelOption = {
  id: string;
  label: string;
  provider: string;
  available: boolean;
};

const models = ref<ModelOption[]>([
  { id: "elefant-local", label: "Ollama · Elefant Legal Blend", provider: "Local", available: true },
  { id: "gpt-4.1-mini", label: "GPT-4.1 mini", provider: "OpenAI", available: false },
  { id: "sonnet-3.5", label: "Claude 3.5 Sonnet", provider: "Anthropic", available: false },
]);

export function useModels() {
  const availableModels = computed(() => models.value.filter((m) => m.available));

  function setAvailability(providerId: string, available: boolean) {
    for (const m of models.value) {
      if (m.provider.toLowerCase() === providerId.toLowerCase()) {
        m.available = available;
      }
    }
  }

  return { models, availableModels, setAvailability };
}
