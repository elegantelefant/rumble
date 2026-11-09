// ABOUTME: Builds a Tauri latest.json from the signatures of release updater artifacts.
// ABOUTME: Explicit platform mappings avoid guessing target architectures from filenames.
import { readFile, stat } from "node:fs/promises";
import { basename } from "node:path";

// node scripts/updater-manifest.mjs VERSION TAG darwin-aarch64=PATH.app.tar.gz.sig [...]
const [version, tag, ...entries] = process.argv.slice(2);
if (!version || !tag || entries.length === 0) {
  throw new Error("Usage: updater-manifest.mjs VERSION TAG PLATFORM=ARTIFACT.sig [...]");
}
const platforms = {};
for (const entry of entries) {
  const separator = entry.indexOf("=");
  const platform = entry.slice(0, separator);
  const signaturePath = entry.slice(separator + 1);
  if (separator < 1 || !/^(darwin|linux|windows)-(aarch64|x86_64|i686)(-\w+)?$/.test(platform)
      || !signaturePath.endsWith(".sig") || platforms[platform]) {
    throw new Error(`Invalid or duplicate platform mapping: ${entry}`);
  }
  const artifact = signaturePath.slice(0, -4);
  if (!(await stat(artifact)).isFile()) throw new Error(`Missing updater artifact: ${artifact}`);
  const signature = (await readFile(signaturePath, "utf8")).trim();
  if (!signature) throw new Error(`Empty signature: ${signaturePath}`);
  platforms[platform] = {
    signature,
    url: `https://github.com/elegantelefant/rumble/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(basename(artifact))}`,
  };
}
process.stdout.write(`${JSON.stringify({ version, platforms }, null, 2)}\n`);
