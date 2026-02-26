<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    variant?: "full" | "compact" | "mark";
  }>(),
  {
    variant: "full",
  },
);
</script>

<template>
  <div
    class="flex items-center gap-3"
    :class="props.variant === 'mark' ? 'justify-center' : 'justify-start'"
  >
    <div
      v-if="props.variant !== 'compact'"
      class="flex h-10 w-10 items-center justify-center rounded-lg border border-[color:color-mix(in_srgb,var(--primary-400)_60%,transparent)] bg-[color:color-mix(in_srgb,var(--primary-200)_70%,white)]"
      aria-label="Elefant logo"
    >
      <slot name="mark">
        <svg viewBox="0 0 32 32" aria-hidden="true" class="h-6 w-6 text-[var(--primary-500)]">
          <circle cx="16" cy="16" r="14" fill="currentColor" opacity="0.12" />
          <path
            d="M11 23V9h5.2c3.4 0 5.8 2.2 5.8 5.4 0 3.1-2.4 5.6-5.8 5.6H11Z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <path
            d="M17 14h3m-3 4h2.5"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
          />
        </svg>
      </slot>
    </div>
    <div v-if="props.variant !== 'mark'" class="leading-tight text-left">
      <div
        class="font-semibold tracking-tight text-[var(--primary-900)]"
        :class="props.variant === 'full' ? 'text-[1.65rem]' : 'text-lg'"
      >
        Elefant
      </div>
      <div
        class="uppercase tracking-[0.45em] text-[var(--primary-500)]"
        :class="props.variant === 'full' ? 'text-[0.68rem]' : 'text-[0.58rem]'"
      >
        Ivory
      </div>
    </div>
  </div>
</template>
