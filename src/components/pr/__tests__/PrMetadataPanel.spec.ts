import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PrMetadataPanel from "../PrMetadataPanel.vue";
import { prLabels, prParticipantSuggestions } from "@/api";
import type { PlatformCapabilities, PrDetail, PrLabel, PrMetadataUpdate, User } from "@/types";

vi.mock("@/api", () => ({
  prLabels: vi.fn(),
  prParticipantSuggestions: vi.fn(),
}));

const author: User = { id: 1, login: "author", name: "Author", avatar_url: "" };
const reviewer: User = { id: 2, login: "reviewer", name: "Reviewer", avatar_url: "" };
const assignee: User = { id: 3, login: "assignee", name: "Assignee", avatar_url: "" };

const detail: PrDetail = {
  summary: {
    number: 42,
    title: "原始标题",
    author,
    state: "open",
    created_at: "2026-07-18T00:00:00Z",
    updated_at: "2026-07-18T01:00:00Z",
    labels: ["bug", "review"],
  },
  body: "原始描述",
  source_branch: "feature",
  target_branch: "main",
  mergeable: true,
  head_sha: "head-sha",
  base_sha: "base-sha",
  draft: false,
  reviewers: [reviewer],
  assignees: [assignee],
  milestone: { id: 7, number: 7, title: "0.6.0" },
  metadata_permissions: {
    can_edit_title_body: true,
    can_toggle_draft: true,
    can_manage_reviewers: true,
    can_manage_assignees: true,
    can_manage_labels: true,
    can_manage_milestone: true,
  },
};

function capabilities(overrides: Partial<PlatformCapabilities> = {}): PlatformCapabilities {
  return {
    platform: "github",
    review_events: ["comment", "approve", "request_changes"],
    merge_strategies: ["merge", "squash", "rebase"],
    supports_fork_context: true,
    supports_issue_auto_close: true,
    supports_compare_diff: true,
    supports_review_thread_resolution: true,
    supports_remote_file_viewed_state: true,
    supports_pr_title_body_edit: true,
    supports_pr_draft_toggle: true,
    supports_pr_reviewer_management: true,
    supports_pr_assignee_management: true,
    supports_pr_label_management: true,
    supports_pr_milestone_management: true,
    supports_pr_creation: true,
    merge_queue_kind: "merge_queue",
    ...overrides,
  };
}

function mountPanel(
  props: Partial<{
    detail: PrDetail;
    capabilities: PlatformCapabilities | null;
    saving: boolean;
    statusMessage: string;
    errorMessage: string;
  }> = {},
) {
  return mount(PrMetadataPanel, {
    props: {
      detail,
      platform: "github",
      owner: "owner",
      repo: "repo",
      capabilities: capabilities(),
      saving: false,
      ...props,
    },
  });
}

describe("PrMetadataPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(prParticipantSuggestions).mockResolvedValue([
      reviewer,
      assignee,
      { id: 4, login: "alice", name: "Alice", avatar_url: "" },
      { id: 5, login: "Bob", name: "Bob", avatar_url: "" },
      { id: 6, login: "carol", name: "Carol", avatar_url: "" },
    ]);
    vi.mocked(prLabels).mockResolvedValue([
      { name: "bug", color: "d73a4a", description: "需要修复的问题" },
      { name: "feature", color: "a2eeef", description: "新功能" },
      { name: "documentation", color: null, description: null },
    ]);
  });

  it("展示元数据并按当前详情初始化编辑表单", async () => {
    const wrapper = mountPanel();
    expect(wrapper.text()).toContain("Reviewers");
    expect(wrapper.text()).toContain("Assignees");
    expect(wrapper.text()).toContain("reviewer");
    expect(wrapper.text()).toContain("assignee");
    expect(wrapper.text()).toContain("0.6.0");

    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await flushPromises();
    expect(wrapper.get<HTMLInputElement>('[data-testid="metadata-title"]').element.value).toBe(
      "原始标题",
    );
    expect(wrapper.get<HTMLTextAreaElement>('[data-testid="metadata-body"]').element.value).toBe(
      "原始描述",
    );
    expect(wrapper.get('[data-testid="metadata-reviewers"] .app-multi-select-value').text()).toBe(
      "reviewer",
    );
  });

  it("编辑时可以分别搜索 Reviewers、Assignees 和 Labels", async () => {
    const wrapper = mountPanel();
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await flushPromises();

    for (const [testId, query, expected] of [
      ["metadata-reviewers", "alice", "alice"],
      ["metadata-assignees", "car", "carol"],
      ["metadata-labels", "feat", "feature"],
    ] as const) {
      await wrapper.get(`[data-testid="${testId}"] [role="combobox"]`).trigger("click");
      await wrapper.get(`[data-testid="${testId}"] input[type="search"]`).setValue(query);
      expect(
        wrapper.get(`[data-testid="${testId}"] .multi-select-option[data-value="${expected}"]`),
      ).toBeTruthy();
      await wrapper.get(`[data-testid="${testId}"] [role="combobox"]`).trigger("click");
    }
  });

  it("已挂载标签会使用仓库标签的颜色和描述", async () => {
    const wrapper = mountPanel();
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await flushPromises();
    await wrapper.get('[data-testid="metadata-labels"] [role="combobox"]').trigger("click");

    const bugOption = wrapper.get(
      '[data-testid="metadata-labels"] .multi-select-option[data-value="bug"]',
    );
    expect(bugOption.find(".multi-select-swatch").attributes("style")).toContain(
      "background-color: rgb(215, 58, 74)",
    );
    expect(bugOption.text()).toContain("需要修复的问题");
  });

  it("候选项加载失败后可以在编辑态就地重试", async () => {
    vi.mocked(prParticipantSuggestions)
      .mockRejectedValueOnce(new Error("成员加载失败"))
      .mockResolvedValueOnce([
        { id: 10, login: "retry-member", name: "Retry Member", avatar_url: "" },
      ]);

    const wrapper = mountPanel();
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await flushPromises();

    expect(wrapper.get(".options-error").text()).toContain("成员加载失败");
    await wrapper.get('[data-testid="metadata-options-retry"]').trigger("click");
    await flushPromises();

    expect(wrapper.find(".options-error").exists()).toBe(false);
    expect(prParticipantSuggestions).toHaveBeenCalledTimes(2);
    await wrapper.get('[data-testid="metadata-reviewers"] [role="combobox"]').trigger("click");
    expect(wrapper.text()).toContain("retry-member");
  });

  it("仓库上下文变化后忽略旧候选项的迟到响应", async () => {
    let resolveOldParticipants: (value: User[]) => void = () => undefined;
    let resolveOldLabels: (value: PrLabel[]) => void = () => undefined;
    vi.mocked(prParticipantSuggestions)
      .mockReturnValueOnce(
        new Promise<User[]>((resolve) => {
          resolveOldParticipants = resolve;
        }),
      )
      .mockResolvedValueOnce([{ id: 8, login: "new-member", name: "New Member", avatar_url: "" }]);
    vi.mocked(prLabels)
      .mockReturnValueOnce(
        new Promise<PrLabel[]>((resolve) => {
          resolveOldLabels = resolve;
        }),
      )
      .mockResolvedValueOnce([{ name: "new-label", color: null, description: null }]);

    const wrapper = mountPanel();
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await wrapper.setProps({
      detail: {
        ...detail,
        summary: { ...detail.summary, number: 43, labels: [] },
        reviewers: [],
        assignees: [],
      },
      owner: "other",
    });
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await flushPromises();

    resolveOldParticipants([{ id: 9, login: "old-member", name: "Old Member", avatar_url: "" }]);
    resolveOldLabels([{ name: "old-label", color: null, description: null }]);
    await flushPromises();

    await wrapper.get('[data-testid="metadata-reviewers"] [role="combobox"]').trigger("click");
    expect(wrapper.text()).toContain("new-member");
    expect(wrapper.text()).not.toContain("old-member");
    await wrapper.get('[data-testid="metadata-labels"] [role="combobox"]').trigger("click");
    expect(wrapper.text()).toContain("new-label");
    expect(wrapper.text()).not.toContain("old-label");
  });

  it("解析并去重列表，同时携带 expected_updated_at", async () => {
    const wrapper = mountPanel();
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await flushPromises();
    await wrapper.get('[data-testid="metadata-title"]').setValue("  新标题  ");
    await wrapper.get('[data-testid="metadata-body"]').setValue("新描述");
    await wrapper.get('[data-testid="metadata-draft"]').setValue(true);
    await wrapper.get('[data-testid="metadata-reviewers"] [role="combobox"]').trigger("click");
    await wrapper
      .get('[data-testid="metadata-reviewers"] .multi-select-option[data-value="reviewer"]')
      .trigger("click");
    await wrapper
      .get('[data-testid="metadata-reviewers"] .multi-select-option[data-value="alice"]')
      .trigger("click");
    await wrapper
      .get('[data-testid="metadata-reviewers"] .multi-select-option[data-value="Bob"]')
      .trigger("click");
    await wrapper.get('[data-testid="metadata-assignees"] [role="combobox"]').trigger("click");
    await wrapper
      .get('[data-testid="metadata-assignees"] .multi-select-option[data-value="assignee"]')
      .trigger("click");
    await wrapper
      .get('[data-testid="metadata-assignees"] .multi-select-option[data-value="carol"]')
      .trigger("click");
    await wrapper.get('[data-testid="metadata-labels"] [role="combobox"]').trigger("click");
    await wrapper
      .get('[data-testid="metadata-labels"] .multi-select-option[data-value="review"]')
      .trigger("click");
    await wrapper
      .get('[data-testid="metadata-labels"] .multi-select-option[data-value="feature"]')
      .trigger("click");
    await wrapper.get('[data-testid="metadata-milestone"]').setValue("  0.7.0  ");
    await wrapper.get("form").trigger("submit");

    const update = wrapper.emitted("save")?.[0]?.[0] as PrMetadataUpdate;
    expect(update).toEqual({
      title: "新标题",
      body: "新描述",
      draft: true,
      reviewers: ["alice", "Bob"],
      assignees: ["carol"],
      labels: ["bug", "feature"],
      milestone: "0.7.0",
      expected_updated_at: "2026-07-18T01:00:00Z",
    });
  });

  it("标题为空时阻止提交并展示校验错误", async () => {
    const wrapper = mountPanel();
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await flushPromises();
    await wrapper.get('[data-testid="metadata-title"]').setValue("   ");
    await wrapper.get("form").trigger("submit");

    expect(wrapper.emitted("save")).toBeUndefined();
    expect(wrapper.get('[role="alert"]').text()).toContain("PR 标题不能为空");
  });

  it("按平台使用参与者名称：Gitee 显示评审者和测试者", async () => {
    const wrapper = mountPanel({
      capabilities: capabilities({
        platform: "gitee",
        supports_pr_draft_toggle: false,
        supports_pr_assignee_management: true,
      }),
    });
    expect(wrapper.text()).toContain("评审者");
    expect(wrapper.text()).toContain("测试者");
    expect(wrapper.text()).not.toContain("Reviewers");
    expect(wrapper.text()).not.toContain("Assignees");
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-testid="metadata-draft"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="metadata-assignees"]').exists()).toBe(true);
  });

  it("权限为 false 时禁用对应字段，保存状态禁用整个表单", async () => {
    const restricted = {
      ...detail,
      metadata_permissions: {
        ...detail.metadata_permissions,
        can_manage_labels: false,
      },
    };
    const wrapper = mountPanel({ detail: restricted });
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await wrapper.setProps({ saving: true });
    expect(wrapper.get('[data-testid="metadata-labels"] .app-multi-select').classes()).toContain(
      "disabled",
    );
    expect(wrapper.get('button[type="submit"]').attributes("disabled")).toBeDefined();
  });

  it("取消编辑恢复原始值", async () => {
    const wrapper = mountPanel();
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await wrapper.get('[data-testid="metadata-title"]').setValue("临时标题");
    await wrapper.get('button[type="button"]').trigger("click");
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    expect(wrapper.get<HTMLInputElement>('[data-testid="metadata-title"]').element.value).toBe(
      "原始标题",
    );
  });
});
