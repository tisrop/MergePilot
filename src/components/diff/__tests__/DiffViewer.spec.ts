import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DiffViewer from "@/components/diff/DiffViewer.vue";
import { useUiSettingsStore } from "@/stores/useUiSettingsStore";
import type { DiffResult } from "@/types";

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
};

async function mountViewer(value = diff) {
  const wrapper = mount(DiffViewer, { props: { diff: value } });
  await flushPromises();
  return wrapper;
}

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
