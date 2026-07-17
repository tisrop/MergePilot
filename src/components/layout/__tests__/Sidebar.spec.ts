import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Sidebar from "@/components/layout/Sidebar.vue";
import { useAuthStore } from "@/stores/useAuthStore";
import { usePrStore } from "@/stores/usePrStore";
import { useRepoStore } from "@/stores/useRepoStore";

const storage = new Map<string, string>();

vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

vi.mock("@/api", () => ({
  authStatus: vi.fn(),
  repoList: vi.fn(),
  prList: vi.fn(),
  prDetail: vi.fn(),
  prDiff: vi.fn(),
  prMerge: vi.fn(),
  prClose: vi.fn(),
  prReopen: vi.fn(),
}));

describe("Sidebar", () => {
  beforeEach(() => {
    localStorage.clear();
    setActivePinia(createPinia());
  });

  it("Diff 专注模式默认折叠整个侧栏并保留可访问导航", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/", name: "home", component: { template: "<div />" } },
        { path: "/pr", name: "pr-list", component: { template: "<div />" } },
        { path: "/issue", name: "issue-list", component: { template: "<div />" } },
        { path: "/settings", name: "settings", component: { template: "<div />" } },
      ],
    });
    await router.push("/pr");
    await router.isReady();

    const auth = useAuthStore();
    auth.platforms.github.isLoggedIn = true;
    auth.platforms.gitlab.isLoggedIn = true;
    auth.platforms.gitee.isLoggedIn = true;
    useRepoStore().setActiveRepo("tisrop", "MergeBeacon");

    const wrapper = mount(Sidebar, {
      props: { isDiffFocusMode: true },
      global: { plugins: [router] },
    });

    expect(wrapper.get(".sidebar").classes()).toContain("is-collapsed");
    expect(wrapper.find(".repo-section").exists()).toBe(true);
    expect(wrapper.get('[aria-label="拉取请求（PR）"]').attributes("title")).toBe("拉取请求（PR）");
    expect(wrapper.get('[aria-label="Issues"]').attributes("title")).toBe("Issues");
    expect(wrapper.get('[aria-label="设置"]').attributes("title")).toBe("设置");
    expect(wrapper.get(".compact-platform").attributes("aria-label")).toBe("当前平台：GitHub");
    expect(wrapper.get(".compact-repo-name").text()).toBe("MergeBeacon");
    expect(wrapper.get(".compact-repo").attributes("title")).toBe("当前仓库：tisrop/MergeBeacon");
    expect(wrapper.get(".compact-repo").attributes("aria-label")).toBe(
      "当前仓库：tisrop/MergeBeacon",
    );
    expect(wrapper.get('[aria-label="展开侧栏"]').attributes("aria-expanded")).toBe("false");
  });

  it("持久化 Diff 专注模式的手动展开选择", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/pr", name: "pr-list", component: { template: "<div />" } }],
    });
    await router.push("/pr");
    await router.isReady();

    const prepareAuth = () => {
      const auth = useAuthStore();
      auth.platforms.github.isLoggedIn = true;
      auth.platforms.gitlab.isLoggedIn = true;
      auth.platforms.gitee.isLoggedIn = true;
    };
    prepareAuth();

    const wrapper = mount(Sidebar, {
      props: { isDiffFocusMode: true },
      global: { plugins: [router] },
    });
    await wrapper.get('[aria-label="展开侧栏"]').trigger("click");

    expect(wrapper.get(".sidebar").classes()).not.toContain("is-collapsed");
    expect(wrapper.get('[aria-label="折叠侧栏"]').attributes("aria-expanded")).toBe("true");
    expect(localStorage.getItem("mergebeacon:diff-sidebar-expanded")).toBe("true");
    wrapper.unmount();

    setActivePinia(createPinia());
    prepareAuth();
    const restoredWrapper = mount(Sidebar, {
      props: { isDiffFocusMode: true },
      global: { plugins: [router] },
    });

    expect(restoredWrapper.get(".sidebar").classes()).not.toContain("is-collapsed");
    expect(restoredWrapper.find('[aria-label="折叠侧栏"]').exists()).toBe(true);

    await restoredWrapper.get('[aria-label="折叠侧栏"]').trigger("click");
    expect(restoredWrapper.get(".sidebar").classes()).toContain("is-collapsed");
    expect(localStorage.getItem("mergebeacon:diff-sidebar-expanded")).toBe("false");
  });

  it("非 Diff 页面始终展示完整侧栏", async () => {
    localStorage.setItem("mergebeacon:diff-sidebar-expanded", "false");
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/issue", name: "issue-list", component: { template: "<div />" } }],
    });
    await router.push("/issue");
    await router.isReady();

    const auth = useAuthStore();
    auth.platforms.github.isLoggedIn = true;
    auth.platforms.gitlab.isLoggedIn = true;
    auth.platforms.gitee.isLoggedIn = true;
    const wrapper = mount(Sidebar, { global: { plugins: [router] } });

    expect(wrapper.get(".sidebar").classes()).not.toContain("is-collapsed");
    expect(wrapper.find(".repo-section").exists()).toBe(true);
    expect(wrapper.find(".compact-repo").exists()).toBe(false);
    expect(wrapper.find('[aria-label="展开侧栏"]').exists()).toBe(false);
    expect(wrapper.find('[aria-label="折叠侧栏"]').exists()).toBe(false);
  });

  it("从 PR 详情切换平台时清空旧上下文并返回 PR 列表", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/pr", name: "pr-list", component: { template: "<div />" } },
        {
          path: "/pr/:platform/:owner/:repo/:number",
          name: "pr-detail",
          component: { template: "<div />" },
        },
      ],
    });
    await router.push("/pr/github/owner/repo/1");
    await router.isReady();

    const auth = useAuthStore();
    auth.platforms.github.isLoggedIn = true;
    auth.platforms.gitlab.isLoggedIn = true;
    auth.platforms.gitee.isLoggedIn = true;
    const pr = usePrStore();
    pr.currentPr = {
      summary: {
        number: 1,
        title: "旧平台 PR",
        author: { id: 1, login: "user", name: "User", avatar_url: "" },
        state: "open",
        created_at: "",
        updated_at: "",
        labels: [],
      },
      body: "",
      source_branch: "feature",
      target_branch: "main",
      mergeable: true,
      head_sha: "old-sha",
      base_sha: "base-sha",
    };

    const wrapper = mount(Sidebar, { global: { plugins: [router] } });
    const gitlabButton = wrapper
      .findAll(".platform-selector button")
      .find((button) => button.text() === "GitLab");
    expect(gitlabButton).toBeDefined();

    await gitlabButton!.trigger("click");
    await flushPromises();

    expect(auth.activePlatform).toBe("gitlab");
    expect(pr.currentPr).toBeNull();
    expect(router.currentRoute.value.name).toBe("pr-list");
  });
});
