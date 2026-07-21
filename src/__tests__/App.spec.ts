import { createPinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { checkForUpdates } from "@/api";
import { useUpdateStore } from "@/stores/useUpdateStore";
import App from "../App.vue";

const storage = new Map<string, string>();
const noUpdate = {
  current_version: "0.7.0",
  available: false,
  version: null,
  notes: null,
  published_at: null,
  update_mode: "installer" as const,
};

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

    const wrapper = mount(App, {
      global: {
        plugins: [pinia, router],
        stubs: { CommandPalette: true, NotificationManager: true },
      },
    });
    await flushPromises();

    expect(router.currentRoute.value.path).toBe("/pr");
    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(useUpdateStore(pinia).updateResult?.version).toBe("0.4.0");
    expect(window.__goToSettings).toBeTypeOf("function");
    expect(window.__openCommandPalette).toBeTypeOf("function");
    wrapper.unmount();
    expect(window.__goToSettings).toBeUndefined();
    expect(window.__openCommandPalette).toBeUndefined();
  });

  it("普通参数路由变化时复用页面组件", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue(noUpdate);
    const mounted = vi.fn();
    const page = {
      data: () => ({ marker: "" }),
      mounted,
      template: '<input data-testid="route-marker" v-model="marker" />',
    };
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/workspace/:section", name: "workspace", component: page }],
    });
    await router.push("/workspace/first");
    await router.isReady();
    const wrapper = mount(App, {
      global: {
        plugins: [createPinia(), router],
        stubs: { CommandPalette: true, NotificationManager: true },
      },
    });
    await wrapper.get<HTMLInputElement>('[data-testid="route-marker"]').setValue("保留状态");

    await router.push("/workspace/second");
    await flushPromises();

    expect(mounted).toHaveBeenCalledOnce();
    expect(wrapper.get<HTMLInputElement>('[data-testid="route-marker"]').element.value).toBe(
      "保留状态",
    );
    wrapper.unmount();
  });

  it("切换 PR 详情编号时重新挂载详情组件", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue(noUpdate);
    const mounted = vi.fn();
    const detailPage = { mounted, template: "<div>PR 详情</div>" };
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        {
          path: "/pr/:platform/:owner/:repo/:number",
          name: "pr-detail",
          component: detailPage,
        },
      ],
    });
    await router.push("/pr/github/team/repo/1");
    await router.isReady();
    const wrapper = mount(App, {
      global: {
        plugins: [createPinia(), router],
        stubs: { CommandPalette: true, NotificationManager: true },
      },
    });

    await router.push("/pr/github/team/repo/2");
    await flushPromises();

    expect(mounted).toHaveBeenCalledTimes(2);
    wrapper.unmount();
  });
});
