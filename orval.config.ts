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
      // Wipe the output dirs before generating so schemas dropped from the spec
      // don't linger as orphaned files (and dead index.ts re-exports). Cleans
      // only inside target/schemas dirs, leaving sibling client.ts untouched.
      clean: true,
      override: {
        mutator: {
          path: "./src/api/client.ts",
          name: "apiClient",
        },
      },
    },
  },
});
