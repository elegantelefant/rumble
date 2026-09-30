// ABOUTME: Guards against privacy/security copy drifting back out of the shared mode map.
// ABOUTME: Scans src/ for claim phrases that must come from backendMode.ts, not be written inline.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const SRC_ROOT = join(__dirname, "..", "src");
const EXEMPT_FILE = join(SRC_ROOT, "composables", "backendMode.ts");
const SCANNED_EXTENSIONS = [".vue", ".ts"];

// Case-insensitive; matched as plain substrings, not word-bounded, so a
// hyphen or spacing variant still gets caught.
const CLAIM_PHRASES = [
  "never leave",
  "encrypt",
  "on-device",
  "stays local",
  "sqlcipher",
  "zero-knowledge",
  "end-to-end",
  "this device",
  "this machine",
];

function listSourceFiles(dir: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      files.push(...listSourceFiles(full));
    } else if (SCANNED_EXTENSIONS.some((ext) => entry.endsWith(ext))) {
      files.push(full);
    }
  }
  return files;
}

describe("no hardcoded privacy claims outside the mode map", () => {
  it("finds every claim phrase only in backendMode.ts", () => {
    const violations: string[] = [];

    for (const file of listSourceFiles(SRC_ROOT)) {
      if (file === EXEMPT_FILE) continue;
      const content = readFileSync(file, "utf-8").toLowerCase();
      for (const phrase of CLAIM_PHRASES) {
        if (content.includes(phrase)) {
          violations.push(`${relative(SRC_ROOT, file)}: "${phrase}"`);
        }
      }
    }

    expect(violations).toEqual([]);
  });
});
