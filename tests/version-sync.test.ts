// ABOUTME: Verifies the desktop, Rust, and sidecar manifests share one app version.
// ABOUTME: Guards the Tauri package reference and records the Rust package author.

import { readFileSync } from "node:fs"
import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")

const read = (relativePath: string) =>
  readFileSync(resolve(repoRoot, relativePath), "utf8")

const cargoVersion = (cargoToml: string) =>
  cargoToml.match(/^version = "([^"]+)"/m)?.[1]

const cargoAuthors = (cargoToml: string) =>
  cargoToml.match(/^authors = \["([^"]+)"\]/m)?.[1]

const sidecarVersion = (pyprojectToml: string) =>
  pyprojectToml.match(/^version = "([^"]+)"/m)?.[1]

describe("release manifest metadata", () => {
  it("keeps every app manifest on the package version", () => {
    const packageJson = JSON.parse(read("package.json")) as { version: string }
    const tauriConfig = JSON.parse(read("src-tauri/tauri.conf.json")) as {
      version: string
    }
    const cargoToml = read("src-tauri/Cargo.toml")
    const pyprojectToml = read("src-tauri/sidecar/pyproject.toml")

    expect(tauriConfig.version).toBe("../package.json")
    expect(cargoVersion(cargoToml)).toBe(packageJson.version)
    expect(sidecarVersion(pyprojectToml)).toBe(packageJson.version)
  })

  it("identifies the Cargo package author", () => {
    const cargoToml = read("src-tauri/Cargo.toml")

    expect(cargoAuthors(cargoToml)).toBe("Elegant Elephant Pte Ltd")
  })
})
