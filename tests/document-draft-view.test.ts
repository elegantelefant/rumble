// ABOUTME: Tests for DocumentDraftView.vue form validation logic.
// ABOUTME: Covers field validation, form-level validation, error display, submission gating, and export.

import { mount, flushPromises } from "@vue/test-utils"
import DocumentDraftView from "../src/views/DocumentDraftView.vue"
import { TOAST_KEY } from "../src/composables/toast"
import { invoke } from "@tauri-apps/api/core"
import { createDraftJob, waitForJob } from "../src/api/sidecar"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

vi.mock("../src/api/sidecar", () => ({
  createDraftJob: vi.fn(),
  waitForJob: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(invoke).mockResolvedValue(true)
})

async function fillAndGenerate(wrapper: ReturnType<typeof mountDraft>) {
  await wrapper.find('input[placeholder="Full name"]').setValue("Jane Doe")
  await wrapper.find('input[type="date"]').setValue("2025-01-01")
  await wrapper.find('input[placeholder="$100,000"]').setValue("120000")
  await wrapper.find('input[placeholder="Role"]').setValue("Engineer")
  await wrapper.find("button.btn-primary").trigger("click")
  await flushPromises()
}

const mockAddToast = vi.fn()

function mountDraft() {
  return mount(DocumentDraftView, {
    global: {
      provide: {
        [TOAST_KEY as symbol]: { addToast: mockAddToast },
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


describe("DocumentDraftView fields payload", () => {
  it("sends key, label, type, value and aliases for each filled field", async () => {
    vi.mocked(createDraftJob).mockResolvedValue({ job_id: "job-1" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { draft: "Sample draft text.", warnings: [] },
    } as never)

    const wrapper = mountDraft()
    await fillAndGenerate(wrapper)

    expect(createDraftJob).toHaveBeenCalledWith(
      expect.objectContaining({
        fields: [
          { key: "employeeName", label: "Employee Name", type: undefined, value: "Jane Doe", aliases: undefined },
          { key: "startDate", label: "Start Date", type: "date", value: "2025-01-01", aliases: ["Commencement Date"] },
          { key: "salary", label: "Salary", type: undefined, value: "120000", aliases: ["Compensation", "Annual Salary", "Base Salary"] },
          { key: "position", label: "Position", type: undefined, value: "Engineer", aliases: ["Role", "Job Title", "Employee Title"] },
        ],
      }),
    )
  })
})


describe("DocumentDraftView export", () => {
  it("shows info toast when exporting with no draft generated", async () => {
    const wrapper = mountDraft()
    const buttons = wrapper.findAll("button")
    const exportWordButton = buttons.find((b) => b.text() === "Export to Word")!
    await exportWordButton.trigger("click")
    expect(invoke).not.toHaveBeenCalled()
    expect(mockAddToast).toHaveBeenCalledWith(
      "Generate a draft before exporting.",
      "info",
    )
  })

  it("shows coming soon toast for PDF export and does not invoke", async () => {
    vi.mocked(createDraftJob).mockResolvedValue({ job_id: "job-1" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { draft: "Sample draft text.", warnings: [] },
    } as never)

    const wrapper = mountDraft()
    await fillAndGenerate(wrapper)

    const buttons = wrapper.findAll("button")
    const exportPdfButton = buttons.find((b) => b.text() === "Export to PDF")!
    await exportPdfButton.trigger("click")

    expect(invoke).not.toHaveBeenCalled()
    expect(mockAddToast).toHaveBeenCalledWith("PDF export is coming soon.", "info")
  })

  it("calls export_draft_docx with draft text once generated", async () => {
    vi.mocked(createDraftJob).mockResolvedValue({ job_id: "job-1" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { draft: "Sample draft text.", warnings: [] },
    } as never)

    const wrapper = mountDraft()
    await fillAndGenerate(wrapper)

    const buttons = wrapper.findAll("button")
    const exportWordButton = buttons.find((b) => b.text() === "Export to Word")!
    await exportWordButton.trigger("click")
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith("export_draft_docx", { text: "Sample draft text." })
  })

  it("does not toast success when the user cancels the save dialog", async () => {
    vi.mocked(createDraftJob).mockResolvedValue({ job_id: "job-1" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { draft: "Sample draft text.", warnings: [] },
    } as never)

    const wrapper = mountDraft()
    await fillAndGenerate(wrapper)

    vi.mocked(invoke).mockResolvedValue(false)
    mockAddToast.mockClear()

    const buttons = wrapper.findAll("button")
    const exportWordButton = buttons.find((b) => b.text() === "Export to Word")!
    await exportWordButton.trigger("click")
    await flushPromises()

    expect(mockAddToast).not.toHaveBeenCalledWith(
      "Draft exported as Word document.",
      "success",
    )
  })

  it("shows error toast when export_draft_docx invoke fails", async () => {
    vi.mocked(createDraftJob).mockResolvedValue({ job_id: "job-1" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { draft: "Sample draft text.", warnings: [] },
    } as never)
    // Tauri rejects with the raw string payload, not an Error object.
    vi.mocked(invoke).mockRejectedValue("failed to create file: permission denied")

    const wrapper = mountDraft()
    await fillAndGenerate(wrapper)

    const buttons = wrapper.findAll("button")
    const exportWordButton = buttons.find((b) => b.text() === "Export to Word")!
    await exportWordButton.trigger("click")
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith("export_draft_docx", { text: "Sample draft text." })
    expect(mockAddToast).toHaveBeenCalledWith(
      "failed to create file: permission denied",
      "error",
    )
  })
})
