// ABOUTME: Help entry points share a dialog with the installed version and explicit external links.
// ABOUTME: External opens are user initiated and go through the Tauri opener plugin.
import { mount, flushPromises } from "@vue/test-utils";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import AppShell from "../src/layouts/AppShell.vue";
import HelpDialog from "../src/components/HelpDialog.vue";
import SidebarNav from "../src/modules/navigation/SidebarNav.vue";

vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("vue-router", () => ({
  useRouter: () => ({ push: vi.fn() }),
  useRoute: () => ({ path: "/review" }),
  RouterView: { template: "<div />" },
}));

let wrapper: ReturnType<typeof mount>;

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getVersion).mockResolvedValue("9.8.7-test");
  vi.mocked(openUrl).mockResolvedValue(undefined);
});

afterEach(() => wrapper?.unmount());

async function openHelp() {
  wrapper = mount(AppShell, {
    attachTo: document.body,
    global: { stubs: { TopBar: true } },
  });
  await flushPromises();
  const help = wrapper.findComponent(SidebarNav).findAll("button").find(button => button.text() === "Help");
  expect(help).toBeDefined();
  await help!.trigger("click");
  return wrapper.findComponent(HelpDialog);
}

describe("Help dialog", () => {
  it("shows the version returned by getVersion from the sidebar Help entry", async () => {
    const dialog = await openHelp();
    expect(getVersion).toHaveBeenCalledOnce();
    expect(dialog.text()).toContain("Rumble v9.8.7-test");
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("opens the support address through the opener only on click", async () => {
    const dialog = await openHelp();
    const link = dialog.find('a[href="mailto:ping@elefant.legal"]');
    expect(link.text()).toBe("ping@elefant.legal");
    expect(openUrl).not.toHaveBeenCalled();
    await link.trigger("click");
    expect(openUrl).toHaveBeenCalledExactlyOnceWith("mailto:ping@elefant.legal");
  });

  it("opens Report a problem through the opener only on click", async () => {
    const dialog = await openHelp();
    const link = dialog.find('a[href="https://github.com/elegantelefant/rumble/issues/new"]');
    expect(link.text()).toBe("Report a problem");
    expect(openUrl).not.toHaveBeenCalled();
    await link.trigger("click");
    expect(openUrl).toHaveBeenCalledExactlyOnceWith("https://github.com/elegantelefant/rumble/issues/new");
  });

  it("opens the same dialog from the Help palette command", async () => {
    await openHelp();
    await wrapper.findComponent(HelpDialog).find("button").trigger("click");
    expect(wrapper.findComponent(HelpDialog).exists()).toBe(false);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true }));
    await wrapper.vm.$nextTick();
    const command = wrapper.findAll('[role="option"]').find(item => item.text() === "Help");
    expect(command).toBeDefined();
    await command!.trigger("click");
    expect(wrapper.findAll('[role="dialog"]')).toHaveLength(1);
    expect(wrapper.findComponent(HelpDialog).text()).toContain("Rumble v9.8.7-test");
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("traps keyboard focus and closes on Escape", async () => {
    const dialog = await openHelp();
    const close = dialog.find("button");
    expect(document.activeElement).toBe(close.element);
    await close.trigger("keydown", { key: "Tab" });
    expect(document.activeElement).toBe(dialog.find("a").element);
    await dialog.find("a").trigger("keydown", { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(close.element);
    await close.trigger("keydown", { key: "Escape" });
    expect(wrapper.findComponent(HelpDialog).exists()).toBe(false);
  });

  it("shows an error if the system cannot open a link", async () => {
    const dialog = await openHelp();
    vi.mocked(openUrl).mockRejectedValueOnce(new Error("unavailable"));
    await dialog.find("a").trigger("click");
    await flushPromises();
    expect(dialog.find('[role="alert"]').text()).toContain("Could not open the link");
  });
});
