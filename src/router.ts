import { createRouter, createWebHashHistory } from "vue-router";
import { ref } from "vue";

/** Tracks whether Ollama setup has been verified this session. */
export const setupVerified = ref(false);

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/login", component: () => import("./views/LoginView.vue") },
    { path: "/setup", component: () => import("./views/SetupView.vue") },
    {
      path: "/",
      component: () => import("./layouts/AppShell.vue"),
      children: [
        { path: "review", component: () => import("./views/DocumentReviewView.vue") },
        { path: "draft", component: () => import("./views/DocumentDraftView.vue") },
        { path: "evidence", component: () => import("./views/EvidenceReviewView.vue") },
        { path: "research", component: () => import("./views/ResearchView.vue") },
        { path: "translation", component: () => import("./views/TranslationView.vue") },
        // { path: "evals", component: () => import("./views/EvalsView.vue") },
        // { path: "briefcases", component: () => import("./views/BriefcasesView.vue") },
        // { path: "plugins", component: () => import("./views/PluginsView.vue") },
        { path: "settings", component: () => import("./views/SettingsView.vue") },
        { path: "", redirect: "/review" },
      ],
    },
    { path: "/:pathMatch(.*)*", redirect: "/review" },
  ],
});

router.beforeEach(async (to) => {
  const publicPaths = ["/login", "/setup"];
  const isPublic = publicPaths.includes(to.path);
  const isAuthenticated = true; // TODO: replace with BetterAuth session check

  if (!isPublic && !isAuthenticated) {
    return "/login";
  }
  if (isPublic && isAuthenticated && to.path === "/login") {
    return "/review";
  }

  // On first navigation to an app page, check Ollama readiness
  const inTauri = "__TAURI_INTERNALS__" in window;
  if (!isPublic && !setupVerified.value && inTauri) {
    try {
      const { ready } = await import("./api/sidecar");
      const res = await ready();
      if (res.status === "ready") {
        setupVerified.value = true;
        return true;
      }
    } catch (err) {
      // invoke() rejects with a string for a real backend error (sidecar up,
      // Ollama down); anything else means the IPC bridge itself is broken,
      // which is not a readiness result and must not be swallowed as one.
      if (typeof err !== "string") {
        throw err;
      }
    }
    // Redirect to setup if check failed
    return "/setup";
  }

  return true;
});

export default router;
