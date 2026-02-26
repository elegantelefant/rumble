// ABOUTME: Orval configuration for generating vue-query composables from openapi.json.
// ABOUTME: Routes all generated API calls through the Tauri IPC bridge in src/api/client.ts.
import { defineConfig } from "orval";

export default defineConfig({
  api: {
    input: { target: "./openapi.json" },
    output: {
      target: "src/api/generated",
      schemas: "src/api/models",
      client: "vue-query",
      mode: "tags-split",
      override: {
        mutator: {
          path: "./src/api/client.ts",
          name: "apiClient",
        },
      },
    },
  },
});
