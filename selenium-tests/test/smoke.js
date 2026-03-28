// ABOUTME: Selenium smoke tests for Rumble via tauri-driver.
// ABOUTME: Verifies the app launches and basic shell elements render.

import { expect } from "chai";
import { getDriver } from "./helpers.js";

describe("Smoke — app launches", function () {
  it("window renders content", async function () {
    const driver = getDriver();
    const heading = await driver.findElement({ css: "h1" });
    const text = await heading.getText();
    expect(text).to.be.a("string").with.length.greaterThan(0);
  });

  it("page title is set", async function () {
    const driver = getDriver();
    const title = await driver.getTitle();
    expect(title).to.include("Elefant");
  });
});
