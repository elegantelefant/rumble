<!-- ABOUTME: The one banner on a file whose work diverged: who edited which version, and the three ways forward. -->
<!-- ABOUTME: Combine in Word is a manual step for now, and the banner says so. -->
<script setup lang="ts">
import { computed } from "vue";
import type { FileView, VersionView } from "../api/sync";

const props = defineProps<{ file: FileView; busy: boolean }>();
const emit = defineEmits<{ (e: "combine"): void; (e: "use", version: VersionView): void }>();

const heads = computed(() => props.file.versions.filter((v) => v.head));
const who = (v: VersionView) => (v.mine ? "You" : v.author);
const editors = computed(() => {
  const names = [...new Set(heads.value.map(who))];
  return names.length > 1 ? `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}` : names[0];
});
const others = computed(() => [...new Set(heads.value.filter((v) => !v.mine).map((v) => `${v.author}'s`))].join(" and "));
const useLabel = (v: VersionView) => (v.mine ? "Use mine" : `Use ${v.author}'s`);
</script>

<template>
  <div class="rounded border border-[var(--warning)] bg-[#fef7e7] p-3 text-sm text-[var(--primary-800)]" role="status">
    <p class="font-semibold">{{ editors }} each edited v{{ file.forkedFrom }}.</p>
    <div class="mt-2 flex flex-wrap gap-2">
      <button class="btn-primary" type="button" :disabled="busy" @click="emit('combine')">Combine in Word</button>
      <button v-for="v in heads" :key="v.id" class="btn-secondary" type="button" :disabled="busy" @click="emit('use', v)">
        {{ useLabel(v) }}
      </button>
    </div>
    <p class="mt-2 text-xs text-[var(--primary-600)]">
      Combine in Word opens your version with {{ others || "the other version" }} beside it. Bring the changes across
      yourself (Word's Compare, on the Review tab, can help), then press Save: that becomes the next version. Using one
      side keeps the other in the history.
    </p>
  </div>
</template>
