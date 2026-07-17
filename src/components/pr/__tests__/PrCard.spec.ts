import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import PrCard from "@/components/pr/PrCard.vue";
import type { PrSummary, PrStatusSummary } from "@/types";

const readyStatus: PrStatusSummary = {
  status: "ready",
  draft: false,
  has_conflicts: false,
  checks_status: "ready",
  approvals_status: "ready",
  blocking_reasons: [],
};

function pr(overrides: Partial<PrSummary> = {}): PrSummary {
  return {
    number: 42,
    title: "Improve PR list",
    author: { id: 1, login: "dev", name: "Dev", avatar_url: "" },
    state: "open",
    created_at: "2026-07-16T00:00:00Z",
    updated_at: "2026-07-17T00:00:00Z",
    labels: ["frontend"],
    status: readyStatus,
    ...overrides,
  };
}

describe("PrCard", () => {
  it("Open PR 展示总体、审批和 CI/测试状态", () => {
    const wrapper = mount(PrCard, { props: { pr: pr() } });

    expect(wrapper.text()).toContain("可合并");
    expect(wrapper.text()).toContain("审批已满足");
    expect(wrapper.text()).toContain("CI/测试通过");
    expect(wrapper.get(".status-summary").attributes("title")).toBe("可合并");
  });

  it("被阻塞的 Open PR 展示 Draft、冲突和具体阻塞原因", () => {
    const wrapper = mount(PrCard, {
      props: {
        pr: pr({
          status: {
            status: "blocked",
            draft: true,
            has_conflicts: true,
            checks_status: "blocked",
            approvals_status: "blocked",
            blocking_reasons: [
              { code: "changes_requested", message: "已有评审请求修改" },
              { code: "conflicts", message: "源分支存在合并冲突" },
            ],
          },
        }),
      },
    });

    expect(wrapper.text()).toContain("被阻塞");
    expect(wrapper.text()).toContain("审批未完成");
    expect(wrapper.text()).toContain("CI/测试失败");
    expect(wrapper.text()).toContain("Draft");
    expect(wrapper.text()).toContain("存在冲突");
    expect(wrapper.get(".status-summary").attributes("title")).toBe(
      "已有评审请求修改；源分支存在合并冲突",
    );
  });

  it("未知状态不会显示为可合并", () => {
    const wrapper = mount(PrCard, {
      props: {
        pr: pr({
          status: {
            ...readyStatus,
            status: "unknown",
            checks_status: "unknown",
            approvals_status: "unknown",
          },
        }),
      },
    });

    expect(wrapper.text()).toContain("状态未知");
    expect(wrapper.text()).toContain("审批未知");
    expect(wrapper.text()).toContain("CI/测试未知");
    expect(wrapper.text()).not.toContain("可合并");
  });

  it.each(["closed", "merged"] as const)("%s PR 不展示实时审批和 CI 状态", (state) => {
    const wrapper = mount(PrCard, { props: { pr: pr({ state, status: null }) } });

    expect(wrapper.find(".status-summary").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("审批");
    expect(wrapper.text()).not.toContain("CI/测试");
  });
});
