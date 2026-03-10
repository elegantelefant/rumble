// ABOUTME: Shared test utilities for Rumble frontend tests.
// ABOUTME: Provides withSetup helper for composable testing with lifecycle hooks.

import { createApp, type App } from "vue"

/**
 * Wrap a composable that uses lifecycle hooks (onMounted, provide/inject)
 * in a host component so the hooks actually fire.
 */
export function withSetup<T>(composable: () => T): [T, App] {
  let result!: T
  const app = createApp({
    setup() {
      result = composable()
      return () => {}
    },
  })
  app.mount(document.createElement("div"))
  return [result, app]
}
