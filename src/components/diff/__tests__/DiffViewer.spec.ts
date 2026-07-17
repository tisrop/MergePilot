import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DiffViewer from "@/components/diff/DiffViewer.vue";
import { useUiSettingsStore } from "@/stores/useUiSettingsStore";
import type { DiffResult, Platform, PrFileContent } from "@/types";

const { prFileContentMock } = vi.hoisted(() => ({
  prFileContentMock: vi.fn(),
}));

vi.mock("@/api", () => ({
  prFileContent: prFileContentMock,
}));

const storage = new Map<string, string>();

vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

const diff: DiffResult = {
  diff: `diff --git a/src/components/App.ts b/src/components/App.ts
index 1111111..2222222 100644
--- a/src/components/App.ts
+++ b/src/components/App.ts
@@ -1 +1 @@
-export const state = "old";
+export const state = "new";
diff --git a/tests/App.spec.ts b/tests/App.spec.ts
index 3333333..4444444 100644
--- a/tests/App.spec.ts
+++ b/tests/App.spec.ts
@@ -1 +1,2 @@
 describe("App", () => {});
+it("works", () => {});`,
  files: [
    {
      filename: "src/components/App.ts",
      status: "modified",
      patch: '@@ -1 +1 @@\n-export const state = "old";\n+export const state = "new";',
      additions: 1,
      deletions: 1,
    },
    {
      filename: "tests/App.spec.ts",
      status: "added",
      patch: '@@ -1 +1,2 @@\n describe("App", () => {});\n+it("works", () => {});',
      additions: 1,
      deletions: 0,
    },
  ],
  patch_schema_version: 1,
  patches: [],
};

interface ContextProps {
  platform?: Platform;
  owner?: string;
  repo?: string;
  baseSha?: string;
  headSha?: string;
}

async function mountViewer(value = diff, extraProps: ContextProps = {}) {
  const wrapper = mount(DiffViewer, { props: { diff: value, ...extraProps } });
  await flushPromises();
  return wrapper;
}

const standardizedDiff: DiffResult = {
  diff: "",
  files: [
    {
      filename: "src/components/App.ts",
      status: "modified",
      patch: "",
      additions: 1,
      deletions: 1,
    },
    {
      filename: "tests/App.spec.ts",
      status: "added",
      patch: "",
      additions: 1,
      deletions: 0,
    },
  ],
  patch_schema_version: 1,
  patches: [
    {
      filename: "src/components/App.ts",
      old_path: "src/components/App.ts",
      new_path: "src/components/App.ts",
      status: "modified",
      additions: 1,
      deletions: 1,
      content_kind: "text",
      patch: "",
      message: null,
      hunks: [
        {
          header: "@@ -1,2 +1,2 @@",
          old_start: 1,
          old_count: 2,
          new_start: 1,
          new_count: 2,
          section_header: null,
          lines: [
            { kind: "context", content: "const state = true;", old_line: 1, new_line: 1 },
            { kind: "deletion", content: 'const value = "<old>";', old_line: 2, new_line: null },
            { kind: "addition", content: 'const value = "<new>";', old_line: null, new_line: 2 },
          ],
        },
      ],
    },
    {
      filename: "tests/App.spec.ts",
      old_path: null,
      new_path: "tests/App.spec.ts",
      status: "added",
      additions: 1,
      deletions: 0,
      content_kind: "text",
      patch: "",
      message: null,
      hunks: [
        {
          header: "@@ -0,0 +1 @@",
          old_start: 0,
          old_count: 0,
          new_start: 1,
          new_count: 1,
          section_header: null,
          lines: [{ kind: "addition", content: 'it("works");', old_line: null, new_line: 1 }],
        },
      ],
    },
  ],
};

const contextDiff: DiffResult = {
  diff: "",
  files: [
    {
      filename: "src/context.ts",
      status: "modified",
      patch: "",
      additions: 0,
      deletions: 0,
    },
  ],
  patch_schema_version: 1,
  patches: [
    {
      filename: "src/context.ts",
      old_path: "src/context.old.ts",
      new_path: "src/context.ts",
      status: "renamed",
      additions: 0,
      deletions: 0,
      content_kind: "text",
      patch: "",
      message: null,
      hunks: [
        {
          header: "@@ -3 +3 @@",
          old_start: 3,
          old_count: 1,
          new_start: 3,
          new_count: 1,
          section_header: null,
          lines: [{ kind: "context", content: "unchanged 3", old_line: 3, new_line: 3 }],
        },
        {
          header: "@@ -7 +7 @@",
          old_start: 7,
          old_count: 1,
          new_start: 7,
          new_count: 1,
          section_header: null,
          lines: [{ kind: "context", content: "unchanged 7", old_line: 7, new_line: 7 }],
        },
      ],
    },
  ],
};

const contextProps: Required<ContextProps> = {
  platform: "github",
  owner: "octo",
  repo: "demo",
  baseSha: "base-sha",
  headSha: "head-sha",
};

function fileContent(path: string, revision: string, content: string): PrFileContent {
  return { path, revision, content, truncated: false, binary: false };
}

function mockContextFiles(options?: { truncated?: boolean; binary?: boolean }): void {
  prFileContentMock.mockImplementation(
    async (_platform: Platform, _owner: string, _repo: string, path: string, revision: string) => ({
      ...fileContent(
        path,
        revision,
        [
          revision === "base-sha" ? "base 1" : "<script>alert(1)</script>",
          `${revision} 2`,
          "unchanged 3",
          `${revision} 4`,
          `${revision} 5`,
          `${revision} 6`,
          "unchanged 7",
          `${revision} 8`,
        ].join("\n"),
      ),
      truncated: options?.truncated ?? false,
      binary: options?.binary ?? false,
    }),
  );
}

describe("DiffViewer 受控标准 patch", () => {
  beforeEach(() => {
    storage.clear();
    prFileContentMock.mockReset();
    setActivePinia(createPinia());
  });

  it("使用标准 patch 受控渲染 hunk、双侧行号和纯文本代码", async () => {
    const wrapper = await mountViewer(standardizedDiff);

    expect(wrapper.find(".legacy-diff").exists()).toBe(false);
    expect(wrapper.get(".controlled-file-wrapper").attributes("data-file-path")).toBe(
      "src/components/App.ts",
    );
    expect(wrapper.findAll(".controlled-hunk-header")).toHaveLength(2);
    expect(wrapper.findAll(".controlled-hunk-header-text")).toHaveLength(1);
    expect(wrapper.get(".controlled-side-left .controlled-hunk-header").text()).toBe(
      "@@ -1,2 +1,2 @@",
    );
    expect(wrapper.get(".controlled-side-right .controlled-hunk-header").text()).toBe("");
    expect(wrapper.findAll(".controlled-line-deletion")).toHaveLength(1);
    expect(wrapper.findAll(".controlled-line-addition")).toHaveLength(1);
    expect(wrapper.get(".controlled-side-left").text()).toContain("<old>");
    expect(wrapper.get(".controlled-side-right").text()).toContain("<new>");
    expect(
      wrapper.get(".controlled-side-left .controlled-line-deletion").attributes("data-line"),
    ).toBe("2");
    expect(
      wrapper.get(".controlled-side-right .controlled-line-addition").attributes("data-line"),
    ).toBe("2");
    expect(wrapper.find(".controlled-side-left script").exists()).toBe(false);
  });

  it("切换文件时只渲染对应的标准 patch", async () => {
    const wrapper = await mountViewer(standardizedDiff);

    await wrapper.get('[data-file-path="tests/App.spec.ts"]').trigger("click");
    await flushPromises();

    expect(wrapper.get(".controlled-file-wrapper").attributes("data-file-path")).toBe(
      "tests/App.spec.ts",
    );
    expect(wrapper.get(".controlled-side-right").text()).toContain('it("works");');
    expect(wrapper.findAll(".controlled-line-addition")).toHaveLength(1);
  });

  it("对二进制或不可用 patch 显示稳定提示而不是空白", async () => {
    const binaryDiff: DiffResult = {
      ...standardizedDiff,
      files: [standardizedDiff.files[0]],
      patches: [
        {
          ...standardizedDiff.patches[0],
          content_kind: "binary",
          hunks: [],
          message: "二进制文件不提供文本 Diff",
        },
      ],
    };

    const wrapper = await mountViewer(binaryDiff);

    expect(wrapper.get(".controlled-file-message").text()).toContain("二进制文件");
    expect(wrapper.find(".diff-empty").exists()).toBe(false);
  });

  it("纯重命名没有文本 hunk 时不显示上下文展开操作", async () => {
    const metadataOnlyRename: DiffResult = {
      diff: "",
      files: [
        {
          filename: "src/new-name.ts",
          status: "renamed",
          patch: "",
          additions: 0,
          deletions: 0,
        },
      ],
      patch_schema_version: 1,
      patches: [
        {
          filename: "src/new-name.ts",
          old_path: "src/old-name.ts",
          new_path: "src/new-name.ts",
          status: "renamed",
          additions: 0,
          deletions: 0,
          content_kind: "metadata_only",
          patch: "",
          message: "该文件仅包含重命名、权限或其他元数据变更",
          hunks: [],
        },
      ],
    };

    const wrapper = await mountViewer(metadataOnlyRename, contextProps);

    expect(wrapper.get(".controlled-file-message").text()).toContain("仅包含重命名");
    expect(wrapper.find(".context-toolbar-button").exists()).toBe(false);
    expect(wrapper.find(".context-gap-button").exists()).toBe(false);
    expect(prFileContentMock).not.toHaveBeenCalled();
  });

  it("从行号槽按方向展开单个上下文，并按 base/head 路径请求文件内容", async () => {
    mockContextFiles();
    const wrapper = await mountViewer(contextDiff, contextProps);
    const buttons = wrapper.findAll(".context-gap-button");
    const leftHunkHeaders = wrapper.findAll(".controlled-side-left .controlled-hunk-header");

    expect(leftHunkHeaders).toHaveLength(2);
    expect(leftHunkHeaders[0].findAll(".context-gap-button")).toHaveLength(1);
    expect(leftHunkHeaders[1].findAll(".context-gap-button")).toHaveLength(2);
    expect(
      wrapper.findAll(".controlled-side-right .controlled-hunk-header .context-gap-placeholder"),
    ).toHaveLength(2);
    expect(buttons).toHaveLength(3);
    expect(buttons[0].attributes("aria-label")).toBe("展开上方未变更上下文（20 行）");
    expect(buttons[0].text()).toBe("↑");
    expect(buttons[1].attributes("aria-label")).toBe("向上展开未变更上下文（20 行）");
    expect(buttons[1].text()).toBe("↑");
    expect(buttons[2].attributes("aria-label")).toBe("向下展开未变更上下文（20 行）");
    expect(buttons[2].text()).toBe("↓");

    await buttons[0].trigger("click");
    await flushPromises();

    expect(prFileContentMock).toHaveBeenCalledTimes(2);
    expect(prFileContentMock).toHaveBeenCalledWith(
      "github",
      "octo",
      "demo",
      "src/context.old.ts",
      "base-sha",
    );
    expect(prFileContentMock).toHaveBeenCalledWith(
      "github",
      "octo",
      "demo",
      "src/context.ts",
      "head-sha",
    );
    expect(wrapper.findAll(".controlled-context-line")).toHaveLength(4);
    expect(wrapper.get(".controlled-side-left").text()).toContain("base 1");
    expect(wrapper.get(".controlled-side-right").text()).toContain("<script>alert(1)</script>");
    expect(wrapper.find(".controlled-side-right script").exists()).toBe(false);
    expect(wrapper.get('[aria-label="展开下方未变更上下文（20 行）"]').text()).toBe("↓");
  });

  it("每次点击只展开 20 行，直到该方向的上下文全部可见", async () => {
    const largeContextDiff: DiffResult = {
      ...contextDiff,
      patches: [
        {
          ...contextDiff.patches[0],
          hunks: [
            {
              header: "@@ -51 +51 @@",
              old_start: 51,
              old_count: 1,
              new_start: 51,
              new_count: 1,
              section_header: null,
              lines: [{ kind: "context", content: "changed 51", old_line: 51, new_line: 51 }],
            },
          ],
        },
      ],
    };
    prFileContentMock.mockImplementation(
      async (_platform: Platform, _owner: string, _repo: string, path: string, revision: string) =>
        fileContent(
          path,
          revision,
          Array.from({ length: 60 }, (_, index) => `line ${index + 1}`).join("\n"),
        ),
    );
    const wrapper = await mountViewer(largeContextDiff, contextProps);
    const topLabel = '[aria-label="展开上方未变更上下文（20 行）"]';

    await wrapper.get(topLabel).trigger("click");
    await flushPromises();
    expect(wrapper.findAll(".controlled-context-line")).toHaveLength(40);
    expect(wrapper.find(topLabel).exists()).toBe(true);

    await wrapper.get(topLabel).trigger("click");
    expect(wrapper.findAll(".controlled-context-line")).toHaveLength(80);
    expect(wrapper.find(topLabel).exists()).toBe(true);

    await wrapper.get(topLabel).trigger("click");
    expect(wrapper.findAll(".controlled-context-line")).toHaveLength(100);
    expect(wrapper.find(topLabel).exists()).toBe(false);
    expect(wrapper.get(".controlled-side-left").text()).toContain("line 1");
    expect(wrapper.get(".controlled-side-right").text()).toContain("line 50");
  });

  it("文件 patch 已覆盖全文时加载后移除无效的展开操作", async () => {
    const fullFileDiff: DiffResult = {
      ...contextDiff,
      patches: [
        {
          ...contextDiff.patches[0],
          hunks: [
            {
              header: "@@ -1,2 +1,2 @@",
              old_start: 1,
              old_count: 2,
              new_start: 1,
              new_count: 2,
              section_header: null,
              lines: [
                { kind: "context", content: "line 1", old_line: 1, new_line: 1 },
                { kind: "context", content: "line 2", old_line: 2, new_line: 2 },
              ],
            },
          ],
        },
      ],
    };
    prFileContentMock.mockImplementation(
      async (_platform: Platform, _owner: string, _repo: string, path: string, revision: string) =>
        fileContent(path, revision, "line 1\nline 2"),
    );
    const wrapper = await mountViewer(fullFileDiff, contextProps);

    await wrapper.get(".context-toolbar-button").trigger("click");
    await flushPromises();

    expect(wrapper.find(".context-toolbar-button").exists()).toBe(false);
    expect(wrapper.findAll(".controlled-context-line")).toHaveLength(0);
    expect(wrapper.findAll(".controlled-hunk")).toHaveLength(2);
  });

  it("工具栏可以展开和收起全部上下文，且不会移除原始 hunk", async () => {
    mockContextFiles();
    const wrapper = await mountViewer(contextDiff, contextProps);

    expect(wrapper.findAll(".context-toolbar-button")).toHaveLength(1);
    expect(wrapper.get(".context-toolbar-button").text()).toBe("展开全部上下文");

    await wrapper.get(".context-toolbar-button").trigger("click");
    await flushPromises();

    expect(wrapper.findAll(".controlled-context-line")).toHaveLength(12);
    expect(wrapper.findAll(".context-gap-button")).toHaveLength(0);
    expect(wrapper.findAll(".controlled-hunk")).toHaveLength(4);
    expect(wrapper.findAll(".context-toolbar-button")).toHaveLength(1);
    expect(wrapper.get(".context-toolbar-button").text()).toBe("收起全部上下文");

    await wrapper.get(".context-toolbar-button").trigger("click");

    expect(wrapper.findAll(".controlled-context-line")).toHaveLength(0);
    expect(wrapper.findAll(".context-gap-button")).toHaveLength(4);
    expect(wrapper.findAll(".controlled-hunk")).toHaveLength(4);
    expect(wrapper.findAll(".context-toolbar-button")).toHaveLength(1);
    expect(wrapper.get(".context-toolbar-button").text()).toBe("展开全部上下文");
  });

  it.each([
    { response: { truncated: true, binary: false }, message: "文件过大" },
    { response: { truncated: false, binary: true }, message: "二进制文件" },
  ])("文件内容不可展开时保留原 Diff：$message", async ({ response, message }) => {
    mockContextFiles(response);
    const wrapper = await mountViewer(contextDiff, contextProps);

    await wrapper.get(".context-gap-button").trigger("click");
    await flushPromises();

    expect(wrapper.get(".context-load-error").text()).toContain(message);
    expect(wrapper.findAll(".context-load-error")).toHaveLength(1);
    expect(wrapper.findAll(".controlled-context-line")).toHaveLength(0);
    expect(wrapper.findAll(".controlled-hunk")).toHaveLength(4);
  });

  it("文件内容请求失败时显示一次错误且不白屏", async () => {
    prFileContentMock.mockRejectedValue(new Error("网络失败"));
    const wrapper = await mountViewer(contextDiff, contextProps);

    await wrapper.get(".context-gap-button").trigger("click");
    await flushPromises();

    expect(wrapper.get(".context-load-error").text()).toContain("网络失败");
    expect(wrapper.findAll(".context-load-error")).toHaveLength(1);
    expect(wrapper.findAll(".controlled-hunk")).toHaveLength(4);
    expect(wrapper.findAll(".context-gap-button")).toHaveLength(3);
  });

  it.each([
    {
      status: "added" as const,
      oldPath: null,
      newPath: "src/new.ts",
      baseSha: "",
      headSha: "head-sha",
      expectedPath: "src/new.ts",
      expectedRevision: "head-sha",
      contentSide: "right",
    },
    {
      status: "removed" as const,
      oldPath: "src/old.ts",
      newPath: null,
      baseSha: "base-sha",
      headSha: "",
      expectedPath: "src/old.ts",
      expectedRevision: "base-sha",
      contentSide: "left",
    },
  ])(
    "$status 文件只请求存在的一侧，且不会生成虚假的 0 行上下文",
    async ({
      status,
      oldPath,
      newPath,
      baseSha,
      headSha,
      expectedPath,
      expectedRevision,
      contentSide,
    }) => {
      const oneSidedDiff: DiffResult = {
        diff: "",
        files: [
          {
            filename: expectedPath,
            status,
            patch: "",
            additions: status === "added" ? 1 : 0,
            deletions: status === "removed" ? 1 : 0,
          },
        ],
        patch_schema_version: 1,
        patches: [
          {
            filename: expectedPath,
            old_path: oldPath,
            new_path: newPath,
            status,
            additions: status === "added" ? 1 : 0,
            deletions: status === "removed" ? 1 : 0,
            content_kind: "text",
            patch: "",
            message: null,
            hunks: [
              {
                header: status === "added" ? "@@ -0,0 +1 @@" : "@@ -1 +0,0 @@",
                old_start: status === "added" ? 0 : 1,
                old_count: status === "added" ? 0 : 1,
                new_start: status === "added" ? 1 : 0,
                new_count: status === "added" ? 1 : 0,
                section_header: null,
                lines: [
                  {
                    kind: status === "added" ? "addition" : "deletion",
                    content: "changed",
                    old_line: status === "removed" ? 1 : null,
                    new_line: status === "added" ? 1 : null,
                  },
                ],
              },
            ],
          },
        ],
      };
      prFileContentMock.mockResolvedValue(
        fileContent(expectedPath, expectedRevision, "changed\ntrailing"),
      );
      const wrapper = await mountViewer(oneSidedDiff, {
        ...contextProps,
        baseSha,
        headSha,
      });

      await wrapper.get(".context-toolbar-button").trigger("click");
      await flushPromises();

      expect(prFileContentMock).toHaveBeenCalledTimes(1);
      expect(prFileContentMock).toHaveBeenCalledWith(
        "github",
        "octo",
        "demo",
        expectedPath,
        expectedRevision,
      );
      expect(wrapper.get(`.controlled-side-${contentSide}`).text()).toContain("trailing");
      expect(wrapper.findAll('[data-line="0"]')).toHaveLength(0);
    },
  );

  it("切换 revision 后丢弃迟到的旧文件内容响应", async () => {
    const resolvers: Array<(value: PrFileContent) => void> = [];
    prFileContentMock.mockImplementation(
      (_platform: Platform, _owner: string, _repo: string, path: string, revision: string) =>
        new Promise<PrFileContent>((resolve) => {
          resolvers.push((value) => resolve({ ...value, path, revision }));
        }),
    );
    const wrapper = await mountViewer(contextDiff, contextProps);

    void wrapper.get(".context-gap-button").trigger("click");
    await flushPromises();
    expect(resolvers).toHaveLength(2);

    await wrapper.setProps({ baseSha: "new-base", headSha: "new-head" });
    resolvers[0](fileContent("src/context.old.ts", "base-sha", "stale base"));
    resolvers[1](fileContent("src/context.ts", "head-sha", "stale head"));
    await flushPromises();

    expect(wrapper.findAll(".controlled-context-line")).toHaveLength(0);
    expect(wrapper.text()).not.toContain("stale base");
    expect(wrapper.text()).not.toContain("stale head");
    expect(wrapper.find(".context-load-error").exists()).toBe(false);
  });
});

describe("DiffViewer 文件树", () => {
  beforeEach(() => {
    storage.clear();
    setActivePinia(createPinia());
  });

  it("按目录层级展示变更文件和统计", async () => {
    const wrapper = await mountViewer();

    expect(wrapper.get('[role="tree"]').text()).toContain("src");
    expect(wrapper.get('[role="tree"]').text()).toContain("components");
    expect(wrapper.get('[data-file-path="src/components/App.ts"]').text()).toContain("App.ts");
    expect(
      wrapper.get('[data-file-path="src/components/App.ts"] .tree-label').attributes("title"),
    ).toContain("修改文件");
    expect(
      wrapper.get('[data-file-path="tests/App.spec.ts"] .tree-label').attributes("title"),
    ).toContain("新增文件");
    expect(wrapper.get(".navigator-header").text()).toContain("+2");
    expect(wrapper.get(".navigator-header").text()).toContain("-1");
  });

  it("选择文件后右侧只显示对应的 Diff 上下文", async () => {
    const wrapper = await mountViewer();
    const renderedFiles = wrapper.findAll<HTMLElement>(".d2h-file-wrapper");

    expect(renderedFiles).toHaveLength(2);
    expect(renderedFiles[0].element.hidden).toBe(false);
    expect(renderedFiles[1].element.hidden).toBe(true);

    await wrapper.get('[data-file-path="tests/App.spec.ts"]').trigger("click");
    await flushPromises();

    expect(renderedFiles[0].element.hidden).toBe(true);
    expect(renderedFiles[1].element.hidden).toBe(false);
    expect(wrapper.get(".selected-file-name").text()).toBe("tests/App.spec.ts");
  });

  it("目录支持折叠并保留当前文件上下文", async () => {
    const wrapper = await mountViewer();
    const srcDirectory = wrapper
      .findAll<HTMLButtonElement>('[role="treeitem"]')
      .find((row) => !row.attributes("data-file-path") && row.text().trim() === "src");

    expect(srcDirectory).toBeDefined();
    if (!srcDirectory) throw new Error("未找到 src 目录节点");
    await srcDirectory.trigger("click");

    expect(
      wrapper.get('[role="tree"]').find('[data-file-path="src/components/App.ts"]').exists(),
    ).toBe(false);
    expect(wrapper.get(".selected-file-name").text()).toBe("src/components/App.ts");
  });

  it("顶部横向滚动条与左右 Diff 同步滚动", async () => {
    const wrapper = await mountViewer();
    const topScrollbar = wrapper.get<HTMLElement>(".diff-top-scrollbar");
    const sideScrollers = wrapper.findAll<HTMLElement>(".d2h-file-side-diff").slice(0, 2);

    expect(sideScrollers).toHaveLength(2);
    topScrollbar.element.scrollLeft = 120;
    await topScrollbar.trigger("scroll");
    expect(sideScrollers[0].element.scrollLeft).toBe(120);
    expect(sideScrollers[1].element.scrollLeft).toBe(120);
    expect(sideScrollers[0].element.style.getPropertyValue("--diff-line-number-offset")).toBe(
      "120px",
    );
    expect(sideScrollers[1].element.style.getPropertyValue("--diff-line-number-offset")).toBe(
      "120px",
    );

    sideScrollers[0].element.scrollLeft = 48;
    await sideScrollers[0].trigger("scroll");
    expect(sideScrollers[1].element.scrollLeft).toBe(48);
    expect(topScrollbar.element.scrollLeft).toBe(48);
  });

  it("关闭同步滚动后在顶部显示左右独立滚动条", async () => {
    useUiSettingsStore().setDiffSyncScrollEnabled(false);
    const wrapper = await mountViewer();
    const topScrollbars = wrapper.findAll<HTMLElement>(".diff-top-scrollbar");
    const sideScrollers = wrapper.findAll<HTMLElement>(".d2h-file-side-diff").slice(0, 2);

    expect(wrapper.get(".diff-top-scrollbars").classes()).toContain("independent");
    expect(topScrollbars).toHaveLength(2);
    expect(sideScrollers).toHaveLength(2);

    topScrollbars[0].element.scrollLeft = 64;
    await topScrollbars[0].trigger("scroll");

    expect(sideScrollers[0].element.scrollLeft).toBe(64);
    expect(sideScrollers[1].element.scrollLeft).toBe(0);
    expect(sideScrollers[0].element.style.getPropertyValue("--diff-line-number-offset")).toBe(
      "64px",
    );
    expect(sideScrollers[1].element.style.getPropertyValue("--diff-line-number-offset")).toBe(
      "0px",
    );

    sideScrollers[1].element.scrollLeft = 32;
    await sideScrollers[1].trigger("scroll");

    expect(topScrollbars[0].element.scrollLeft).toBe(64);
    expect(topScrollbars[1].element.scrollLeft).toBe(32);
  });

  it("低相似度替换行只保留整行浅色背景", async () => {
    const unrelatedDiff: DiffResult = {
      diff: `diff --git a/src/handler.ts b/src/handler.ts
index 1111111..2222222 100644
--- a/src/handler.ts
+++ b/src/handler.ts
@@ -1 +1 @@
-const oldHandler = createLegacyHandler();
+export async function loadRepositoryContext() {`,
      files: [
        {
          filename: "src/handler.ts",
          status: "modified",
          patch:
            "@@ -1 +1 @@\n-const oldHandler = createLegacyHandler();\n+export async function loadRepositoryContext() {",
          additions: 1,
          deletions: 1,
        },
      ],
      patch_schema_version: 1,
      patches: [],
    };

    const wrapper = await mountViewer(unrelatedDiff);
    const inlineHighlights = wrapper.findAll<HTMLElement>(
      ".d2h-code-line-ctn ins, .d2h-code-line-ctn del",
    );

    expect(inlineHighlights.length).toBeGreaterThan(0);
    expect(
      inlineHighlights.every((highlight) =>
        highlight.classes().includes("d2h-low-similarity-highlight"),
      ),
    ).toBe(true);
    expect(inlineHighlights.some((highlight) => highlight.element.tagName === "DEL")).toBe(true);
    expect(inlineHighlights.some((highlight) => highlight.element.tagName === "INS")).toBe(true);
  });

  it("局部字符变化仍保留词级高亮", async () => {
    const localChangeDiff: DiffResult = {
      diff: `diff --git a/src/config.ts b/src/config.ts
index 1111111..2222222 100644
--- a/src/config.ts
+++ b/src/config.ts
@@ -1 +1 @@
-const timeout = 1000;
+const timeout = 2000;`,
      files: [
        {
          filename: "src/config.ts",
          status: "modified",
          patch: "@@ -1 +1 @@\n-const timeout = 1000;\n+const timeout = 2000;",
          additions: 1,
          deletions: 1,
        },
      ],
      patch_schema_version: 1,
      patches: [],
    };

    const wrapper = await mountViewer(localChangeDiff);
    const inlineHighlights = wrapper.findAll<HTMLElement>(
      ".d2h-code-line-ctn ins, .d2h-code-line-ctn del",
    );

    expect(inlineHighlights).toHaveLength(2);
    expect(
      inlineHighlights.some((highlight) =>
        highlight.classes().includes("d2h-low-similarity-highlight"),
      ),
    ).toBe(false);
  });

  it("标准化高亮时仍将远端 HTML 作为文本渲染", async () => {
    const htmlDiff: DiffResult = {
      diff: `diff --git a/src/value.ts b/src/value.ts
index 1111111..2222222 100644
--- a/src/value.ts
+++ b/src/value.ts
@@ -1 +1 @@
-const value = "<script>";
+const value = "<safe>";`,
      files: [
        {
          filename: "src/value.ts",
          status: "modified",
          patch: '@@ -1 +1 @@\n-const value = "<script>";\n+const value = "<safe>";',
          additions: 1,
          deletions: 1,
        },
      ],
      patch_schema_version: 1,
      patches: [],
    };

    const wrapper = await mountViewer(htmlDiff);

    expect(wrapper.get(".diff2html-container").text()).toContain("<script>");
    expect(wrapper.get(".diff2html-container").text()).toContain("<safe>");
    expect(wrapper.find(".diff2html-container script").exists()).toBe(false);
    expect(wrapper.find(".diff2html-container safe").exists()).toBe(false);
  });

  it("可以隐藏和恢复文件导航栏", async () => {
    const wrapper = await mountViewer();
    const toggle = wrapper.get(".navigator-toggle");

    await toggle.trigger("click");
    expect(wrapper.find(".file-navigator").exists()).toBe(false);
    expect(wrapper.get(".diff-workspace").classes()).toContain("navigator-collapsed");

    await toggle.trigger("click");
    expect(wrapper.find(".file-navigator").exists()).toBe(true);
  });
});
