// ABOUTME: Tests for src/utils.ts pure functions.
// ABOUTME: Covers formatSize, oldestFileDate, largestFile with edge cases.

import { formatSize, oldestFileDate, largestFile } from "../src/utils"
import type { FileInfo } from "../src/types"

// --- formatSize ---

describe("formatSize", () => {
  it("formats zero bytes", () => {
    expect(formatSize(0)).toBe("0.0 B")
  })

  it("formats bytes below 1 KB", () => {
    expect(formatSize(512)).toBe("512.0 B")
  })

  it("formats exact kilobyte boundary", () => {
    expect(formatSize(1024)).toBe("1.0 KB")
  })

  it("formats fractional kilobytes", () => {
    expect(formatSize(1536)).toBe("1.5 KB")
  })

  it("formats megabytes", () => {
    expect(formatSize(1024 * 1024)).toBe("1.0 MB")
  })

  it("formats gigabytes", () => {
    expect(formatSize(1024 ** 3)).toBe("1.0 GB")
  })

  it("formats terabytes", () => {
    expect(formatSize(1024 ** 4)).toBe("1.0 TB")
  })

  it("caps at TB for very large values", () => {
    expect(formatSize(1024 ** 5)).toBe("1024.0 TB")
  })
})

// --- oldestFileDate ---

const makeFile = (modified: string, size = 100): FileInfo => ({
  path: `/test/${modified}`,
  size,
  modified,
})

describe("oldestFileDate", () => {
  it("returns dash for empty array", () => {
    expect(oldestFileDate([])).toBe("—")
  })

  it("returns the date for a single file", () => {
    expect(oldestFileDate([makeFile("2024-06-15 10:00:00")])).toBe("2024-06-15")
  })

  it("returns the oldest date from multiple files", () => {
    const files = [
      makeFile("2024-06-15 10:00:00"),
      makeFile("2023-01-01 08:00:00"),
      makeFile("2025-12-31 23:59:59"),
    ]
    expect(oldestFileDate(files)).toBe("2023-01-01")
  })
})

// --- largestFile ---

describe("largestFile", () => {
  it("returns null for empty array", () => {
    expect(largestFile([])).toBeNull()
  })

  it("returns the only file for single-element array", () => {
    const file = makeFile("2024-01-01", 500)
    expect(largestFile([file])).toBe(file)
  })

  it("returns the file with the largest size", () => {
    const small = makeFile("2024-01-01", 100)
    const large = makeFile("2024-01-02", 9999)
    const medium = makeFile("2024-01-03", 500)
    expect(largestFile([small, large, medium])).toBe(large)
  })

  it("returns first file on tie", () => {
    const a = makeFile("a.txt", 100)
    const b = makeFile("b.txt", 100)
    expect(largestFile([a, b])).toBe(a)
  })
})
