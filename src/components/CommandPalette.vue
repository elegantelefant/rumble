<script setup lang="ts">
import { shallowRef, computed, watch, onMounted, onBeforeUnmount } from "vue";

const props = defineProps<{
  open: boolean;
  commands: { id: string; label: string; shortcut?: string; action: () => void }[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

const query = shallowRef("");
const highlightedIndex = shallowRef(0);

const filteredCommands = computed(() => {
  const q = query.value.toLowerCase().trim();
  if (!q) return props.commands;
  return props.commands.filter((cmd) => cmd.label.toLowerCase().includes(q));
});

watch(query, () => {
  highlightedIndex.value = 0;
});

function resetState() {
  query.value = "";
  highlightedIndex.value = 0;
}

function closePalette() {
  resetState();
  emit("close");
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) return;
  if (event.key === "Escape") {
    event.preventDefault();
    closePalette();
  } else if (event.key === "ArrowDown") {
    event.preventDefault();
    highlightedIndex.value = (highlightedIndex.value + 1) % filteredCommands.value.length;
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    highlightedIndex.value =
      (highlightedIndex.value - 1 + filteredCommands.value.length) % filteredCommands.value.length;
  } else if (event.key === "Enter") {
    event.preventDefault();
    const cmd = filteredCommands.value[highlightedIndex.value];
    if (cmd) {
      cmd.action();
      closePalette();
    }
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <transition name="fade">
    <div v-if="props.open" class="fixed inset-0 z-[999] flex items-start justify-center bg-black/30 backdrop-blur-sm">
      <div class="mt-24 w-full max-w-xl rounded-xl bg-white shadow-xl">
        <div class="border-b border-[var(--primary-200)] p-4">
          <label class="sr-only" for="palette-search">Search shortcuts</label>
          <input
            id="palette-search"
            v-model="query"
            class="input"
            type="text"
            placeholder="Search shortcuts or jump to a workspace area..."
            autofocus
          />
        </div>
        <ul class="max-h-72 overflow-y-auto p-2">
          <li
            v-for="(command, index) in filteredCommands"
            :key="command.id"
            class="flex items-center justify-between rounded-md px-3 py-2 text-sm transition"
            :class="index === highlightedIndex ? 'bg-[var(--primary-200)]' : ''"
            @mouseenter="highlightedIndex = index"
            @click="() => { command.action(); closePalette(); }"
          >
            <span>{{ command.label }}</span>
            <span v-if="command.shortcut" class="text-xs text-[var(--primary-500)]">{{ command.shortcut }}</span>
          </li>
          <li v-if="filteredCommands.length === 0" class="px-3 py-4 text-sm text-[var(--primary-500)]">
            No matches. Try a different query.
          </li>
        </ul>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 150ms ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>














