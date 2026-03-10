<script setup lang="ts">
import { computed, ref } from "vue";
import { FileText, Edit3, Microscope, BookOpen, Globe, Settings, ArrowUp, ArrowDown } from "lucide-vue-next";

const props = defineProps<{
  open: boolean;
  activePath: string;
}>();

const emit = defineEmits<{
  (e: "navigate", path: string): void;
  (e: "close"): void;
}>();

type ToolDefinition = {
  label: string;
  icon: typeof FileText;
  path: string;
  disabled?: boolean;
  sublabel?: string;
  premium?: boolean;
};

const reorderableToolDefinitions: ToolDefinition[] = [
  { label: "Document Review", icon: FileText, path: "/review" },
  { label: "Research", icon: BookOpen, path: "/research" },
  { label: "Document Draft", icon: Edit3, path: "/draft" },
  {
    label: "Evidence Review",
    icon: Microscope,
    path: "/evidence",
    disabled: true,
    sublabel: "Coming Soon",
  },
  { label: "Translation", icon: Globe, path: "/translation" },
];

const fixedToolDefinitions: ToolDefinition[] = [
  { label: "Settings", icon: Settings, path: "/settings" },
];

const userInfo = {
  user: "CoastalTower238",
  team: "SilverEcho951",
  version: "Rumble v0.1.0a",
};

const reorderableTools = ref<ToolDefinition[]>([...reorderableToolDefinitions]);

const hasMultipleTools = computed(() => reorderableTools.value.length > 1);
const logoSrc = new URL("../../assets/elefant-tray.png", import.meta.url).href;

function moveTool(index: number, direction: -1 | 1) {
  const nextIndex = index + direction;
  if (nextIndex < 0 || nextIndex >= reorderableTools.value.length) return;
  const updated = [...reorderableTools.value];
  const [moved] = updated.splice(index, 1);
  updated.splice(nextIndex, 0, moved);
  reorderableTools.value = updated;
}

const isActive = (path: string) => props.activePath === path;

function handleItemClick(path: string, disabled?: boolean) {
  if (disabled) return;
  emit("navigate", path);
}
</script>

<template>
  <aside
    class="sidebar fixed inset-y-0 left-0 z-40 flex w-72 flex-col border-r border-[var(--primary-800)] bg-[var(--primary-900)] text-white transition md:static md:translate-x-0"
    :class="[props.open ? 'translate-x-0' : '-translate-x-full md:translate-x-0']"
  >
    <div class="flex items-center justify-between px-4 py-4 md:hidden">
      <span class="text-lg font-semibold">Elefant - Rumble</span>
      <button class="btn-secondary text-white" @click="emit('close')">Close</button>
    </div>

    <div class="flex flex-1 flex-col px-5 pt-6 pb-4">
      <div class="mb-6 space-y-3">
        <div class="flex items-center gap-3">
          <img :src="logoSrc" alt="Elefant logo" class="h-10 w-10 rounded-lg border border-[color:color-mix(in_srgb,var(--primary-400)_60%,transparent)] bg-[color:color-mix(in_srgb,var(--primary-200)_70%,white)] object-contain p-1.5" />
          <div class="leading-tight">
            <div class="text-xl font-bold tracking-tight text-white">Elefant - Rumble</div>
            <div class="text-xs uppercase tracking-[0.45em] text-[var(--primary-500)]">Workspace</div>
          </div>
        </div>
        <p class="text-xs font-medium uppercase text-[var(--primary-400)]">
          Confidential AI Tools Running Entirely On Your Device.
        </p>
      </div>

      <nav class="space-y-2" aria-label="Primary tools">
        <h2 class="sr-only">Workspace tools</h2>
        <div
          v-for="(tool, index) in reorderableTools"
          :key="tool.path"
          class="group relative flex items-center rounded-lg border border-transparent bg-[color:color-mix(in_srgb,var(--primary-900)_60%,var(--primary-800)_40%)]/60 transition hover:border-[var(--primary-700)]"
        >
          <button
            type="button"
            class="flex flex-1 items-center justify-between rounded-l-lg px-3 py-2 text-left"
            :aria-disabled="tool.disabled ? 'true' : 'false'"
            :class="[
              tool.disabled
                ? 'cursor-not-allowed opacity-50'
                : isActive(tool.path)
                  ? 'bg-[color:color-mix(in_srgb,var(--primary-800)_70%,transparent)]'
                  : '',
            ]"
            @click="handleItemClick(tool.path, tool.disabled)"
          >
            <div class="flex items-center gap-3">
              <component
                :is="tool.icon"
                class="h-4 w-4"
                :class="tool.disabled ? 'text-[var(--primary-600)]' : isActive(tool.path) ? 'text-[var(--accent-400)]' : 'text-[var(--primary-400)] group-hover:text-[var(--accent-400)]'"
              />
              <div>
                <div class="text-sm font-semibold text-white">{{ tool.label }}</div>
                <div v-if="tool.sublabel" class="text-xs text-[var(--primary-500)]">
                  {{ tool.sublabel }}
                </div>
              </div>
            </div>
            <span v-if="tool.premium" class="text-[var(--warning)]">★</span>
          </button>
          <div
            v-if="hasMultipleTools"
            class="flex h-full w-10 flex-col items-center justify-center gap-1 border-l border-[color:color-mix(in_srgb,var(--primary-700)_60%,transparent)] bg-[color:color-mix(in_srgb,var(--primary-900)_90%,black_10%)]"
          >
            <button
              type="button"
              class="rounded p-1 text-[var(--primary-400)] transition hover:text-[var(--accent-300)] disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="index === 0"
              :aria-label="`Move ${tool.label} up`"
              @click.stop="moveTool(index, -1)"
            >
              <ArrowUp class="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              class="rounded p-1 text-[var(--primary-400)] transition hover:text-[var(--accent-300)] disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="index === reorderableTools.length - 1"
              :aria-label="`Move ${tool.label} down`"
              @click.stop="moveTool(index, 1)"
            >
              <ArrowDown class="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </nav>

      <div class="mt-6 space-y-2 border-t border-[var(--primary-700)] pt-4" aria-label="Administrative tools">
        <h2 class="sr-only">Administrative tools</h2>
        <button
          type="button"
          v-for="tool in fixedToolDefinitions"
          :key="tool.path"
          class="group flex w-full items-center justify-between rounded-lg px-3 py-2 text-left transition"
          :aria-disabled="tool.disabled ? 'true' : 'false'"
          :class="[
            tool.disabled
              ? 'cursor-not-allowed opacity-50'
              : isActive(tool.path)
                ? 'bg-[color:color-mix(in_srgb,var(--primary-800)_70%,transparent)] border-l-4 border-[var(--accent-400)]'
                : 'hover:bg-[color:color-mix(in_srgb,var(--primary-800)_40%,transparent)]',
          ]"
          @click="handleItemClick(tool.path, tool.disabled)"
        >
          <div class="flex items-center gap-3">
            <component
              :is="tool.icon"
              class="h-4 w-4"
              :class="tool.disabled ? 'text-[var(--primary-600)]' : isActive(tool.path) ? 'text-[var(--accent-400)]' : 'text-[var(--primary-400)] group-hover:text-[var(--accent-400)]'"
            />
            <div>
              <div class="text-sm font-semibold text-white">{{ tool.label }}</div>
              <div v-if="tool.sublabel" class="text-xs text-[var(--primary-500)]">
                {{ tool.sublabel }}
              </div>
            </div>
          </div>
        </button>
      </div>
    </div>

    <div class="space-y-3 px-5 pb-6 pt-4 text-sm text-[var(--primary-300)]">
      <div class="rounded-lg border border-[color:color-mix(in_srgb,var(--primary-700)_70%,transparent)] bg-[color:color-mix(in_srgb,var(--primary-900)_85%,black_15%)] px-3 py-4">
        <div class="text-xs uppercase text-[var(--primary-500)]">Signed in</div>
        <div class="mt-1 text-sm font-semibold text-white">{{ userInfo.user }}</div>
        <div class="text-xs text-[var(--primary-400)]">Team · {{ userInfo.team }}</div>
      </div>
      <div class="text-xs text-[var(--primary-500)]">
        {{ userInfo.version }}
      </div>
    </div>
  </aside>
</template>
