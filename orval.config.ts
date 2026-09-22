// ABOUTME: Orval configuration for generating contract types from sidecar-openapi.json.
// ABOUTME: Types only — API calls go through the hand-written client in src/api/sidecar.ts.
import { defineConfig } from "orval";

export default defineConfig({
  api: {
    input: "./sidecar-openapi.json",
    output: {
      // orval refuses to run without a client target; .orval/ is gitignored
      // and outside tsconfig's include, so the generated client is discarded.
      target: ".orval/discard.ts",
      schemas: "src/api/models",
    },
  },
});
