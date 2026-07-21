import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { prDependencies } from "@/api";
import type { PrDependencyGraph } from "@/types";
import PrDependenciesPanel from "../PrDependenciesPanel.vue";

vi.mock("@/api", () => ({
  prDependencies: vi.fn(),
}));

const graph: PrDependencyGraph = {
  current_number: 2,
  nodes: [
    {
      number: 1,
      title: "基础重构",
      state: "closed",
      source_branch: "feature-a",
      target_branch: "main",
    },
    {
      number: 2,
      title: "功能实现",
      state: "open",
      source_branch: "feature-b",
      target_branch: "feature-a",
    },
    {
      number: 3,
      title: "后续补强",
      state: "open",
      source_branch: "feature-c",
      target_branch: "feature-b",
    },
  ],
  edges: [
    { parent_number: 1, child_number: 2 },
    { parent_number: 2, child_number: 3 },
  ],
  suggested_merge_order: [1, 2, 3],
  blocking_parent_numbers: [1],
  has_cycle: false,
  truncated: false,
};

function mountPanel(overrides: Record<string, unknown> = {}) {
  return mount(PrDependenciesPanel, {
    props: {
      platform: "github",
      owner: "team",
      repo: "repo",
      prNumber: 2,
      revision: "updated-1",
      ...overrides,
    },
    global: {
      stubs: {
        RouterLink: { template: "<a><slot /></a>" },
      },
    },
  });
}

describe("PrDependenciesPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(prDependencies).mockResolvedValue(graph);
  });

  it("展示未合并父项阻塞和建议合并顺序", async () => {
    const wrapper = mountPanel();
    await flushPromises();

    expect(prDependencies).toHaveBeenCalledWith("github", "team", "repo", 2);
    expect(wrapper.text()).toContain("仍依赖 1 个尚未合并的父项");
    expect(wrapper.findAll(".dependency-flow li")).toHaveLength(3);
    expect(wrapper.findAll(".node-title").map((node) => node.text())).toEqual([
      "#1 基础重构",
      "#2 功能实现",
      "#3 后续补强",
    ]);
    expect(wrapper.find(".dependency-flow li.current").text()).toContain("当前");
    expect(wrapper.find(".dependency-flow li.blocker").text()).toContain("已关闭，依赖未合并");
  });

  it("当前项已关闭时只展示历史关系，不再提示阻塞或建议合并", async () => {
    vi.mocked(prDependencies).mockResolvedValueOnce({
      ...graph,
      nodes: graph.nodes.map((node) =>
        node.number === graph.current_number ? { ...node, state: "closed" } : node,
      ),
      blocking_parent_numbers: [],
    });
    const wrapper = mountPanel({ platform: "gitlab" });
    await flushPromises();

    expect(wrapper.text()).toContain("当前 MR 已结束，仅展示历史依赖关系");
    expect(wrapper.get(".order-heading h4").text()).toBe("历史依赖关系");
    expect(wrapper.text()).not.toContain("建议合并顺序");
    expect(wrapper.text()).not.toContain("阻塞当前项");
  });

  it("没有相连节点时展示明确空状态", async () => {
    vi.mocked(prDependencies).mockResolvedValueOnce({
      ...graph,
      nodes: [graph.nodes[1]],
      edges: [],
      suggested_merge_order: [2],
      blocking_parent_numbers: [],
    });
    const wrapper = mountPanel({ platform: "gitlab" });
    await flushPromises();

    expect(wrapper.text()).toContain("未发现与当前 MR 相连的分支依赖");
  });

  it("候选遍历触顶时提示结果可能不完整", async () => {
    vi.mocked(prDependencies).mockResolvedValueOnce({ ...graph, truncated: true });
    const wrapper = mountPanel();
    await flushPromises();

    expect(wrapper.text()).toContain("结果可能不完整");
  });

  it("失败后允许重新加载", async () => {
    vi.mocked(prDependencies)
      .mockRejectedValueOnce(new Error("平台请求失败"))
      .mockResolvedValueOnce(graph);
    const wrapper = mountPanel();
    await flushPromises();

    expect(wrapper.get(".dependency-error").text()).toContain("平台请求失败");
    await wrapper.get(".dependency-error button").trigger("click");
    await flushPromises();

    expect(prDependencies).toHaveBeenCalledTimes(2);
    expect(wrapper.find(".dependency-error").exists()).toBe(false);
    expect(wrapper.findAll(".dependency-flow li")).toHaveLength(3);
  });

  it("已有依赖图刷新时保留内容并展示轻量忙碌状态", async () => {
    const wrapper = mountPanel();
    await flushPromises();
    let resolveRefresh!: (value: PrDependencyGraph) => void;
    vi.mocked(prDependencies).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveRefresh = resolve;
        }),
    );

    await wrapper.get(".refresh-button").trigger("click");

    expect(wrapper.get(".dependency-panel").attributes("aria-busy")).toBe("true");
    expect(wrapper.get(".refresh-button").classes()).toContain("loading");
    expect(wrapper.text()).toContain("刷新中");
    expect(wrapper.findAll(".dependency-flow li")).toHaveLength(3);

    resolveRefresh(graph);
    await flushPromises();
    expect(wrapper.get(".dependency-panel").attributes("aria-busy")).toBe("false");
    expect(wrapper.find(".refresh-status").exists()).toBe(false);
  });

  it("切换 PR 后忽略旧依赖请求", async () => {
    let resolveOld!: (value: PrDependencyGraph) => void;
    vi.mocked(prDependencies)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveOld = resolve;
          }),
      )
      .mockResolvedValueOnce({
        ...graph,
        current_number: 4,
        nodes: [
          {
            number: 4,
            title: "新 PR",
            state: "open",
            source_branch: "new-feature",
            target_branch: "main",
          },
        ],
        edges: [],
        suggested_merge_order: [4],
        blocking_parent_numbers: [],
      });
    const wrapper = mountPanel();
    await wrapper.setProps({ prNumber: 4, revision: "updated-2" });
    await flushPromises();
    resolveOld(graph);
    await flushPromises();

    expect(wrapper.text()).toContain("未发现与当前 PR 相连的分支依赖");
    expect(wrapper.text()).not.toContain("基础重构");
  });
});
