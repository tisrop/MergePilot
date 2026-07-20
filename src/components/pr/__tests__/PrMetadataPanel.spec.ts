import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import PrMetadataPanel from "../PrMetadataPanel.vue";
import type { PlatformCapabilities, PrDetail, PrMetadataUpdate, User } from "@/types";

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
      capabilities: capabilities(),
      saving: false,
      ...props,
    },
  });
}

describe("PrMetadataPanel", () => {
  it("展示元数据并按当前详情初始化编辑表单", async () => {
    const wrapper = mountPanel();
    expect(wrapper.text()).toContain("Reviewers");
    expect(wrapper.text()).toContain("Assignees");
    expect(wrapper.text()).toContain("reviewer");
    expect(wrapper.text()).toContain("assignee");
    expect(wrapper.text()).toContain("0.6.0");

    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    expect(wrapper.get<HTMLInputElement>('[data-testid="metadata-title"]').element.value).toBe(
      "原始标题",
    );
    expect(wrapper.get<HTMLTextAreaElement>('[data-testid="metadata-body"]').element.value).toBe(
      "原始描述",
    );
    expect(wrapper.get<HTMLInputElement>('[data-testid="metadata-reviewers"]').element.value).toBe(
      "reviewer",
    );
  });

  it("解析并去重列表，同时携带 expected_updated_at", async () => {
    const wrapper = mountPanel();
    await wrapper.get('[data-testid="edit-pr-metadata"]').trigger("click");
    await wrapper.get('[data-testid="metadata-title"]').setValue("  新标题  ");
    await wrapper.get('[data-testid="metadata-body"]').setValue("新描述");
    await wrapper.get('[data-testid="metadata-draft"]').setValue(true);
    await wrapper.get('[data-testid="metadata-reviewers"]').setValue("alice, Bob, alice");
    await wrapper.get('[data-testid="metadata-assignees"]').setValue("carol, carol");
    await wrapper.get('[data-testid="metadata-labels"]').setValue("bug, feature, bug");
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
    expect(wrapper.get('[data-testid="metadata-labels"]').attributes("disabled")).toBeDefined();
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
