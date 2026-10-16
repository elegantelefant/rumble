// ABOUTME: Guards against e2e specs bypassing gotoRoute with a raw page.goto() call.
// ABOUTME: A bare page.goto(path) leaves the hash empty and silently lands on /review (rumble#61).
import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const E2E_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "e2e");

function specFiles(): string[] {
  return readdirSync(E2E_DIR).filter((name) => name.endsWith(".spec.ts"));
}

describe("e2e specs navigate via gotoRoute, not page.goto", () => {
  it("contains no direct page.goto() call in any spec", () => {
    const offenders = specFiles()
      .map((file) => {
        const content = readFileSync(join(E2E_DIR, file), "utf-8");
        const hits = content.match(/\bpage\.goto\(/g);
        return hits ? `${file} (${hits.length})` : null;
      })
      .filter((hit): hit is string => hit !== null);

    expect(offenders, `page.goto() bypasses the hash-aware router; use gotoRoute instead:\n${offenders.join("\n")}`).toEqual([]);
  });
});
