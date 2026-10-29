// ABOUTME: Guards against privacy/security copy drifting out of backendMode.ts, including code comments.
// ABOUTME: Scans src/, README.md and the bundle config for claim phrases outside the map or explicit allowlist.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = join(__dirname, "..");
const SRC_ROOT = join(ROOT, "src");
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

// Files outside src/ that the strongest claims actually live in -- the
// README a user reads first, and the bundle config whose CSP enforces (or
// fails to enforce) a claim in practice.
const EXTRA_SCANNED_FILES = [join(ROOT, "README.md"), join(ROOT, "src-tauri", "tauri.conf.json")];

// Explicit per-file, per-phrase exemptions. Each entry is a claim this PR
// deliberately kept and checked true (#87's "Checked that the claims are
// true"), not a phrase the scanner failed to notice. Anything else, in this
// file or any other, still fails loudly.
const ALLOWLIST: Record<string, string[]> = {
  [join(ROOT, "README.md")]: [
    // README.md:3 -- "A private, on-device legal assistant ... Your data
    // never leaves your machine" -- the one claim #87 keeps, evidenced by
    // NETWORK.md linked right beside it.
    "on-device",
    "never leave",
  ],
};

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
  it("finds every claim phrase only in backendMode.ts or an explicit allowlist entry", () => {
    const violations: string[] = [];
    const files = [...listSourceFiles(SRC_ROOT), ...EXTRA_SCANNED_FILES];

    for (const file of files) {
      if (file === EXEMPT_FILE) continue;
      const content = readFileSync(file, "utf-8").toLowerCase();
      const allowed = ALLOWLIST[file] ?? [];
      for (const phrase of CLAIM_PHRASES) {
        if (content.includes(phrase) && !allowed.includes(phrase)) {
          violations.push(`${relative(ROOT, file)}: "${phrase}"`);
        }
      }
    }

    expect(violations).toEqual([]);
  });
});
