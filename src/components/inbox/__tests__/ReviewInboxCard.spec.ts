import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import ReviewInboxCard from "@/components/inbox/ReviewInboxCard.vue";
import type { ReviewInboxItem } from "@/types";

function inboxItem(): ReviewInboxItem {
  return {
    platform: "github",
    owner: "team",
    repo: "project",
    repository_full_name: "team/project",
    categories: ["review_requested"],
    relationships: ["reviewer", "assignee"],
    status: {
      status: "blocked",
      draft: true,
      has_conflicts: true,
      checks_status: "blocked",
      approvals_status: "blocked",
      blocking_reasons: [
        { code: "checks_failed", message: "CI 检查未通过" },
        { code: "changes_requested", message: "已有评审请求修改" },
      ],
    },
    summary: {
      number: 42,
      title: "Improve inbox",
      author: { id: 1, login: "dev", name: "Dev", avatar_url: "" },
      state: "open",
      created_at: "2026-07-16T00:00:00Z",
      updated_at: "2026-07-17T00:00:00Z",
      labels: [],
    },
  };
}

describe("ReviewInboxCard", () => {
  it("展示具体关系、审批、CI、Draft、冲突和阻塞原因", () => {
    const wrapper = mount(ReviewInboxCard, { props: { item: inboxItem() } });

    expect(wrapper.text()).toContain("评审人");
    expect(wrapper.text()).toContain("负责人");
    expect(wrapper.text()).toContain("被阻塞");
    expect(wrapper.text()).toContain("审批未完成");
    expect(wrapper.text()).toContain("CI/测试失败");
    expect(wrapper.text()).toContain("Draft");
    expect(wrapper.text()).toContain("存在冲突");
    expect(wrapper.get(".status-summary").attributes("title")).toBe(
      "CI 检查未通过；已有评审请求修改",
    );
    expect(wrapper.get(".status-summary").attributes("aria-label")).toContain("CI 检查未通过");
  });
});
