// ABOUTME: Guards against offering the inert Theme control in Settings.
// ABOUTME: Keeps both working sidebar selects in the appearance form.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync("src/views/SettingsView.vue", "utf8");

describe("Settings appearance controls", () => {
  it("has no Theme select", () => {
    expect(source).not.toMatch(/<label\b[^>]*>\s*Theme\s*<select\b/);
    expect(source).not.toMatch(/<select\b[^>]*v-model="appearanceSettings\.theme"/);
  });

  it("has no theme key in appearanceSettings", () => {
    const appearance = source.match(/const appearanceSettings = reactive\(\{([\s\S]*?)\}\);/);
    expect(appearance).not.toBeNull();
    expect(appearance![1]).not.toMatch(/\btheme\s*:/);
  });

  it("keeps both sidebar selects", () => {
    expect(source).toMatch(/<label\b[^>]*>\s*Navigation sidebar\s*<select\b[^>]*v-model="appearanceSettings\.navigationSidebar"/);
    expect(source).toMatch(/<label\b[^>]*>\s*Chat\/history sidebar\s*<select\b[^>]*v-model="appearanceSettings\.contextSidebar"/);
  });
});
