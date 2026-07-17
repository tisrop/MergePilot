import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PrDetailPage from "@/pages/PrDetailPage.vue";
import type { PrDetail, PrMergeReadiness, User } from "@/types";

const mocks = vi.hoisted(() => ({
  router: {
    push: vi.fn(),
  },
  authStore: {
    platforms: {
      github: { user: null as User | null, isLoggedIn: true },
      gitlab: { user: null as User | null, isLoggedIn: false },
      gitee: { user: null as User | null, isLoggedIn: false },
    },
  },
  prStore: {
    currentPr: null as PrDetail | null,
    diff: null,
    mergeReadiness: null as PrMergeReadiness | null,
    readinessLoading: false,
    readinessError: null as string | null,
    error: null as string | null,
    fetchPrDetail: vi.fn().mockResolvedValue(undefined),
    fetchPrDiff: vi.fn().mockResolvedValue(undefined),
    fetchMergeReadiness: vi.fn().mockResolvedValue(undefined),
    mergePr: vi.fn(),
    closePr: vi.fn().mockResolvedValue(undefined),
    reopenPr: vi.fn(),
  },
  capabilityStore: {
    values: {
      github: {
        platform: "github",
        review_events: ["comment", "approve", "request_changes"],
        merge_strategies: ["merge", "squash", "rebase"],
        supports_fork_context: true,
        supports_issue_auto_close: true,
      },
    },
    errors: {},
    load: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock("vue-router", () => ({
  useRoute: () => ({
    params: { platform: "github", owner: "owner", repo: "repo", number: "42" },
  }),
  useRouter: () => mocks.router,
}));
vi.mock("@/stores/useAuthStore", () => ({ useAuthStore: () => mocks.authStore }));
vi.mock("@/stores/usePrStore", () => ({ usePrStore: () => mocks.prStore }));
vi.mock("@/stores/useCapabilityStore", () => ({
  useCapabilityStore: () => mocks.capabilityStore,
}));
vi.mock("@/api", () => ({ reviewCommentAdd: vi.fn() }));

const author: User = {
  id: 7,
  login: "pr-author",
  name: "PR Author",
  avatar_url: "",
};

const detail: PrDetail = {
  summary: {
    number: 42,
    title: "权限测试",
    author,
    state: "open",
    created_at: "",
    updated_at: "",
    labels: [],
  },
  body: "",
  source_branch: "feature",
  target_branch: "main",
  mergeable: true,
  head_sha: "abc123",
  base_sha: "base-sha",
};

const readiness: PrMergeReadiness = {
  status: "blocked",
  head_sha: "abc123",
  mergeable: true,
  draft: false,
  has_conflicts: false,
  checks_status: "ready",
  approvals_status: "ready",
  approvals_required: null,
  approvals_received: null,
  has_merge_permission: false,
  branch_behind: false,
  blocking_reasons: [],
};

function mountPage() {
  return mount(PrDetailPage, {
    global: {
      stubs: {
        AppLayout: {
          props: { isDiffFocusMode: Boolean },
          template: `<main data-testid="app-layout" :data-focus-mode="isDiffFocusMode ? 'true' : 'false'">
            <slot name="header" /><slot />
          </main>`,
        },
        DiffViewer: true,
        ReviewForm: true,
        ReviewList: true,
        AiReviewPanel: true,
        MergeReadinessPanel: true,
      },
    },
  });
}

describe("PrDetailPage 关闭权限", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.authStore.platforms.github.user = {
      id: 99,
      login: "reviewer",
      name: "Reviewer",
      avatar_url: "",
    };
    mocks.prStore.currentPr = detail;
    mocks.prStore.mergeReadiness = { ...readiness };
    mocks.prStore.readinessLoading = false;
    mocks.prStore.readinessError = null;
    mocks.prStore.error = null;
  });

  it("点击返回按钮稳定跳转到 PR 列表", async () => {
    const wrapper = mountPage();
    const button = wrapper.get('[data-testid="back-to-pr-list"]');

    expect(button.attributes("title")).toBe("返回 PR 列表");
    expect(button.attributes("aria-label")).toBe("返回 PR 列表");
    await button.trigger("click");

    expect(mocks.router.push).toHaveBeenCalledWith({ name: "pr-list" });
  });

  it("非作者且没有仓库写入权限时禁用关闭按钮", async () => {
    const wrapper = mountPage();
    const button = wrapper.get('[data-testid="close-pr-button"]');

    expect(button.attributes("disabled")).toBeDefined();
    expect(button.attributes("title")).toBe("只有 PR 作者或具备仓库写入权限的成员才能关闭 PR");
    await button.trigger("click");
    expect(mocks.prStore.closePr).not.toHaveBeenCalled();
  });

  it("PR 作者没有合并权限时仍可关闭自己的 PR", () => {
    mocks.authStore.platforms.github.user = { ...author, login: "PR-AUTHOR" };
    const wrapper = mountPage();
    const button = wrapper.get('[data-testid="close-pr-button"]');

    expect(button.attributes("disabled")).toBeUndefined();
  });

  it("具备仓库写入权限的非作者可以关闭 PR", () => {
    mocks.prStore.mergeReadiness = { ...readiness, has_merge_permission: true };
    const wrapper = mountPage();
    const button = wrapper.get('[data-testid="close-pr-button"]');

    expect(button.attributes("disabled")).toBeUndefined();
  });

  it("平台未返回权限时保守禁用关闭按钮", () => {
    mocks.prStore.mergeReadiness = { ...readiness, has_merge_permission: null };
    const wrapper = mountPage();
    const button = wrapper.get('[data-testid="close-pr-button"]');

    expect(button.attributes("disabled")).toBeDefined();
    expect(button.attributes("title")).toBe("平台未返回当前账号的关闭权限");
  });

  it("合并状态未知但没有明确阻断条件时允许尝试合并", async () => {
    mocks.prStore.mergeReadiness = {
      ...readiness,
      status: "unknown",
      checks_status: "unknown",
      has_merge_permission: null,
      blocking_reasons: [],
    };
    mocks.prStore.mergePr.mockResolvedValue({ merged: true, issue_close_failures: [] });
    const wrapper = mountPage();
    const button = wrapper.get(".merge-main");

    expect(button.attributes("disabled")).toBeUndefined();
    await button.trigger("click");
    expect(mocks.prStore.mergePr).toHaveBeenCalledOnce();
  });

  it("合并状态未知但存在已确认阻断条件时仍禁用合并", () => {
    mocks.prStore.mergeReadiness = {
      ...readiness,
      status: "unknown",
      checks_status: "unknown",
      has_merge_permission: null,
      blocking_reasons: [{ code: "platform_blocked", message: "平台规则阻止合并" }],
    };
    const wrapper = mountPage();

    expect(wrapper.get(".merge-main").attributes("disabled")).toBeDefined();
  });

  it("仅在 Diff 标签启用侧栏专注模式", async () => {
    const wrapper = mountPage();

    expect(wrapper.get('[data-testid="app-layout"]').attributes("data-focus-mode")).toBe("true");
    const reviewsTab = wrapper
      .findAll(".tabs button")
      .find((button) => button.text() === "评审意见");
    expect(reviewsTab).toBeDefined();
    await reviewsTab!.trigger("click");

    expect(wrapper.get('[data-testid="app-layout"]').attributes("data-focus-mode")).toBe("false");
  });
});
