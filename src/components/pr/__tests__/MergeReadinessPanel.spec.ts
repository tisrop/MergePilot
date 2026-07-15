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
) {
  return mount(MergeReadinessPanel, {
    props: { readiness, loading: false, error },
  });
}

describe("MergeReadinessPanel", () => {
  it("显示已具备合并条件和检查版本", () => {
    const wrapper = mountPanel();

    expect(wrapper.text()).toContain("可合并");
    expect(wrapper.text()).toContain("01234567");
    expect(wrapper.text()).toContain("测试 / CI");
    expect(wrapper.text()).toContain("合并权限");
    expect(wrapper.text()).not.toContain("状态未知");
  });

  it("没有合并权限时显示阻塞原因", () => {
    const wrapper = mountPanel({
      ...baseReadiness,
      status: "blocked",
      has_merge_permission: false,
      blocking_reasons: [{ code: "no_merge_permission", message: "当前账号没有该仓库的合并权限" }],
    });

    expect(wrapper.text()).toContain("合并权限无");
    expect(wrapper.text()).toContain("当前账号没有该仓库的合并权限");
  });

  it("阻塞时显示结构化原因", () => {
    const wrapper = mountPanel({
      ...baseReadiness,
      status: "blocked",
      blocking_reasons: [{ code: "conflicts", message: "源分支存在合并冲突" }],
    });

    expect(wrapper.text()).toContain("已阻断");
    expect(wrapper.text()).toContain("源分支存在合并冲突");
  });

  it("未知状态不会显示可合并", () => {
    const wrapper = mountPanel({ ...baseReadiness, status: "unknown", checks_status: "unknown" });

    expect(wrapper.text()).toContain("状态未知");
    expect(wrapper.text()).not.toContain("服务端已确认当前版本满足可合并条件");
  });

  it("错误状态支持重试", async () => {
    const wrapper = mountPanel(null, "读取失败");
    await wrapper.get("button").trigger("click");

    expect(wrapper.text()).toContain("读取失败");
    expect(wrapper.emitted("retry")).toHaveLength(1);
  });
});
