// ABOUTME: Guards the webview CSP against allowing HTTPS connections in any backend mode.
// ABOUTME: Chat streaming uses host IPC, so connect-src needs no HTTPS origins.

import { readFileSync } from "node:fs";
import { join } from "node:path";

describe("webview connect-src", () => {
  it("contains no https: origin", () => {
    const config = JSON.parse(
      readFileSync(join(__dirname, "..", "src-tauri", "tauri.conf.json"), "utf-8"),
    );
    const directives = config.app.security.csp
      .split(";")
      .map((directive: string) => directive.trim().split(/\s+/));
    const connectSrc = directives.find(([name]: string[]) => name === "connect-src");

    expect(connectSrc).toBeDefined();
    expect(connectSrc.slice(1).filter((source: string) => /^https:/i.test(source))).toEqual([]);
  });
});
