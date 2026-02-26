<script setup lang="ts">
import ToastProvider from "./components/ToastProvider.vue";
</script>

<template>
  <ToastProvider>
    <RouterView />
  </ToastProvider>
</template>