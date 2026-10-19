// ABOUTME: Shared e2e helpers. The app's router uses createWebHashHistory
// ABOUTME: (router.ts), so navigation must target the hash, not the pathname.
import type { Page } from "@playwright/test";

/**
 * Navigate to an in-app route. The router resolves routes from
 * `location.hash` only (createWebHashHistory) -- a bare `page.goto(path)`
 * leaves the hash empty, which the router's catch-all/empty-child redirect
 * silently resolves to `/review` regardless of what path was requested
 * (rumble#61). `path` is the app route, e.g. "/draft", "/review", "/login".
 */
export function gotoRoute(page: Page, path: string) {
  return page.goto(`/#${path}`);
}
