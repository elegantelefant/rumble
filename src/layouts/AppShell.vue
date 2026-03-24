<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
import { RouterView, useRouter, useRoute } from "vue-router";
import SidebarNav from "../modules/navigation/SidebarNav.vue";
import TopBar from "../modules/navigation/TopBar.vue";
import CommandPalette from "../components/CommandPalette.vue";

const sidebarOpen = ref(false);
const paletteOpen = ref(false);

const router = useRouter();
const route = useRoute();

const paletteCommands = [
  {
    id: "review",
    label: "Go to Document Review",
    shortcut: "R",
    action: () => router.push("/review"),
  },
  {
    id: "upload",
    label: "Upload documents",
    action: () => router.push("/review"),
  },
  {
    id: "draft",
    label: "Start a new draft",
    action: () => router.push("/draft"),
  },
];

function handleNavigate(path: string) {
  sidebarOpen.value = false;
  router.push(path);
}

function toggleSidebar() {
  sidebarOpen.value = !sidebarOpen.value;
}

function openPalette() {
  paletteOpen.value = true;
}

function handleKeydown(event: KeyboardEvent) {
  const key = event.key.toLowerCase();
  if ((event.metaKey || event.ctrlKey) && key === "`") {
    event.preventDefault();
    toggleSidebar();
  }
  if ((event.metaKey || event.ctrlKey) && key === "k") {
    event.preventDefault();
    paletteOpen.value = !paletteOpen.value;
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <div class="flex min-h-screen bg-[var(--primary-100)] text-[var(--gray-950)]">
    <SidebarNav
      :open="sidebarOpen"
      :active-path="route.path"
      @navigate="handleNavigate"
      @close="sidebarOpen = false"
    />

    <div class="flex flex-1 flex-col">
      <TopBar @toggle-sidebar="toggleSidebar" @toggle-palette="openPalette" />

      <main class="flex-1 overflow-auto bg-[var(--primary-100)] p-6">
        <RouterView v-slot="{ Component }">
          <KeepAlive include="DocumentReviewView,ResearchView,TranslationView">
            <component :is="Component" />
          </KeepAlive>
        </RouterView>
      </main>
    </div>

    <div
      class="fixed inset-0 z-30 bg-black/40 transition-opacity md:hidden"
      v-if="sidebarOpen"
      @click="sidebarOpen = false"
      aria-hidden="true"
    />

    <CommandPalette :open="paletteOpen" :commands="paletteCommands" @close="paletteOpen = false" />
  </div>
</template>

