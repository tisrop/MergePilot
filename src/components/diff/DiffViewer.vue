<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted, nextTick } from "vue";
import { storeToRefs } from "pinia";
import { html } from "diff2html";
import "diff2html/bundles/css/diff2html.min.css";
import type { DiffResult, FileStatus, PrFile } from "@/types";
import AppSelect from "@/components/shared/AppSelect.vue";
import { useUiSettingsStore } from "@/stores/useUiSettingsStore";

const props = defineProps<{
  diff: DiffResult | null;
}>();

const uiSettings = useUiSettingsStore();
const { isDiffSyncScrollEnabled } = storeToRefs(uiSettings);

const emit = defineEmits<{
  addComment: [
    path: string,
    startLine: number,
    endLine: number,
    side: "left" | "right",
    body: string,
  ];
}>();

interface FileTreeNode {
  key: string;
  name: string;
  kind: "directory" | "file";
  children: FileTreeNode[];
  file: PrFile | null;
}

interface FileTreeRow extends FileTreeNode {
  depth: number;
}

const containerRef = ref<HTMLElement | null>(null);
const workspaceRef = ref<HTMLElement | null>(null);
const topScrollbarRef = ref<HTMLElement | null>(null);
const leftTopScrollbarRef = ref<HTMLElement | null>(null);
const rightTopScrollbarRef = ref<HTMLElement | null>(null);
const diffScrollRef = ref<HTMLElement | null>(null);
const topScrollbarContentWidth = ref(0);
const independentTopScrollbarWidths = ref<[number, number]>([0, 0]);
const navigatorVisible = ref(true);
const navigatorWidth = ref(270);
const resizingNavigator = ref(false);
const selectedFilePath = ref("");
const expandedDirectories = ref<Set<string>>(new Set());

const statusDescriptions: Record<FileStatus, string> = {
  added: "新增",
  modified: "修改",
  removed: "删除",
  renamed: "重命名",
};

function sortTree(nodes: FileTreeNode[]): void {
  nodes.sort((left, right) => {
    if (left.kind !== right.kind) return left.kind === "directory" ? -1 : 1;
    return left.name.localeCompare(right.name);
  });
  nodes.forEach((node) => sortTree(node.children));
}

function buildFileTree(files: PrFile[]): FileTreeNode[] {
  const root: FileTreeNode = {
    key: "",
    name: "",
    kind: "directory",
    children: [],
    file: null,
  };

  files.forEach((file) => {
    const segments = file.filename.split("/").filter(Boolean);
    if (segments.length === 0) return;

    let parent = root;
    let currentPath = "";
    segments.forEach((segment, index) => {
      currentPath = currentPath ? `${currentPath}/${segment}` : segment;
      const isFile = index === segments.length - 1;
      let child = parent.children.find(
        (node) => node.name === segment && node.kind === (isFile ? "file" : "directory"),
      );
      if (!child) {
        child = {
          key: isFile ? file.filename : `directory:${currentPath}`,
          name: segment,
          kind: isFile ? "file" : "directory",
          children: [],
          file: isFile ? file : null,
        };
        parent.children.push(child);
      }
      parent = child;
    });
  });

  sortTree(root.children);
  return root.children;
}

function firstFilePath(nodes: FileTreeNode[]): string {
  for (const node of nodes) {
    if (node.file) return node.file.filename;
    const nested = firstFilePath(node.children);
    if (nested) return nested;
  }
  return "";
}

function collectDirectoryKeys(nodes: FileTreeNode[], keys = new Set<string>()): Set<string> {
  nodes.forEach((node) => {
    if (node.kind === "directory") {
      keys.add(node.key);
      collectDirectoryKeys(node.children, keys);
    }
  });
  return keys;
}

const fileTree = computed(() => buildFileTree(props.diff?.files ?? []));
const visibleTreeRows = computed(() => {
  const rows: FileTreeRow[] = [];
  const visit = (nodes: FileTreeNode[], depth: number) => {
    nodes.forEach((node) => {
      rows.push({ ...node, depth });
      if (node.kind === "directory" && expandedDirectories.value.has(node.key)) {
        visit(node.children, depth + 1);
      }
    });
  };
  visit(fileTree.value, 1);
  return rows;
});
const selectedFile = computed(
  () => props.diff?.files.find((file) => file.filename === selectedFilePath.value) ?? null,
);
const totalAdditions = computed(() =>
  (props.diff?.files ?? []).reduce((total, file) => total + file.additions, 0),
);
const totalDeletions = computed(() =>
  (props.diff?.files ?? []).reduce((total, file) => total + file.deletions, 0),
);
const fileSignature = computed(() =>
  (props.diff?.files ?? []).map((file) => file.filename).join("\0"),
);

const renderedDiff = computed(() => props.diff?.diff ?? "");

const MAX_INLINE_HIGHLIGHT_RATIO = 0.8;
const LOW_SIMILARITY_HIGHLIGHT_CLASS = "d2h-low-similarity-highlight";

function textLength(value: string | null): number {
  return Array.from(value ?? "").length;
}

function normalizeInlineHighlights(renderedHtml: string): string {
  const document = new DOMParser().parseFromString(renderedHtml, "text/html");

  document.querySelectorAll<HTMLElement>(".d2h-code-line-ctn").forEach((line) => {
    const highlights = Array.from(line.querySelectorAll<HTMLElement>("ins, del"));
    const totalLength = textLength(line.textContent);
    if (highlights.length === 0 || totalLength === 0) return;

    const highlightedLength = highlights.reduce(
      (length, highlight) => length + textLength(highlight.textContent),
      0,
    );
    if (highlightedLength / totalLength < MAX_INLINE_HIGHLIGHT_RATIO) return;

    // diff2html 会把低相似度替换行的大部分内容标成词级变化。GitHub 对这种行只保留
    // 整行浅色背景，避免整段代码被误显示为深红或深绿。
    highlights.forEach((highlight) => highlight.classList.add(LOW_SIMILARITY_HIGHLIGHT_CLASS));
  });

  return document.body.innerHTML;
}

const diffHtml = computed(() => {
  if (!renderedDiff.value) return "";
  const options = {
    drawFileList: false,
    matching: "lines" as const,
    outputFormat: "side-by-side" as const,
    renderNothingWhenEmpty: false,
  };
  try {
    return normalizeInlineHighlights(html(renderedDiff.value, options));
  } catch {
    // 后端 patch 解析失败时始终回退到平台原始 diff，不能用空字符串覆盖视图。
    if (props.diff?.diff && renderedDiff.value !== props.diff.diff) {
      try {
        return normalizeInlineHighlights(html(props.diff.diff, options));
      } catch {
        return "";
      }
    }
    return "";
  }
});

function toggleDirectory(key: string) {
  const next = new Set(expandedDirectories.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expandedDirectories.value = next;
}

function activateTreeRow(row: FileTreeRow) {
  if (row.kind === "directory") {
    toggleDirectory(row.key);
  } else if (row.file) {
    void selectFile(row.file.filename);
  }
}

const MIN_NAVIGATOR_WIDTH = 180;
const MAX_NAVIGATOR_WIDTH = 520;
const MIN_DIFF_WIDTH = 320;

function clampNavigatorWidth(width: number): number {
  const workspaceWidth = workspaceRef.value?.clientWidth ?? window.innerWidth;
  const availableWidth = Math.max(MIN_NAVIGATOR_WIDTH, workspaceWidth - MIN_DIFF_WIDTH);
  return Math.round(
    Math.min(Math.max(width, MIN_NAVIGATOR_WIDTH), MAX_NAVIGATOR_WIDTH, availableWidth),
  );
}

function handleNavigatorResize(event: PointerEvent): void {
  if (!resizingNavigator.value || !workspaceRef.value) return;
  const workspaceLeft = workspaceRef.value.getBoundingClientRect().left;
  navigatorWidth.value = clampNavigatorWidth(event.clientX - workspaceLeft);
}

function stopNavigatorResize(): void {
  if (!resizingNavigator.value) return;
  resizingNavigator.value = false;
  document.removeEventListener("pointermove", handleNavigatorResize);
  document.removeEventListener("pointerup", stopNavigatorResize);
  document.removeEventListener("pointercancel", stopNavigatorResize);
  document.body.style.removeProperty("cursor");
  document.body.style.removeProperty("user-select");
}

function startNavigatorResize(event: PointerEvent): void {
  if (event.button !== 0) return;
  event.preventDefault();
  resizingNavigator.value = true;
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  document.addEventListener("pointermove", handleNavigatorResize);
  document.addEventListener("pointerup", stopNavigatorResize);
  document.addEventListener("pointercancel", stopNavigatorResize);
}

function resizeNavigatorWithKeyboard(event: KeyboardEvent): void {
  if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
  event.preventDefault();
  navigatorWidth.value = clampNavigatorWidth(
    navigatorWidth.value + (event.key === "ArrowLeft" ? -16 : 16),
  );
}

async function selectFile(path: string) {
  if (selectedFilePath.value === path) return;
  selectedFilePath.value = path;
  await nextTick();
  if (diffScrollRef.value) diffScrollRef.value.scrollTop = 0;
  setSideDiffScrollLeft(0);
  if (topScrollbarRef.value) topScrollbarRef.value.scrollLeft = 0;
}

function syncRenderedFile() {
  const wrappers = containerRef.value?.querySelectorAll<HTMLElement>(".d2h-file-wrapper");
  if (!wrappers?.length) return;

  const diffPaths = Array.from(
    props.diff?.diff.matchAll(/^diff --git a\/(.+?) b\/(.+)$/gm) ?? [],
    (match) => match[2],
  );
  wrappers.forEach((wrapper, index) => {
    const path = diffPaths[index] || props.diff?.files[index]?.filename || "";
    wrapper.dataset.filePath = path;
    wrapper.hidden = Boolean(selectedFilePath.value && path !== selectedFilePath.value);
  });
}

let diffResizeObserver: ResizeObserver | null = null;

function visibleSideDiffScrollers(): HTMLElement[] {
  return Array.from(
    containerRef.value?.querySelectorAll<HTMLElement>(".d2h-file-side-diff") ?? [],
  ).filter((scroller) => !scroller.closest<HTMLElement>(".d2h-file-wrapper")?.hidden);
}

function updateLineNumberGutterOffset(scroller: HTMLElement): void {
  scroller.style.setProperty("--diff-line-number-offset", `${scroller.scrollLeft}px`);
}

function setSideDiffScrollerScrollLeft(scroller: HTMLElement, scrollLeft: number): void {
  if (scroller.scrollLeft !== scrollLeft) scroller.scrollLeft = scrollLeft;
  updateLineNumberGutterOffset(scroller);
}

function setSideDiffScrollLeft(scrollLeft: number, source?: HTMLElement): void {
  for (const scroller of visibleSideDiffScrollers()) {
    if (scroller !== source) setSideDiffScrollerScrollLeft(scroller, scrollLeft);
  }
}

function bindSideDiffScrollers(): void {
  for (const scroller of visibleSideDiffScrollers()) {
    updateLineNumberGutterOffset(scroller);
    scroller.addEventListener("scroll", handleSideDiffScroll);
  }
}

function updateTopScrollbar(): void {
  const sideScrollers = visibleSideDiffScrollers();
  if (isDiffSyncScrollEnabled.value) {
    const topScroller = topScrollbarRef.value;
    if (!topScroller) return;
    const maxScrollRange = sideScrollers.reduce(
      (maximum, scroller) => Math.max(maximum, scroller.scrollWidth - scroller.clientWidth),
      0,
    );
    topScrollbarContentWidth.value = topScroller.clientWidth + maxScrollRange;
    const sideScrollLeft = sideScrollers[0]?.scrollLeft ?? 0;
    if (topScroller.scrollLeft !== sideScrollLeft) topScroller.scrollLeft = sideScrollLeft;
    return;
  }

  const topScrollers = [leftTopScrollbarRef.value, rightTopScrollbarRef.value];
  independentTopScrollbarWidths.value = topScrollers.map((topScroller, index) => {
    if (!topScroller) return 0;
    const sideScroller = sideScrollers[index];
    const scrollRange = sideScroller
      ? Math.max(0, sideScroller.scrollWidth - sideScroller.clientWidth)
      : 0;
    if (sideScroller && topScroller.scrollLeft !== sideScroller.scrollLeft) {
      topScroller.scrollLeft = sideScroller.scrollLeft;
    }
    return topScroller.clientWidth + scrollRange;
  }) as [number, number];
}

function handleTopScrollbarScroll(): void {
  const topScroller = topScrollbarRef.value;
  if (!topScroller) return;
  setSideDiffScrollLeft(topScroller.scrollLeft);
}

function handleIndependentTopScrollbarScroll(sideIndex: number): void {
  const topScroller = sideIndex === 0 ? leftTopScrollbarRef.value : rightTopScrollbarRef.value;
  const sideScroller = visibleSideDiffScrollers()[sideIndex];
  if (!topScroller || !sideScroller || sideScroller.scrollLeft === topScroller.scrollLeft) return;
  setSideDiffScrollerScrollLeft(sideScroller, topScroller.scrollLeft);
}

function handleSideDiffScroll(event: Event): void {
  const source = event.target;
  if (!(source instanceof HTMLElement) || !source.classList.contains("d2h-file-side-diff")) return;
  updateLineNumberGutterOffset(source);
  if (isDiffSyncScrollEnabled.value) {
    setSideDiffScrollLeft(source.scrollLeft, source);
    if (topScrollbarRef.value && topScrollbarRef.value.scrollLeft !== source.scrollLeft) {
      topScrollbarRef.value.scrollLeft = source.scrollLeft;
    }
    return;
  }

  const sideIndex = visibleSideDiffScrollers().indexOf(source);
  const topScroller =
    sideIndex === 0
      ? leftTopScrollbarRef.value
      : sideIndex === 1
        ? rightTopScrollbarRef.value
        : null;
  if (topScroller && topScroller.scrollLeft !== source.scrollLeft) {
    topScroller.scrollLeft = source.scrollLeft;
  }
}

function handleDiffWheel(event: WheelEvent): void {
  const delta = event.shiftKey ? event.deltaY : event.deltaX;
  const isHorizontalGesture = event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY);
  if (!isHorizontalGesture || delta === 0) return;

  const sideScrollers = visibleSideDiffScrollers();
  const eventTarget = event.target instanceof Element ? event.target : null;
  const source = eventTarget?.closest<HTMLElement>(".d2h-file-side-diff") ?? null;
  const sideIndex = source ? sideScrollers.indexOf(source) : -1;
  const topScroller = isDiffSyncScrollEnabled.value
    ? topScrollbarRef.value
    : sideIndex === 0
      ? leftTopScrollbarRef.value
      : sideIndex === 1
        ? rightTopScrollbarRef.value
        : null;
  if (!topScroller) return;

  event.preventDefault();
  topScroller.scrollLeft += delta;
  if (isDiffSyncScrollEnabled.value) handleTopScrollbarScroll();
  else handleIndependentTopScrollbarScroll(sideIndex);
}

function observeDiffSize(): void {
  diffResizeObserver?.disconnect();
  if (typeof ResizeObserver === "undefined") return;
  diffResizeObserver = new ResizeObserver(updateTopScrollbar);
  if (diffScrollRef.value) diffResizeObserver.observe(diffScrollRef.value);
  if (containerRef.value) diffResizeObserver.observe(containerRef.value);
  for (const scroller of visibleSideDiffScrollers()) diffResizeObserver.observe(scroller);
}

watch(isDiffSyncScrollEnabled, async (enabled) => {
  await nextTick();
  if (enabled) {
    const currentScrollLeft = visibleSideDiffScrollers()[0]?.scrollLeft ?? 0;
    setSideDiffScrollLeft(currentScrollLeft);
  }
  updateTopScrollbar();
  observeDiffSize();
});

watch(
  fileSignature,
  () => {
    const files = props.diff?.files ?? [];
    selectedFilePath.value = firstFilePath(fileTree.value);
    expandedDirectories.value = collectDirectoryKeys(fileTree.value);
  },
  { immediate: true },
);

// 同一批文件再次加载（例如切换 PR 但文件名未变）时，也回到第一个文件。
watch(
  () => props.diff?.diff,
  (next, previous) => {
    if (next !== previous) selectedFilePath.value = firstFilePath(fileTree.value);
  },
);

watch(
  [diffHtml, selectedFilePath],
  async () => {
    await nextTick();
    syncRenderedFile();
    bindSideDiffScrollers();
    updateTopScrollbar();
    observeDiffSize();
  },
  { immediate: true, flush: "post" },
);

const popupRef = ref<HTMLElement | null>(null);

const quickComment = ref<{
  x: number;
  y: number;
  path: string;
  startLine: number;
  endLine: number;
  side: "left" | "right";
  selectedText: string;
} | null>(null);
const quickBody = ref("");
const quickSubmitting = ref(false);
const quickCategory = ref("logic");
const quickSubCategory = ref("");

const categories: Record<string, string[]> = {
  logic: ["边界条件", "空值处理", "异常处理", "并发问题", "状态管理", "类型错误"],
  security: ["注入攻击", "权限控制", "敏感信息泄露", "加密问题", "输入校验", "CSRF/XSS"],
  performance: ["算法复杂度", "内存泄漏", "IO阻塞", "重复计算", "缓存优化", "数据库查询"],
  style: ["命名规范", "注释缺失", "代码冗余", "硬编码", "函数过长", "结构混乱"],
  log: ["日志级别不当", "敏感信息打印", "日志缺失", "异常信息不全", "日志格式", "日志过多"],
};
const categoryLabels: Record<string, string> = {
  logic: "逻辑类",
  security: "安全类",
  performance: "性能类",
  style: "代码风格类",
  log: "日志类",
};

const opinionTemplates: Record<string, string> = {
  边界条件: "请检查此处的边界条件是否处理完整，包括空值、越界、临界值等场景。",
  空值处理: "此处缺少空值判断，建议增加 null/undefined 保护。",
  异常处理: "建议完善异常处理逻辑，确保异常路径能被正确捕获和处理。",
  并发问题: "此处存在并发安全问题，建议考虑加锁或使用原子操作。",
  状态管理: "状态管理逻辑不够清晰，建议简化或拆分状态管理。",
  类型错误: "存在类型不匹配问题，建议使用更精确的类型定义。",
  注入攻击: "存在注入风险，建议使用参数化查询或对输入进行严格过滤。",
  权限控制: "缺少必要的权限校验，建议在此处增加权限检查。",
  敏感信息泄露: "可能泄露敏感信息，建议避免在输出中暴露内部细节。",
  加密问题: "加密方案不够安全，建议使用更安全的加密算法。",
  输入校验: "缺少输入校验，建议对用户输入进行合法性检查。",
  "CSRF/XSS": "存在跨站攻击风险，建议增加 CSRF Token 或 XSS 过滤。",
  算法复杂度: "算法复杂度过高，建议优化以提升性能。",
  内存泄漏: "可能存在内存泄漏风险，请检查资源释放路径。",
  IO阻塞: "IO 操作未异步处理，可能阻塞主线程，建议异步化。",
  重复计算: "存在重复计算，建议提取为变量或缓存结果。",
  缓存优化: "缓存策略可以进一步优化，减少不必要的缓存更新。",
  数据库查询: "数据库查询效率较低，建议添加索引或优化查询。",
  命名规范: "命名不够规范，建议遵循项目命名约定。",
  注释缺失: "此处逻辑较复杂，建议补充注释说明意图。",
  代码冗余: "代码存在冗余，建议抽取为公共方法复用。",
  硬编码: "存在硬编码值，建议抽取为常量或配置项。",
  函数过长: "函数过长，建议拆分为多个小函数。",
  结构混乱: "代码结构不够清晰，建议重新组织逻辑。",
  日志级别不当: "日志级别设置不当，建议根据场景调整。",
  敏感信息打印: "日志中可能包含敏感信息，建议脱敏处理。",
  日志缺失: "关键路径缺少日志，建议补充以方便排查。",
  异常信息不全: "异常信息不够详细，建议补充上下文。",
  日志格式: "日志格式不规范，建议统一格式。",
  日志过多: "日志输出过于频繁，可能影响性能。",
};

function getFileFromNode(node: Node): HTMLElement | null {
  let el: HTMLElement | null =
    node.nodeType === Node.ELEMENT_NODE
      ? (node as HTMLElement)
      : (node.parentElement as HTMLElement);
  while (el) {
    const cls = el.classList;
    if (cls.contains("d2h-file-wrapper") || cls.contains("d2h-wrapper")) return el;
    if (cls.contains("d2h-files-diff") || cls.contains("d2h-file-side-diff")) {
      let p = el.parentElement;
      while (p) {
        if (p.classList.contains("d2h-file-wrapper")) return p;
        p = p.parentElement;
      }
    }
    el = el.parentElement;
  }
  return null;
}

function getSelectionRange(): {
  path: string;
  startLine: number;
  endLine: number;
  side: "left" | "right";
} | null {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed) return null;
  if (!sel.toString().trim()) return null;

  const range = sel.getRangeAt(0);
  if (!range) return null;

  const startFile = getFileFromNode(range.startContainer);
  const endFile = getFileFromNode(range.endContainer);
  if (!startFile || startFile !== endFile) return null;

  const fileNameEl =
    startFile.querySelector(".d2h-file-name") ||
    startFile.querySelector(".d2h-file-name-wrapper .d2h-file-name");
  const filePath = fileNameEl?.textContent?.trim() || "";
  if (!filePath) return null;

  const lines: number[] = [];
  let side: "left" | "right" | null = null;

  startFile.querySelectorAll("tr").forEach((row) => {
    if (!range.intersectsNode(row)) return;
    const lnEls = row.querySelectorAll(".d2h-code-side-linenumber, .d2h-code-linenumber");
    lnEls.forEach((el) => {
      const num = parseInt((el as HTMLElement).textContent || "0", 10);
      if (!num) return;
      lines.push(num);
      if (!side) {
        let p: HTMLElement | null = el as HTMLElement;
        while (p && !p.classList.contains("d2h-file-side-diff")) p = p.parentElement;
        if (p) {
          const container = p.parentElement;
          const sideDiffs = container?.querySelectorAll(".d2h-file-side-diff");
          const isLeft = sideDiffs && sideDiffs.length > 1 && sideDiffs[0] === p;
          side = isLeft ? "left" : "right";
        }
      }
    });
  });

  if (lines.length === 0 || !side) return null;
  return {
    path: filePath,
    startLine: Math.min(...lines),
    endLine: Math.max(...lines),
    side,
  };
}

function handleContextMenu(event: MouseEvent) {
  const target = event.target as HTMLElement;
  if (!containerRef.value?.contains(target)) return;

  event.preventDefault();
  event.stopPropagation();

  const selRange = getSelectionRange();
  if (!selRange) return;
  quickComment.value = {
    x: event.clientX,
    y: event.clientY,
    path: selRange.path,
    startLine: selRange.startLine,
    endLine: selRange.endLine,
    side: selRange.side,
    selectedText: window.getSelection()?.toString().trim() || "",
  };
  quickBody.value = "";
}

function handleDocClick() {
  quickComment.value = null;
}

async function submitQuickComment() {
  if (!quickComment.value || !quickBody.value.trim()) return;
  let finalBody = quickBody.value.trim();
  if (!finalBody.startsWith("【")) {
    const main = categoryLabels[quickCategory.value] || quickCategory.value;
    const sub = quickSubCategory.value ? `-${quickSubCategory.value}` : "";
    finalBody = `【${main}${sub}】${finalBody}`;
  }
  emit(
    "addComment",
    quickComment.value.path,
    quickComment.value.startLine,
    quickComment.value.endLine,
    quickComment.value.side,
    finalBody,
  );
  quickSubmitting.value = true;
  await new Promise((r) => setTimeout(r, 200));
  quickComment.value = null;
  quickBody.value = "";
  quickSubmitting.value = false;
}

function onSubCategoryChange() {
  if (!quickSubCategory.value) {
    quickBody.value = "";
    return;
  }
  const tpl = opinionTemplates[quickSubCategory.value];
  if (tpl) {
    const main = categoryLabels[quickCategory.value] || quickCategory.value;
    quickBody.value = `【${main}-${quickSubCategory.value}】${tpl}`;
  }
}

function handleQuickKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) submitQuickComment();
  if (e.key === "Escape") quickComment.value = null;
}

async function adjustPopupPosition() {
  await nextTick();
  const el = popupRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const overflowRight = rect.right - window.innerWidth;
  const overflowBottom = rect.bottom - window.innerHeight;
  if (overflowRight > 0) {
    el.style.left = Math.max(0, rect.left - overflowRight) + "px";
  }
  if (overflowBottom > 0) {
    el.style.top = Math.max(0, rect.top - overflowBottom) + "px";
  }
}

watch(quickComment, async (val) => {
  if (val) {
    (document.querySelector(".quick-comment-textarea") as HTMLTextAreaElement)?.focus();
    await adjustPopupPosition();
  }
});

watch([quickCategory, quickSubCategory], async () => {
  if (quickComment.value) {
    await adjustPopupPosition();
  }
});

onMounted(() => {
  updateTopScrollbar();
  observeDiffSize();
  bindSideDiffScrollers();
  document.addEventListener("contextmenu", handleContextMenu, true);
  document.addEventListener("click", handleDocClick);
});

onUnmounted(() => {
  stopNavigatorResize();
  diffResizeObserver?.disconnect();
  for (const scroller of visibleSideDiffScrollers()) {
    scroller.removeEventListener("scroll", handleSideDiffScroll);
  }
  document.removeEventListener("contextmenu", handleContextMenu, true);
  document.removeEventListener("click", handleDocClick);
});
</script>

<template>
  <div class="diff-viewer-wrapper">
    <section
      v-if="diffHtml"
      ref="workspaceRef"
      class="diff-workspace"
      :class="{
        'navigator-collapsed': !navigatorVisible,
        resizing: resizingNavigator,
      }"
      :style="{ '--navigator-width': `${navigatorWidth}px` }"
      aria-label="代码差异浏览器"
    >
      <aside v-if="navigatorVisible" class="file-navigator" aria-label="变更文件">
        <header class="navigator-header">
          <div>
            <strong>文件</strong>
            <span>{{ diff?.files.length ?? 0 }}</span>
          </div>
          <div class="change-summary" aria-label="变更统计">
            <span class="additions">+{{ totalAdditions }}</span>
            <span class="deletions">-{{ totalDeletions }}</span>
          </div>
        </header>

        <nav class="file-tree" role="tree" aria-label="变更文件目录树">
          <button
            v-for="row in visibleTreeRows"
            :key="row.key"
            class="tree-row"
            :class="{ selected: row.file?.filename === selectedFilePath }"
            :style="{ '--tree-depth': row.depth }"
            type="button"
            role="treeitem"
            :aria-level="row.depth"
            :aria-expanded="row.kind === 'directory' ? expandedDirectories.has(row.key) : undefined"
            :aria-current="row.file?.filename === selectedFilePath ? 'true' : undefined"
            :aria-label="
              row.file
                ? `${statusDescriptions[row.file.status]}文件：${row.file.filename}`
                : `目录：${row.name}`
            "
            :data-file-path="row.file?.filename"
            @click="activateTreeRow(row)"
          >
            <svg
              v-if="row.kind === 'directory'"
              class="disclosure-icon"
              :class="{ expanded: expandedDirectories.has(row.key) }"
              width="12"
              height="12"
              viewBox="0 0 12 12"
              fill="none"
              aria-hidden="true"
            >
              <path d="m4 2.5 3.5 3.5L4 9.5" stroke="currentColor" stroke-width="1.4" />
            </svg>
            <span v-else class="disclosure-spacer" aria-hidden="true" />
            <svg
              v-if="row.kind === 'directory'"
              class="tree-icon directory-icon"
              width="15"
              height="15"
              viewBox="0 0 16 16"
              fill="none"
              aria-hidden="true"
            >
              <path
                d="M1.75 4.25h4l1.3 1.5h7.2v6.5a1.5 1.5 0 0 1-1.5 1.5h-10a1.5 1.5 0 0 1-1.5-1.5v-6.5c0-.83.67-1.5 1.5-1.5Z"
                stroke="currentColor"
                stroke-width="1.2"
              />
            </svg>
            <svg
              v-else
              class="tree-icon"
              width="15"
              height="15"
              viewBox="0 0 16 16"
              fill="none"
              aria-hidden="true"
            >
              <path d="M4 1.75h5l3 3v9.5H4v-12.5Z" stroke="currentColor" stroke-width="1.2" />
              <path d="M9 1.75v3h3" stroke="currentColor" stroke-width="1.2" />
            </svg>
            <span
              class="tree-label"
              :title="
                row.file
                  ? `${statusDescriptions[row.file.status]}文件：${row.file.filename}`
                  : row.name
              "
            >
              {{ row.name }}
            </span>
            <template v-if="row.file">
              <span class="file-change-count" aria-hidden="true">
                <span v-if="row.file.additions" class="additions">+{{ row.file.additions }}</span>
                <span v-if="row.file.deletions" class="deletions">-{{ row.file.deletions }}</span>
              </span>
            </template>
          </button>
        </nav>
        <div
          class="navigator-resizer"
          role="separator"
          aria-label="调整文件列表宽度"
          aria-orientation="vertical"
          :aria-valuemin="MIN_NAVIGATOR_WIDTH"
          :aria-valuemax="MAX_NAVIGATOR_WIDTH"
          :aria-valuenow="navigatorWidth"
          tabindex="0"
          @pointerdown="startNavigatorResize"
          @keydown="resizeNavigatorWithKeyboard"
        />
      </aside>

      <section class="diff-context" aria-label="文件差异上下文">
        <header class="diff-toolbar">
          <button
            class="navigator-toggle"
            type="button"
            :aria-pressed="navigatorVisible"
            :title="navigatorVisible ? '隐藏文件列表' : '显示文件列表'"
            @click="navigatorVisible = !navigatorVisible"
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <rect x="1.75" y="2.25" width="12.5" height="11.5" rx="1.5" stroke="currentColor" />
              <path d="M5.25 2.5v11" stroke="currentColor" />
            </svg>
          </button>
          <div class="selected-file-heading">
            <span class="selected-file-name" :title="selectedFile?.filename">
              {{ selectedFile?.filename ?? "全部变更" }}
            </span>
            <span v-if="selectedFile" class="selected-file-status">
              {{ statusDescriptions[selectedFile.status] }}
            </span>
          </div>
          <div v-if="selectedFile" class="selected-file-stats" aria-label="当前文件变更统计">
            <span class="additions">+{{ selectedFile.additions }}</span>
            <span class="deletions">-{{ selectedFile.deletions }}</span>
          </div>
        </header>

        <div class="diff-top-scrollbars" :class="{ independent: !isDiffSyncScrollEnabled }">
          <div
            v-if="isDiffSyncScrollEnabled"
            ref="topScrollbarRef"
            class="diff-top-scrollbar"
            role="region"
            aria-label="同步代码差异横向滚动条"
            tabindex="0"
            @scroll="handleTopScrollbarScroll"
          >
            <div
              class="diff-top-scrollbar-content"
              :style="{ width: `${topScrollbarContentWidth}px` }"
              aria-hidden="true"
            />
          </div>
          <template v-else>
            <div
              ref="leftTopScrollbarRef"
              class="diff-top-scrollbar"
              role="region"
              aria-label="左侧代码差异横向滚动条"
              tabindex="0"
              @scroll="handleIndependentTopScrollbarScroll(0)"
            >
              <div
                class="diff-top-scrollbar-content"
                :style="{ width: `${independentTopScrollbarWidths[0]}px` }"
                aria-hidden="true"
              />
            </div>
            <div
              ref="rightTopScrollbarRef"
              class="diff-top-scrollbar"
              role="region"
              aria-label="右侧代码差异横向滚动条"
              tabindex="0"
              @scroll="handleIndependentTopScrollbarScroll(1)"
            >
              <div
                class="diff-top-scrollbar-content"
                :style="{ width: `${independentTopScrollbarWidths[1]}px` }"
                aria-hidden="true"
              />
            </div>
          </template>
        </div>
        <div ref="diffScrollRef" class="diff-scroll-region" @wheel="handleDiffWheel">
          <div ref="containerRef" class="diff2html-container" v-html="diffHtml" />
        </div>
      </section>
    </section>

    <div v-else class="diff-empty">暂无 diff 数据</div>

    <Teleport to="body">
      <div
        v-if="quickComment"
        ref="popupRef"
        class="quick-comment-popup"
        :style="{ left: quickComment.x + 'px', top: quickComment.y - 8 + 'px' }"
        @click.stop
        @keydown="handleQuickKeydown"
      >
        <div class="popup-header">
          <span class="file-ref">
            {{ quickComment.path.split("/").pop() }}:L{{ quickComment.startLine
            }}{{
              quickComment.endLine !== quickComment.startLine ? "-L" + quickComment.endLine : ""
            }}
          </span>
          <button class="close-btn" @click="quickComment = null">
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
        <pre v-if="quickComment.selectedText" class="selected-code">{{
          quickComment.selectedText
        }}</pre>

        <div class="popup-category">
          <AppSelect
            v-model="quickCategory"
            :options="Object.entries(categoryLabels).map(([value, label]) => ({ value, label }))"
            @update:model-value="
              quickSubCategory = '';
              quickBody = '';
            "
          />
          <AppSelect
            v-if="categories[quickCategory]"
            v-model="quickSubCategory"
            :options="[
              { value: '', label: '-- 二级分类 --' },
              ...categories[quickCategory].map((sub: string) => ({ value: sub, label: sub })),
            ]"
            @update:model-value="onSubCategoryChange"
          />
        </div>

        <textarea
          v-model="quickBody"
          class="quick-comment-textarea"
          placeholder="输入评审意见... (⌘+Enter 提交, Esc 取消)"
          rows="3"
        />
        <div class="popup-actions">
          <button class="btn btn-sm" @click="quickComment = null">取消</button>
          <button
            class="btn btn-sm btn-primary"
            :disabled="!quickBody.trim() || quickSubmitting"
            @click="submitQuickComment"
          >
            {{ quickSubmitting ? "提交中..." : "提交" }}
          </button>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.diff-workspace {
  display: grid;
  grid-template-columns: minmax(180px, var(--navigator-width)) minmax(0, 1fr);
  height: clamp(480px, 68vh, 760px);
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-surface);
  box-shadow: var(--shadow-sm);
}

.diff-workspace.navigator-collapsed {
  grid-template-columns: minmax(0, 1fr);
}

.file-navigator {
  position: relative;
  display: flex;
  min-width: 0;
  flex-direction: column;
  overflow: hidden;
  border-right: 1px solid var(--color-border);
  background: color-mix(in srgb, var(--color-surface) 92%, var(--color-bg));
}

.navigator-resizer {
  position: absolute;
  z-index: 3;
  top: 0;
  right: -4px;
  bottom: 0;
  width: 8px;
  cursor: col-resize;
  touch-action: none;
}

.navigator-resizer::after {
  position: absolute;
  top: 0;
  right: 3px;
  bottom: 0;
  width: 2px;
  background: transparent;
  content: "";
  transition: background var(--transition-fast);
}

.navigator-resizer:hover::after,
.navigator-resizer:focus-visible::after,
.diff-workspace.resizing .navigator-resizer::after {
  background: var(--color-primary);
}

.navigator-resizer:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
}

.navigator-header,
.diff-toolbar {
  display: flex;
  min-height: 42px;
  align-items: center;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface-hover);
}

.navigator-header {
  justify-content: space-between;
  padding: 0 var(--space-3);
  color: var(--color-text-secondary);
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.navigator-header > div {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.navigator-header strong {
  color: var(--color-text);
  font-size: 12px;
}

.change-summary,
.selected-file-stats,
.file-change-count {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-family: var(--font-mono);
  font-size: 10px;
}

.additions {
  color: var(--color-success);
}

.deletions {
  color: var(--color-danger);
}

.file-tree {
  flex: 1;
  overflow: auto;
  padding: var(--space-1);
  overscroll-behavior: contain;
}

.tree-row {
  display: flex;
  width: 100%;
  min-height: 28px;
  align-items: center;
  gap: 5px;
  padding: 3px 6px 3px calc(6px + (var(--tree-depth) - 1) * 14px);
  overflow: hidden;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  text-align: left;
  user-select: none;
}

.tree-row:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.tree-row.selected {
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.disclosure-icon,
.disclosure-spacer {
  width: 12px;
  min-width: 12px;
}

.disclosure-icon {
  transition: transform var(--transition-fast);
}

.disclosure-icon.expanded {
  transform: rotate(90deg);
}

.tree-icon {
  width: 15px;
  min-width: 15px;
  color: var(--color-text-tertiary);
}

.directory-icon {
  color: var(--color-warning);
}

.tree-label {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: #1f2328;
  font-family:
    "Mona Sans VF",
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    "Noto Sans",
    Helvetica,
    Arial,
    sans-serif,
    "Apple Color Emoji",
    "Segoe UI Emoji";
  font-size: 13px;
  font-weight: 400;
  line-height: 1.5;
  letter-spacing: normal;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-change-count {
  min-width: 0;
  color: var(--color-text-tertiary);
}

.diff-context {
  display: flex;
  min-width: 0;
  flex-direction: column;
  overflow: hidden;
  background: var(--color-surface);
}

.diff-toolbar {
  gap: var(--space-2);
  padding: 0 var(--space-3) 0 var(--space-2);
}

.navigator-toggle {
  display: inline-flex;
  width: 28px;
  height: 28px;
  min-width: 28px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
}

.navigator-toggle:hover,
.navigator-toggle[aria-pressed="true"] {
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.selected-file-heading {
  display: flex;
  min-width: 0;
  flex: 1;
  align-items: center;
  gap: var(--space-2);
}

.selected-file-name {
  min-width: 0;
  overflow: hidden;
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.selected-file-status {
  padding: 1px 6px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-full, 999px);
  color: var(--color-text-secondary);
  font-size: 10px;
  white-space: nowrap;
}

.selected-file-stats {
  flex-shrink: 0;
  font-size: 11px;
}

.diff-top-scrollbars {
  display: grid;
  min-width: 0;
  height: 14px;
  flex: none;
  grid-template-columns: minmax(0, 1fr);
  border-bottom: 1px solid var(--color-border-light);
  background: var(--color-surface);
}

.diff-top-scrollbars.independent {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.diff-top-scrollbars.independent .diff-top-scrollbar + .diff-top-scrollbar {
  border-left: 1px solid var(--color-border-light);
}

.diff-top-scrollbar {
  min-width: 0;
  height: 14px;
  overflow-x: auto;
  overflow-y: hidden;
  overscroll-behavior-x: contain;
}

.diff-top-scrollbar-content {
  height: 1px;
}

.diff-scroll-region {
  min-width: 0;
  flex: 1;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  background: var(--color-surface);
}

.diff2html-container {
  --d2h-ins-bg-color: var(--diff-add-bg);
  --d2h-ins-border-color: var(--diff-add-line);
  --d2h-ins-highlight-bg-color: var(--diff-add-line);
  --d2h-change-ins-color: var(--diff-add-bg);
  --d2h-del-bg-color: var(--diff-remove-bg);
  --d2h-del-border-color: var(--diff-remove-line);
  --d2h-del-highlight-bg-color: var(--diff-remove-line);
  --d2h-change-del-color: var(--diff-remove-bg);

  width: 100%;
  min-width: 0;
  padding: var(--space-3);
}

.diff2html-container :deep(ins.d2h-low-similarity-highlight),
.diff2html-container :deep(del.d2h-low-similarity-highlight) {
  background-color: transparent;
}

.diff2html-container :deep(.d2h-file-side-diff) {
  position: relative;
  overflow-x: auto;
  scrollbar-width: none;
}

.diff2html-container :deep(.d2h-file-side-diff::-webkit-scrollbar) {
  display: none;
}

.diff2html-container :deep(.d2h-code-side-linenumber),
.diff2html-container :deep(.d2h-code-linenumber) {
  z-index: 1;
  transform: translateX(var(--diff-line-number-offset, 0px));
}

.diff2html-container :deep(.d2h-code-side-linenumber.d2h-del),
.diff2html-container :deep(.d2h-code-linenumber.d2h-del) {
  background-color: var(--diff-remove-line);
}

.diff2html-container :deep(.d2h-code-side-linenumber.d2h-ins),
.diff2html-container :deep(.d2h-code-linenumber.d2h-ins) {
  background-color: var(--diff-add-line);
}

.diff2html-container :deep(.d2h-code-linenumber:hover::after),
.diff2html-container :deep(.d2h-code-linenumber.d2h-info::after) {
  content: "";
  position: absolute;
  top: 50%;
  right: 5px;
  width: 7px;
  height: 7px;
  border: 2px solid var(--color-brand-accent-strong);
  border-radius: 50%;
  background: var(--color-surface);
  box-shadow: 0 0 0 3px rgba(85, 224, 204, 0.13);
  transform: translateY(-50%);
}

.diff2html-container :deep(.d2h-file-wrapper) {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  margin: 0;
  overflow: hidden;
}

.diff2html-container :deep(.d2h-file-header) {
  display: none;
}

@media (max-width: 900px) {
  .diff-workspace {
    grid-template-columns: minmax(180px, var(--navigator-width)) minmax(0, 1fr);
  }

  .file-change-count {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .disclosure-icon {
    transition: none;
  }
}

.diff-empty {
  padding: var(--space-10);
  text-align: center;
  color: var(--color-text-tertiary);
}

.quick-comment-popup {
  position: fixed;
  z-index: 10000;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-xl);
  padding: var(--space-2) var(--space-3);
  min-width: 300px;
  max-width: 420px;
}

.popup-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-2);
}

.file-ref {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--color-text-secondary);
  background: var(--color-surface-hover);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
}

.close-btn {
  border: none;
  background: none;
  color: var(--color-text-tertiary);
  cursor: pointer;
  padding: 2px;
  line-height: 1;
  display: flex;
  align-items: center;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
}

.close-btn:hover {
  background: var(--color-surface-hover);
}

.selected-code {
  margin: 0 0 var(--space-2) 0;
  padding: var(--space-1) var(--space-2);
  background: var(--color-surface-hover);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-family: var(--font-mono);
  line-height: 1.4;
  max-height: 120px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--color-text);
}

.quick-comment-textarea {
  width: 100%;
  padding: var(--space-2) var(--space-2);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-family: inherit;
  resize: vertical;
  min-height: 60px;
  box-sizing: border-box;
  background: var(--color-surface);
  color: var(--color-text);
  transition:
    border-color var(--transition-fast),
    box-shadow var(--transition-fast);
}

.quick-comment-textarea:focus {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px var(--color-primary-light);
}

.popup-category {
  display: flex;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
}

.popup-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-1);
  margin-top: var(--space-2);
}
</style>
