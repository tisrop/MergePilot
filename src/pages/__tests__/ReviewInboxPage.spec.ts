import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ReviewInboxPage from "@/pages/ReviewInboxPage.vue";
import { reviewInboxList } from "@/api";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import { useReviewInboxStore } from "@/stores/useReviewInboxStore";
import type { Paginated, Platform, ReviewInboxItem } from "@/types";

const storage = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

vi.mock("@/api", () => ({
  reviewInboxList: vi.fn(),
}));

function result(platform: Platform): Paginated<ReviewInboxItem> {
  return {
    items: [
      {
        platform,
        owner: "team",
        repo: `${platform}-repo`,
        repository_full_name: `team/${platform}-repo`,
        categories: ["review_requested"],
        relationships: [
          platform === "gitee" ? "tester" : platform === "gitlab" ? "assignee" : "reviewer",
        ],
        status: {
          status: platform === "github" ? "ready" : "blocked",
          draft: false,
          has_conflicts: platform === "gitlab",
          checks_status: platform === "github" ? "ready" : "pending",
          approvals_status: platform === "github" ? "ready" : "blocked",
          blocking_reasons:
            platform === "gitlab"
              ? [{ code: "approvals_required", message: "审批尚未满足合并要求" }]
              : [],
        },
        summary: {
          number: platform === "github" ? 1 : 2,
          title: `${platform} review`,
          author: { id: 1, login: "dev", name: "Dev", avatar_url: "" },
          state: "open",
          created_at: "2025-01-01T00:00:00Z",
          updated_at: platform === "github" ? "2025-01-01T00:00:00Z" : "2025-01-02T00:00:00Z",
          labels: [],
        },
      },
    ],
    page: 1,
    total_pages: 1,
    total_count: 1,
  };
}

describe("ReviewInboxPage", () => {
  beforeEach(() => {
    storage.clear();
    setActivePinia(createPinia());
    vi.mocked(reviewInboxList).mockReset();
    vi.mocked(reviewInboxList).mockImplementation(async (platform) => result(platform));
  });

  it("展示已登录平台的聚合结果并支持仓库过滤", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/inbox", name: "review-inbox", component: ReviewInboxPage },
        { path: "/pr/:platform/:owner/:repo/:number", name: "pr-detail", component: {} },
      ],
    });
    await router.push("/inbox");
    await router.isReady();
    const auth = useAuthStore();
    auth.platforms.github.isLoggedIn = true;
    auth.platforms.gitlab.isLoggedIn = true;

    const wrapper = mount(ReviewInboxPage, {
      global: {
        plugins: [router],
        stubs: {
          AppLayout: { template: "<div><slot name='header' /><slot /></div>" },
        },
      },
    });
    await flushPromises();

    expect(wrapper.get("h2").text()).toBe("PR 收件箱");
    expect(wrapper.get(".subtitle").text()).toBe("汇总需要你评审、负责或测试的 PR/MR");
    expect(wrapper.findAll(".inbox-card")).toHaveLength(2);
    expect(wrapper.findAll(".repository-name").map((node) => node.text())).toEqual([
      "team/gitlab-repo",
      "team/github-repo",
    ]);
    expect(wrapper.text()).toContain("负责人");
    expect(wrapper.text()).toContain("评审人");
    expect(wrapper.text()).toContain("可合并");
    expect(wrapper.text()).toContain("被阻塞");

    await wrapper.get('input[type="search"]').setValue("github-repo");
    expect(wrapper.findAll(".inbox-card")).toHaveLength(1);
    expect(wrapper.get(".repository-name").text()).toBe("team/github-repo");
  });

  it("打开条目时同步平台和仓库后进入对应 PR", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/inbox", name: "review-inbox", component: ReviewInboxPage },
        { path: "/pr/:platform/:owner/:repo/:number", name: "pr-detail", component: {} },
      ],
    });
    await router.push("/inbox");
    await router.isReady();
    const auth = useAuthStore();
    auth.platforms.gitlab.isLoggedIn = true;
    const repo = useRepoStore();

    const wrapper = mount(ReviewInboxPage, {
      global: {
        plugins: [router],
        stubs: {
          AppLayout: { template: "<div><slot name='header' /><slot /></div>" },
        },
      },
    });
    await flushPromises();

    await wrapper.get(".inbox-card").trigger("click");
    await flushPromises();

    expect(auth.activePlatform).toBe("gitlab");
    expect(repo.activeRepo).toEqual({ owner: "team", repo: "gitlab-repo" });
    expect(router.currentRoute.value.name).toBe("pr-detail");
    expect(router.currentRoute.value.params).toMatchObject({
      platform: "gitlab",
      owner: "team",
      repo: "gitlab-repo",
      number: "2",
    });
  });
  it("范围、平台、角色和合并状态筛选可以展开、选择并更新结果", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/inbox", name: "review-inbox", component: ReviewInboxPage },
        { path: "/pr/:platform/:owner/:repo/:number", name: "pr-detail", component: {} },
      ],
    });
    await router.push("/inbox");
    await router.isReady();
    const auth = useAuthStore();
    auth.platforms.github.isLoggedIn = true;
    auth.platforms.gitlab.isLoggedIn = true;

    const wrapper = mount(ReviewInboxPage, {
      attachTo: document.body,
      global: {
        plugins: [router],
        stubs: {
          AppLayout: { template: "<div><slot name='header' /><slot /></div>" },
        },
      },
    });
    await flushPromises();

    const relationshipSelect = wrapper.get('[aria-label="收件箱角色"]');
    await relationshipSelect.trigger("click");
    expect(wrapper.findAll(".filter-field")[2].text()).toContain("评审人");
    expect(wrapper.findAll(".filter-field")[2].text()).toContain("负责人");
    expect(wrapper.findAll(".filter-field")[2].text()).not.toContain("我创建的");
    await wrapper.get('.dropdown-option[data-value="assignee"]').trigger("click");
    expect(wrapper.findAll(".repository-name").map((node) => node.text())).toEqual([
      "team/gitlab-repo",
    ]);
    await relationshipSelect.trigger("click");
    await wrapper.get('.dropdown-option[data-value="all"]').trigger("click");

    const readinessSelect = wrapper.get('[aria-label="收件箱合并状态"]');
    await readinessSelect.trigger("click");
    expect(wrapper.findAll(".filter-field")[3].text()).toContain("可合并");
    expect(wrapper.findAll(".filter-field")[3].text()).toContain("被阻塞");
    await wrapper.get('.dropdown-option[data-value="ready"]').trigger("click");
    expect(wrapper.findAll(".repository-name").map((node) => node.text())).toEqual([
      "team/github-repo",
    ]);
    await readinessSelect.trigger("click");
    await wrapper.get('.dropdown-option[data-value="all"]').trigger("click");

    const statusSelect = wrapper.get('[aria-label="PR 收件箱分类"]');
    await statusSelect.trigger("click");
    expect(wrapper.findAll(".filter-field")[0].text()).toContain("待我处理");
    expect(wrapper.findAll(".filter-field")[0].text()).toContain("我创建的");

    await wrapper.get('.filter-field .dropdown-option[data-value="authored"]').trigger("click");
    await flushPromises();
    expect(vi.mocked(reviewInboxList)).toHaveBeenCalledWith("github", "authored", 1, 20);
    expect(vi.mocked(reviewInboxList)).toHaveBeenCalledWith("gitlab", "authored", 1, 20);

    const platformSelect = wrapper.get('[aria-label="代码托管平台"]');
    await platformSelect.trigger("click");
    const platformField = wrapper.findAll(".filter-field")[1];
    expect(platformField.text()).toContain("全部已启用平台");
    expect(platformField.text()).toContain("GitHub");
    expect(platformField.text()).toContain("GitLab");

    vi.mocked(reviewInboxList).mockClear();
    await platformField.get('.dropdown-option[data-value="github"]').trigger("click");
    await flushPromises();

    expect(reviewInboxList).toHaveBeenCalledTimes(1);
    expect(reviewInboxList).toHaveBeenCalledWith("github", "authored", 1, 20);
    expect(wrapper.findAll(".repository-name").map((node) => node.text())).toEqual([
      "team/github-repo",
    ]);

    vi.mocked(reviewInboxList).mockClear();
    auth.platforms.github.isLoggedIn = false;
    await flushPromises();

    expect(useReviewInboxStore().filters.platform).toBe("all");
    expect(reviewInboxList).toHaveBeenCalledTimes(1);
    expect(reviewInboxList).toHaveBeenCalledWith("gitlab", "authored", 1, 20);

    wrapper.unmount();
  });

  it("设置关闭的平台不会出现在筛选项中，也不会被收件箱请求", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/inbox", name: "review-inbox", component: ReviewInboxPage },
        { path: "/pr/:platform/:owner/:repo/:number", name: "pr-detail", component: {} },
      ],
    });
    await router.push("/inbox");
    await router.isReady();
    const auth = useAuthStore();
    auth.platforms.github.isLoggedIn = true;
    auth.platforms.gitlab.isLoggedIn = true;
    auth.platformVisibility.github = true;
    auth.platformVisibility.gitlab = false;
    auth.platformVisibility.gitee = true;

    const wrapper = mount(ReviewInboxPage, {
      attachTo: document.body,
      global: {
        plugins: [router],
        stubs: {
          AppLayout: { template: "<div><slot name='header' /><slot /></div>" },
        },
      },
    });
    await flushPromises();

    expect(reviewInboxList).toHaveBeenCalledTimes(1);
    expect(reviewInboxList).toHaveBeenCalledWith("github", "review_requested", 1, 20);

    await wrapper.get('[aria-label="代码托管平台"]').trigger("click");
    const platformField = wrapper.findAll(".filter-field")[1];
    expect(platformField.text()).toContain("GitHub");
    expect(platformField.text()).not.toContain("GitLab");

    wrapper.unmount();
  });

  it("关闭当前筛选平台后回到全部已启用平台，重新开启后恢复聚合请求", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/inbox", name: "review-inbox", component: ReviewInboxPage },
        { path: "/pr/:platform/:owner/:repo/:number", name: "pr-detail", component: {} },
      ],
    });
    await router.push("/inbox");
    await router.isReady();
    const auth = useAuthStore();
    auth.platforms.github.isLoggedIn = true;
    auth.platforms.gitlab.isLoggedIn = true;

    const wrapper = mount(ReviewInboxPage, {
      attachTo: document.body,
      global: {
        plugins: [router],
        stubs: {
          AppLayout: { template: "<div><slot name='header' /><slot /></div>" },
        },
      },
    });
    await flushPromises();

    await wrapper.get('[aria-label="代码托管平台"]').trigger("click");
    await wrapper.get('.dropdown-option[data-value="github"]').trigger("click");
    await flushPromises();
    expect(useReviewInboxStore().filters.platform).toBe("github");

    vi.mocked(reviewInboxList).mockClear();
    auth.setPlatformVisibility("github", false);
    await flushPromises();

    expect(useReviewInboxStore().filters.platform).toBe("all");
    expect(reviewInboxList).toHaveBeenCalledTimes(1);
    expect(reviewInboxList).toHaveBeenCalledWith("gitlab", "review_requested", 1, 20);
    await wrapper.get('[aria-label="代码托管平台"]').trigger("click");
    expect(wrapper.findAll(".filter-field")[1].text()).not.toContain("GitHub");

    vi.mocked(reviewInboxList).mockClear();
    auth.setPlatformVisibility("github", true);
    await flushPromises();

    expect(reviewInboxList).toHaveBeenCalledTimes(2);
    expect(
      vi
        .mocked(reviewInboxList)
        .mock.calls.map(([platform]) => platform)
        .sort(),
    ).toEqual(["github", "gitlab"]);

    wrapper.unmount();
  });
});
