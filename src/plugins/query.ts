// ABOUTME: VueQuery plugin configuration for the app.
// ABOUTME: Provides QueryClient with sensible defaults for a desktop app.
import type { VueQueryPluginOptions } from "@tanstack/vue-query";
import { QueryClient, VueQueryPlugin } from "@tanstack/vue-query";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 30_000,
      refetchOnWindowFocus: false,
    },
  },
});

export const vueQueryPluginOptions: VueQueryPluginOptions = { queryClient };
export { VueQueryPlugin };
