import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PrDetailPage from "@/pages/PrDetailPage.vue";
import type { PrDetail, PrMergeReadiness, User } from "@/types";
import { APP_COMMAND_EVENT } from "@/types/commands";
import {
  persistPrCreateWarnings,
  PR_CREATE_WARNING_QUERY,
  readPrCreateWarnings,
} from "@/utils/prCreateWarnings";

const mocks = vi.hoisted(() => ({
  router: {
    push: vi.fn(),
    replace: vi.fn().mockResolvedValue(undefined),
  },
  route: {
    params: { platform: "github", owner: "owner", repo: "repo", number: "42" },
    query: {} as Record<string, string | undefined>,
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
    updateMetadata: vi.fn(),
  },
  reviewInboxStore: {
    applyPrSummary: vi.fn(),
  },
  capabilityStore: {
    values: {
      github: {
        platform: "github",
        review_events: ["comment", "approve", "request_changes"],
        merge_strategies: ["merge", "squash", "rebase"],
        supports_fork_context: true,
        supports_issue_auto_close: true,
        supports_compare_diff: true,
        supports_review_thread_resolution: false,
        supports_remote_file_viewed_state: false,
        supports_pr_title_body_edit: true,
        supports_pr_draft_toggle: true,
        supports_pr_reviewer_management: true,
        supports_pr_assignee_management: true,
        supports_pr_label_management: true,
        supports_pr_milestone_management: true,
      },
    },
    errors: {},
    load: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock("vue-router", () => ({
  useRoute: () => mocks.route,
  useRouter: () => mocks.router,
}));
vi.mock("@/stores/useAuthStore", () => ({ useAuthStore: () => mocks.authStore }));
vi.mock("@/stores/usePrStore", () => ({ usePrStore: () => mocks.prStore }));
vi.mock("@/stores/useReviewInboxStore", () => ({
  useReviewInboxStore: () => mocks.reviewInboxStore,
}));
vi.mock("@/stores/useCapabilityStore", () => ({
  useCapabilityStore: () => mocks.capabilityStore,
}));
vi.mock("@/api", () => ({ reviewCommentAdd: vi.fn() }));

enableAutoUnmount(afterEach);

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
  draft: false,
  reviewers: [],
  assignees: [],
  milestone: null,
  metadata_permissions: {
    can_edit_title_body: true,
    can_toggle_draft: true,
    can_manage_reviewers: true,
    can_manage_assignees: true,
    can_manage_labels: true,
    can_manage_milestone: true,
  },
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

function mountPage(stubs: Record<string, unknown> = {}) {
  return mount(PrDetailPage, {
    global: {
      stubs: {
        AppLayout: {
          props: { isDiffFocusMode: Boolean },
          template: `<main data-testid="app-layout" :data-focus-mode="isDiffFocusMode ? 'true' : 'false'">
            <slot name="header" /><div class="content-body"><slot /></div>
          </main>`,
        },
        DiffViewer: true,
        ReviewForm: true,
        ReviewList: true,
        AiReviewPanel: true,
        MergeReadinessPanel: true,
        ...stubs,
      },
    },
  });
}

describe("PrDetailPage 关闭权限", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.router.replace.mockResolvedValue(undefined);
    mocks.route.query = {};
    window.sessionStorage.clear();
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
    mocks.prStore.updateMetadata.mockResolvedValue({
      detail,
      updated_fields: ["title_body"],
      failures: [],
    });
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

  it("切换 Diff 后保留 AI 评审面板状态", async () => {
    const mounted = vi.fn();
    const wrapper = mountPage({
      AiReviewPanel: {
        data: () => ({ result: "" }),
        mounted,
        template: `<input data-testid="ai-review-result" v-model="result" />`,
      },
    });
    const tab = (label: string) =>
      wrapper.findAll(".tabs button").find((button) => button.text() === label)!;

    await tab("AI 评审").trigger("click");
    await wrapper.get<HTMLInputElement>('[data-testid="ai-review-result"]').setValue("已完成评审");
    await tab("Diff").trigger("click");
    await tab("AI 评审").trigger("click");

    expect(mounted).toHaveBeenCalledOnce();
    expect(wrapper.get<HTMLInputElement>('[data-testid="ai-review-result"]').element.value).toBe(
      "已完成评审",
    );
  });

  it("从 AI 建议切换到 Diff 并传递受控定位请求", async () => {
    const wrapper = mountPage({
      AiReviewPanel: {
        emits: ["locateSuggestion"],
        template: `<button data-testid="locate-ai-suggestion" @click="$emit('locateSuggestion', {
          file: 'src/main.ts',
          line_start: 18,
          line_end: 18,
          severity: 'major',
          category: '逻辑',
          description: '测试建议',
          suggestion: null
        })">定位建议</button>`,
      },
      DiffViewer: {
        props: ["locationRequest"],
        emits: ["locationResult"],
        template: `<section data-testid="diff-location-request">
          <span>{{ locationRequest?.path }}:{{ locationRequest?.line }}</span>
          <button
            data-testid="emit-stale-location-result"
            @click="$emit('locationResult', { id: 0, success: false, message: '旧请求错误' })"
          >旧结果</button>
          <button
            data-testid="emit-location-failure"
            @click="$emit('locationResult', { id: locationRequest.id, success: false, message: '目标行不存在' })"
          >失败</button>
        </section>`,
      },
    });
    const aiTab = wrapper.findAll(".tabs button").find((button) => button.text() === "AI 评审");
    expect(aiTab).toBeDefined();
    await aiTab!.trigger("click");
    const contentBody = wrapper.get<HTMLElement>(".content-body");
    const tabs = wrapper.get<HTMLElement>(".tabs");
    contentBody.element.scrollTop = 480;
    contentBody.element.getBoundingClientRect = () => ({ top: 0 }) as DOMRect;
    tabs.element.getBoundingClientRect = () => ({ top: -480 }) as DOMRect;

    await wrapper.get('[data-testid="locate-ai-suggestion"]').trigger("click");

    const returnToAiButton = wrapper
      .findAll(".tabs button")
      .find((button) => button.text() === "返回 AI 评审");
    expect(returnToAiButton).toBeDefined();
    expect(contentBody.element.scrollTop).toBe(0);
    expect(wrapper.get('[data-testid="diff-location-request"]').text()).toContain("src/main.ts:18");
    expect(
      wrapper
        .findAll(".tabs button")
        .find((button) => button.text() === "Diff")
        ?.classes(),
    ).toContain("active");

    await wrapper.get('[data-testid="emit-stale-location-result"]').trigger("click");
    expect(wrapper.find(".diff-location-error").exists()).toBe(false);
    await wrapper.get('[data-testid="emit-location-failure"]').trigger("click");
    expect(wrapper.get(".diff-location-error").text()).toContain("目标行不存在");

    await returnToAiButton!.trigger("click");
    expect(contentBody.element.scrollTop).toBe(480);
    expect(wrapper.get('[data-testid="locate-ai-suggestion"]').isVisible()).toBe(true);
    expect(
      wrapper
        .findAll(".tabs button")
        .find((button) => button.text() === "AI 评审")
        ?.classes(),
    ).toContain("active");
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
  it("保存元数据后同步详情列表和已加载收件箱摘要", async () => {
    const updatedDetail: PrDetail = {
      ...detail,
      summary: { ...detail.summary, title: "更新后的标题" },
    };
    mocks.prStore.updateMetadata.mockResolvedValue({
      detail: updatedDetail,
      updated_fields: ["title_body"],
      failures: [],
    });
    const wrapper = mountPage();

    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await wrapper.get('[data-testid="metadata-title"]').setValue("更新后的标题");
    await wrapper.get(".metadata-form").trigger("submit");
    await flushPromises();

    expect(mocks.prStore.updateMetadata).toHaveBeenCalledWith(
      "github",
      "owner",
      "repo",
      42,
      expect.objectContaining({
        title: "更新后的标题",
        expected_updated_at: detail.summary.updated_at,
      }),
    );
    expect(mocks.reviewInboxStore.applyPrSummary).toHaveBeenCalledWith(
      "github",
      "owner",
      "repo",
      updatedDetail.summary,
    );
    expect(wrapper.text()).toContain("元数据已更新");
  });

  it("忽略 store 判定为过期的元数据响应", async () => {
    mocks.prStore.updateMetadata.mockResolvedValue(null);
    const wrapper = mountPage();

    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await wrapper.get(".metadata-form").trigger("submit");
    await flushPromises();

    expect(mocks.reviewInboxStore.applyPrSummary).not.toHaveBeenCalled();
    expect(wrapper.text()).not.toContain("元数据已更新");
    expect(wrapper.text()).not.toContain("保存 PR / MR 元数据失败");
  });

  it("元数据部分成功时保留成功提示并展示失败项", async () => {
    mocks.prStore.updateMetadata.mockResolvedValue({
      detail,
      updated_fields: ["labels"],
      failures: [{ field: "reviewers", message: "部分评审者不存在" }],
    });
    const wrapper = mountPage();

    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await wrapper.get(".metadata-form").trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain("部分元数据已更新，请检查失败项。");
    expect(wrapper.text()).toContain("部分评审者不存在");
  });

  it("从会话暂存恢复创建后的部分成功警告", async () => {
    persistPrCreateWarnings("github", "owner", "repo", 42, ["部分标签不存在"]);
    mocks.route.query = { [PR_CREATE_WARNING_QUERY]: "1" };

    const wrapper = mountPage();
    await flushPromises();

    expect(wrapper.text()).toContain("PR / MR 已创建，但部分后续操作失败。");
    expect(wrapper.text()).toContain("部分标签不存在");
    expect(mocks.router.replace).toHaveBeenCalledWith({ query: {} });
    expect(readPrCreateWarnings("github", "owner", "repo", 42)).toEqual([]);
  });

  it("会话暂存缺失时仍根据 query 展示部分成功警告", async () => {
    mocks.route.query = { [PR_CREATE_WARNING_QUERY]: "1" };

    const wrapper = mountPage();
    await flushPromises();

    expect(wrapper.text()).toContain("PR / MR 已创建，但部分后续操作失败。");
    expect(wrapper.text()).toContain("部分参与者或标签可能未能写入");
    expect(mocks.router.replace).toHaveBeenCalledWith({ query: {} });
  });

  it("响应命令面板的 AI 评审和提交评审入口", async () => {
    const wrapper = mountPage();
    await flushPromises();

    window.dispatchEvent(
      new CustomEvent(APP_COMMAND_EVENT, { detail: { type: "start_ai_review" } }),
    );
    await flushPromises();
    const aiTab = wrapper
      .findAll(".tabs button")
      .find((button) => button.text().includes("AI 评审"));
    expect(aiTab?.classes()).toContain("active");

    window.dispatchEvent(
      new CustomEvent(APP_COMMAND_EVENT, { detail: { type: "prepare_review" } }),
    );
    await flushPromises();
    const diffTab = wrapper
      .findAll(".tabs button")
      .find((button) => button.text().includes("Diff"));
    expect(diffTab?.classes()).toContain("active");
    wrapper.unmount();
  });
});
