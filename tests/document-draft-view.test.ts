// ABOUTME: Tests for DocumentDraftView.vue form validation logic.
// ABOUTME: Covers field validation, form-level validation, error display, submission gating.

import { mount } from "@vue/test-utils"
import DocumentDraftView from "../src/views/DocumentDraftView.vue"
import { TOAST_KEY } from "../src/composables/toast"

beforeEach(() => {
  vi.clearAllMocks()
})

function mountDraft() {
  return mount(DocumentDraftView, {
    global: {
      provide: {
        [TOAST_KEY as symbol]: { addToast: vi.fn() },
      },
    },
  })
}

describe("DocumentDraftView", () => {
  it("renders the page title", () => {
    const wrapper = mountDraft()
    expect(wrapper.text()).toContain("Document Draft")
  })

  it("renders template library", () => {
    const wrapper = mountDraft()
    expect(wrapper.text()).toContain("Employment Agreement")
    expect(wrapper.text()).toContain("Non-Disclosure Agreement")
    expect(wrapper.text()).toContain("Service Contract")
  })

  it("shows validation error when required field is empty on blur", async () => {
    const wrapper = mountDraft()
    const nameInput = wrapper.find('input[placeholder="Full name"]')
    await nameInput.setValue("")
    await nameInput.trigger("blur")
    expect(wrapper.text()).toContain("Employee name is required")
  })

  it("clears validation error when field is filled", async () => {
    const wrapper = mountDraft()
    const nameInput = wrapper.find('input[placeholder="Full name"]')

    // Trigger error
    await nameInput.setValue("")
    await nameInput.trigger("blur")
    expect(wrapper.text()).toContain("Employee name is required")

    // Fix it
    await nameInput.setValue("Jane Doe")
    await nameInput.trigger("blur")
    expect(wrapper.text()).not.toContain("Employee name is required")
  })

  it("shows errors for all required fields on Generate Draft click", async () => {
    const wrapper = mountDraft()
    await wrapper.find("button.btn-primary").trigger("click")
    expect(wrapper.text()).toContain("Employee name is required")
    expect(wrapper.text()).toContain("Start date is required")
    expect(wrapper.text()).toContain("Salary is required")
    expect(wrapper.text()).toContain("Position is required")
  })

  it("does not show errors when all required fields are filled", async () => {
    const wrapper = mountDraft()

    await wrapper.find('input[placeholder="Full name"]').setValue("Jane Doe")
    await wrapper.find('input[type="date"]').setValue("2025-01-01")
    await wrapper.find('input[placeholder="$100,000"]').setValue("120000")
    await wrapper.find('input[placeholder="Role"]').setValue("Engineer")

    await wrapper.find("button.btn-primary").trigger("click")

    expect(wrapper.text()).not.toContain("is required")
  })

  it("selects a different template on click", async () => {
    const wrapper = mountDraft()
    const templateButtons = wrapper.findAll("aside button")
    // Click "Non-Disclosure Agreement" (index 1)
    await templateButtons[1].trigger("click")
    // The selected template button should have the active class
    expect(templateButtons[1].classes()).toContain("bg-[var(--primary-200)]")
  })

  it("renders export buttons", () => {
    const wrapper = mountDraft()
    expect(wrapper.text()).toContain("Export to Word")
    expect(wrapper.text()).toContain("Export to PDF")
  })
})
