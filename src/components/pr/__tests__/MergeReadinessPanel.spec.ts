import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import MergeReadinessPanel from "../MergeReadinessPanel.vue";
import type { PrMergeReadiness } from "@/types";

const baseReadiness: PrMergeReadiness = {
  status: "ready",
  head_sha: "0123456789abcdef",
  mergeable: true,
  draft: false,
  has_conflicts: false,
  checks_status: "ready",
  approvals_status: "ready",
  approvals_required: null,
  approvals_received: null,
  has_merge_permission: true,
  branch_behind: false,
  blocking_reasons: [],
};

function mountPanel(
  readiness: PrMergeReadiness | null = baseReadiness,
  error: string | null = null,
  loading = false,
) {
  return mount(MergeReadinessPanel, {
    props: { readiness, loading, error },
  });
}

describe("MergeReadinessPanel", () => {
  it("只显示紧凑状态并在悬浮详情中显示结果", () => {
    const wrapper = mountPanel();

    expect(wrapper.get(".readiness-status").text()).toContain("可合并");
    expect(wrapper.text()).not.toContain("合并就绪检查");
    expect(wrapper.get(".readiness-tooltip").text()).toContain("所有合并条件均已满足");
    expect(wrapper.find(".condition-list").exists()).toBe(false);
  });

  it("阻断时在悬浮详情中显示平台返回的具体原因", () => {
    const wrapper = mountPanel({
      ...baseReadiness,
      status: "blocked",
      has_merge_permission: false,
      blocking_reasons: [{ code: "no_merge_permission", message: "当前账号没有该仓库的合并权限" }],
    });

    expect(wrapper.get(".readiness-status").text()).toContain("已阻断");
    expect(wrapper.get(".readiness-tooltip").text()).toContain("当前账号没有该仓库的合并权限");
    expect(wrapper.find(".condition-list").exists()).toBe(false);
  });

  it("未知状态不会显示可合并", () => {
    const wrapper = mountPanel({ ...baseReadiness, status: "unknown", checks_status: "unknown" });

    expect(wrapper.get(".readiness-status").text()).toContain("状态未知");
    expect(wrapper.get(".readiness-status").text()).not.toContain("可合并");
    expect(wrapper.get(".readiness-tooltip").text()).not.toContain("状态未知");
    expect(wrapper.get(".readiness-tooltip").text()).toContain("仍可尝试合并");
    expect(wrapper.get(".readiness-tooltip").text()).toContain("最终校验");
  });

  it("点击刷新图标重新检查合并状态", async () => {
    const wrapper = mountPanel();
    await wrapper.get(".refresh-button").trigger("click");

    expect(wrapper.emitted("retry")).toHaveLength(1);
    expect(wrapper.get(".refresh-button").attributes("aria-label")).toBe("刷新合并状态");
  });

  it("刷新期间禁用刷新图标并保留已有状态", () => {
    const wrapper = mountPanel(baseReadiness, null, true);

    expect(wrapper.get(".refresh-button").attributes()).toHaveProperty("disabled");
    expect(wrapper.get(".refresh-button").attributes("aria-label")).toBe("正在刷新合并状态");
    expect(wrapper.get(".readiness-status").text()).toContain("可合并");
  });

  it("首次读取失败时通过悬浮详情显示错误并允许重试", async () => {
    const wrapper = mountPanel(null, "读取失败");
    await wrapper.get(".refresh-button").trigger("click");

    expect(wrapper.get(".readiness-tooltip").text()).toContain("读取失败");
    expect(wrapper.emitted("retry")).toHaveLength(1);
  });
});
