<script setup lang="ts">
import { provide, reactive } from "vue";

type Toast = {
  id: number;
  message: string;
  type: "success" | "error" | "info";
};

const toasts = reactive<Toast[]>([]);
let toastCounter = 0;

function addToast(message: string, type: Toast["type"] = "info", duration = 3000) {
  const id = ++toastCounter;
  toasts.push({ id, message, type });
  setTimeout(() => removeToast(id), duration);
}

function removeToast(id: number) {
  const index = toasts.findIndex((toast) => toast.id === id);
  if (index !== -1) toasts.splice(index, 1);
}

provide("toast", { addToast });
</script>

<template>
  <slot />
  <div class="fixed right-4 bottom-4 z-[1000] flex w-80 flex-col gap-3 pointer-events-none">
    <transition-group name="toast">
      <div
        v-for="toast in toasts"
        :key="toast.id"
        class="pointer-events-auto rounded-lg px-4 py-3 text-sm text-white shadow-lg"
        :class="{
          'bg-[var(--success)]': toast.type === 'success',
          'bg-[var(--error)]': toast.type === 'error',
          'bg-[var(--accent-600)]': toast.type === 'info',
        }"
      >
        {{ toast.message }}
      </div>
    </transition-group>
  </div>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 200ms ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
