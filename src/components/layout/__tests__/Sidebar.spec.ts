import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Sidebar from "@/components/layout/Sidebar.vue";
import { useAuthStore } from "@/stores/useAuthStore";
import { usePrStore } from "@/stores/usePrStore";

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

describe("Sidebar 平台切换", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
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
