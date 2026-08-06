// ABOUTME: Tests for BriefcasesView.vue waitlist button.
// ABOUTME: Covers click behavior, toast feedback, and disabled state after joining.

import { mount } from "@vue/test-utils"
import BriefcasesView from "../src/views/BriefcasesView.vue"
import { TOAST_KEY } from "../src/composables/toast"

function mountBriefcases() {
  const addToast = vi.fn()
  const wrapper = mount(BriefcasesView, {
    global: {
      provide: {
        [TOAST_KEY as symbol]: { addToast },
      },
    },
  })
  return { wrapper, addToast }
}

describe("BriefcasesView", () => {
  it("renders the waitlist button", () => {
    const { wrapper } = mountBriefcases()
    expect(wrapper.text()).toContain("Join Waitlist")
  })

  it("shows a success toast when clicking Join Waitlist", async () => {
    const { wrapper, addToast } = mountBriefcases()
    await wrapper.find("button.btn-primary").trigger("click")
    expect(addToast).toHaveBeenCalledWith(
      "You're on the Briefcases waitlist — we'll email you at launch.",
      "success",
    )
  })

  it("disables the button and updates its label after joining", async () => {
    const { wrapper } = mountBriefcases()
    const button = wrapper.find("button.btn-primary")
    await button.trigger("click")
    expect(wrapper.text()).toContain("You're on the list")
    expect(button.attributes("disabled")).toBeDefined()
  })

  it("does not show a toast a second time when clicked again", async () => {
    const { wrapper, addToast } = mountBriefcases()
    const button = wrapper.find("button.btn-primary")
    await button.trigger("click")
    await button.trigger("click")
    expect(addToast).toHaveBeenCalledTimes(1)
  })
})
