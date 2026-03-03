// ABOUTME: Tests for LoginView.vue component.
// ABOUTME: Covers form rendering, passphrase validation, loading state, navigation.

import { mount } from "@vue/test-utils"
import { vi } from "vitest"
import LoginView from "../src/views/LoginView.vue"

const mockPush = vi.fn()

vi.mock("vue-router", () => ({
  useRouter: () => ({ push: mockPush }),
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

function mountLogin() {
  return mount(LoginView)
}

describe("LoginView", () => {
  it("renders the login form", () => {
    const wrapper = mountLogin()
    expect(wrapper.find('input[type="password"]').exists()).toBe(true)
    expect(wrapper.find('button[type="submit"]').exists()).toBe(true)
    expect(wrapper.text()).toContain("Enter Ivory")
  })

  it("renders the remember checkbox", () => {
    const wrapper = mountLogin()
    expect(wrapper.find('input[type="checkbox"]').exists()).toBe(true)
    expect(wrapper.text()).toContain("Remember for 30 days")
  })

  it("renders the generate phrase button", () => {
    const wrapper = mountLogin()
    expect(wrapper.text()).toContain("Generate phrase")
  })

  it("shows error when submitting empty passphrase", async () => {
    const wrapper = mountLogin()
    await wrapper.find("form").trigger("submit")
    expect(wrapper.text()).toContain("Passphrase required")
  })

  it("does not show error initially", () => {
    const wrapper = mountLogin()
    expect(wrapper.text()).not.toContain("Passphrase required")
    expect(wrapper.text()).not.toContain("Login failed")
  })

  it("shows loading state during authentication", async () => {
    const wrapper = mountLogin()
    await wrapper.find('input[type="password"]').setValue("my-secret")
    await wrapper.find("form").trigger("submit")
    expect(wrapper.text()).toContain("Authenticating...")
  })

  it("navigates to /review on successful login", async () => {
    const wrapper = mountLogin()
    await wrapper.find('input[type="password"]').setValue("my-secret")
    await wrapper.find("form").trigger("submit")

    // Advance past the 500ms mock delay
    await vi.advanceTimersByTimeAsync(600)

    expect(mockPush).toHaveBeenCalledWith("/review")
  })

  it("disables submit button while loading", async () => {
    const wrapper = mountLogin()
    await wrapper.find('input[type="password"]').setValue("my-secret")
    await wrapper.find("form").trigger("submit")

    const button = wrapper.find('button[type="submit"]')
    expect(button.attributes("disabled")).toBeDefined()
  })

  it("clears error on subsequent valid submit", async () => {
    const wrapper = mountLogin()

    // First: empty submit → error
    await wrapper.find("form").trigger("submit")
    expect(wrapper.text()).toContain("Passphrase required")

    // Second: fill in and submit → error clears
    await wrapper.find('input[type="password"]').setValue("my-secret")
    await wrapper.find("form").trigger("submit")
    expect(wrapper.text()).not.toContain("Passphrase required")
  })

  it("renders brand logo", () => {
    const wrapper = mountLogin()
    expect(wrapper.text()).toContain("Secure access")
  })
})
