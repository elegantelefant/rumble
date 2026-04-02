// ABOUTME: Selenium smoke tests for Rumble via headless Chrome.
// ABOUTME: Verifies the app launches and basic shell elements render.

import { expect } from "chai";
import { getDriver, screenshot } from "./helpers.js";

describe("Smoke — app launches", function () {
  it("window renders content", async function () {
    const driver = getDriver();
    await screenshot("smoke_initial_load");
    const heading = await driver.findElement({ css: "h1" });
    const text = await heading.getText();
    expect(text).to.be.a("string").with.length.greaterThan(0);
    await screenshot("smoke_heading_verified");
  });

  it("page title is set", async function () {
    const driver = getDriver();
    const title = await driver.getTitle();
    await screenshot("smoke_title_check");
    expect(title).to.include("Elefant");
  });
});
