// ABOUTME: Shared utility functions for generating IDs and formatting timestamps.
// ABOUTME: Eliminates duplication across views that all need the same helpers.

export function generateId(): string {
  return Math.random().toString(36).slice(2, 10);
}

export function formatTimestamp(date = new Date()): string {
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
