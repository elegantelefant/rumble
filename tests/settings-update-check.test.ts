// ABOUTME: Proves that updates require a manual check and a separate install click.
// ABOUTME: Exercises updater IPC, pending actions and string rejections from the host.
import { flushPromises, mount } from "@vue/test-utils";
import { invoke } from "@tauri-apps/api/core";
import SettingsView from "../src/views/SettingsView.vue";
import { TOAST_KEY } from "../src/composables/toast";

vi.mock("@tauri-apps/api/core", async (importOriginal) => ({
  ...await importOriginal<typeof import("@tauri-apps/api/core")>(),
  invoke: vi.fn(),
}));

const addToast = vi.fn();
const offer = { rid: 42, version: "0.2.0" };
function mountSettings() {
  return mount(SettingsView, {
    global: {
      stubs: { "router-link": true },
      provide: { [TOAST_KEY as symbol]: { addToast } },
    },
  });
}
function updateButton(wrapper: ReturnType<typeof mountSettings>) {
  return wrapper.get('section[aria-label="App updates"] button');
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "plugin:updater|check" ? offer : null,
  );
  // Channel registers callbacks with Tauri even when invoke is mocked.
  vi.stubGlobal("__TAURI_INTERNALS__", { transformCallback: vi.fn(() => 1) });
});
afterEach(() => vi.unstubAllGlobals());

it("shows the offered version and installs only on the second click", async () => {
  const wrapper = mountSettings();
  await flushPromises();
  expect(invoke).not.toHaveBeenCalledWith("plugin:updater|check");
  expect(updateButton(wrapper).text()).toBe("Check for updates");
  await updateButton(wrapper).trigger("click");
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith("plugin:updater|check");
  expect(wrapper.get('[role="status"]').text()).toBe("Version 0.2.0 is available.");
  expect(updateButton(wrapper).text()).toBe("Install version 0.2.0");
  expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "plugin:updater|download_and_install")).toBe(false);
  await updateButton(wrapper).trigger("click");
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith("plugin:updater|download_and_install", expect.objectContaining({ rid: 42 }));
  expect(wrapper.text()).toContain("Update installed. Restart Rumble");
  wrapper.unmount();
});

it("reports a string rejection from checking and allows retry", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "plugin:updater|check") throw "network unavailable";
    return null;
  });
  const wrapper = mountSettings();
  await updateButton(wrapper).trigger("click");
  await flushPromises();
  expect(addToast).toHaveBeenCalledWith("Could not update Rumble: network unavailable", "error");
  expect(updateButton(wrapper).attributes("disabled")).toBeUndefined();
  expect(updateButton(wrapper).text()).toBe("Check for updates");
});

it("reports an install string rejection and retains the offered version for retry", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "plugin:updater|check") return offer;
    if (command === "plugin:updater|download_and_install") throw "signature verification failed";
    return null;
  });
  const wrapper = mountSettings();
  await updateButton(wrapper).trigger("click");
  await flushPromises();
  await updateButton(wrapper).trigger("click");
  await flushPromises();
  expect(addToast).toHaveBeenCalledWith("Could not update Rumble: signature verification failed", "error");
  expect(updateButton(wrapper).text()).toBe("Install version 0.2.0");
  expect(updateButton(wrapper).attributes("disabled")).toBeUndefined();
  wrapper.unmount();
});

it("shows no update and prevents duplicate checks while pending", async () => {
  let finish!: (value: null) => void;
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "plugin:updater|check" ? new Promise<null>((resolve) => { finish = resolve; }) : null,
  );
  const wrapper = mountSettings();
  await updateButton(wrapper).trigger("click");
  expect(updateButton(wrapper).attributes("disabled")).toBeDefined();
  finish(null);
  await flushPromises();
  expect(wrapper.text()).toContain("You're up to date.");
  expect(updateButton(wrapper).attributes("disabled")).toBeUndefined();
});
