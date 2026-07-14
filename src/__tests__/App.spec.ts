import { createPinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { checkForUpdates } from "@/api";
import { useUpdateStore } from "@/stores/useUpdateStore";
import App from "../App.vue";

const storage = new Map<string, string>();

vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

vi.mock("@/api", () => ({
  checkForUpdates: vi.fn(),
}));

describe("App", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    storage.clear();
    vi.mocked(checkForUpdates).mockReset();
  });

  it("未进入设置页也会在应用挂载时执行后台更新检查", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.5",
      available: true,
      version: "0.4.0",
      notes: "更新说明",
      published_at: "2026-07-13",
      update_mode: "installer",
    });
    const pinia = createPinia();
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/pr", component: { template: "<div>PR 列表</div>" } }],
    });
    await router.push("/pr");
    await router.isReady();

    const wrapper = mount(App, { global: { plugins: [pinia, router] } });
    await flushPromises();

    expect(router.currentRoute.value.path).toBe("/pr");
    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(useUpdateStore(pinia).updateResult?.version).toBe("0.4.0");
    expect(window.__goToSettings).toBeTypeOf("function");
    wrapper.unmount();
    expect(window.__goToSettings).toBeUndefined();
  });
});
