import { createRouter, createWebHashHistory } from "vue-router";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/login", component: () => import("./views/LoginView.vue") },
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

router.beforeEach((to) => {
  const publicPaths = ["/login"];
  const isPublic = publicPaths.includes(to.path);
  const isAuthenticated = true; // TODO: replace with BetterAuth session check
  if (!isPublic && !isAuthenticated) {
    return "/login";
  }
  if (isPublic && isAuthenticated && to.path === "/login") {
    return "/review";
  }
  return true;
});

export default router;
