<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted, nextTick } from "vue";
import { storeToRefs } from "pinia";
import { html } from "diff2html";
import "diff2html/bundles/css/diff2html.min.css";
import type { DiffResult, FileStatus, PatchHunk, PatchLine, Platform, PrFile } from "@/types";
import { prFileContent } from "@/api";
import AppSelect from "@/components/shared/AppSelect.vue";
import { useUiSettingsStore } from "@/stores/useUiSettingsStore";

const props = defineProps<{
  diff: DiffResult | null;
  platform?: Platform;
  owner?: string;
  repo?: string;
  baseSha?: string;
  headSha?: string;
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

interface ControlledDiffRow {
  key: string;
  left: PatchLine | null;
  right: PatchLine | null;
}

interface ControlledContextGap {
  key: string;
  oldStart: number;
  oldEnd: number;
  newStart: number;
  newEnd: number;
  direction: "up" | "both" | "down";
}

interface ControlledDiffHunk {
  key: string;
  hunk: PatchHunk;
  rows: ControlledDiffRow[];
  gapBefore: ControlledContextGap | null;
}

interface LoadedFileContext {
  identity: string;
  baseLines: string[];
  headLines: string[];
}

interface ContextExpansion {
  fromStart: number;
  fromEnd: number;
}

interface ContextGapAction {
  edge: "start" | "end";
  arrow: "↑" | "↓";
}

const CONTEXT_EXPANSION_STEP = 20;

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

const controlledSides = ["left", "right"] as const;

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
const hasStandardPatchPayload = computed(
  () => props.diff?.patch_schema_version === 1 && Array.isArray(props.diff?.patches),
);
const selectedStandardPatch = computed(
  () =>
    (hasStandardPatchPayload.value ? props.diff?.patches : [])?.find(
      (patch) => patch.filename === selectedFilePath.value,
    ) ?? null,
);

function pairHunkLines(lines: PatchLine[], hunkKey: string): ControlledDiffRow[] {
  const rows: ControlledDiffRow[] = [];
  let deletions: PatchLine[] = [];
  let additions: PatchLine[] = [];
  let rowIndex = 0;
  let previousKind: PatchLine["kind"] | null = null;

  const appendRow = (left: PatchLine | null, right: PatchLine | null) => {
    rows.push({ key: `${hunkKey}:row:${rowIndex}`, left, right });
    rowIndex += 1;
  };
  const flushChanges = () => {
    const rowCount = Math.max(deletions.length, additions.length);
    for (let index = 0; index < rowCount; index += 1) {
      appendRow(deletions[index] ?? null, additions[index] ?? null);
    }
    deletions = [];
    additions = [];
  };

  for (const line of lines) {
    if (line.kind === "context") {
      flushChanges();
      appendRow(line, line);
    } else if (line.kind === "deletion") {
      if (additions.length > 0) flushChanges();
      deletions.push(line);
    } else if (line.kind === "addition") {
      additions.push(line);
    } else {
      flushChanges();
      if (previousKind === "deletion") appendRow(line, null);
      else if (previousKind === "addition") appendRow(null, line);
      else appendRow(line, line);
    }
    previousKind = line.kind;
  }
  flushChanges();
  return rows;
}

const loadedFileContext = ref<LoadedFileContext | null>(null);
const expandedContextGaps = ref<Map<string, ContextExpansion>>(new Map());
const contextLoading = ref(false);
const contextError = ref("");
let contextRequestSequence = 0;

const contextIdentity = computed(() =>
  [
    props.platform ?? "",
    props.owner ?? "",
    props.repo ?? "",
    selectedStandardPatch.value?.old_path ?? "",
    selectedStandardPatch.value?.new_path ?? "",
    props.baseSha ?? "",
    props.headSha ?? "",
  ].join("\0"),
);

function splitFileLines(content: string): string[] {
  const lines = content.split("\n");
  if (content.endsWith("\n")) lines.pop();
  return lines;
}

function gapBeforeHunk(index: number): ControlledContextGap | null {
  const hunks = selectedStandardPatch.value?.hunks ?? [];
  const hunk = hunks[index];
  if (!hunk) return null;
  const previous = hunks[index - 1];
  const oldStart = Math.max(1, previous ? previous.old_start + previous.old_count : 1);
  const newStart = Math.max(1, previous ? previous.new_start + previous.new_count : 1);
  const oldEnd = Math.max(0, hunk.old_start - 1);
  const newEnd = Math.max(0, hunk.new_start - 1);
  if (oldEnd < oldStart && newEnd < newStart) return null;
  return {
    key: `${selectedStandardPatch.value?.filename ?? "file"}:gap:${index}`,
    oldStart,
    oldEnd,
    newStart,
    newEnd,
    direction: index === 0 ? "up" : "both",
  };
}

const trailingContextGap = computed<ControlledContextGap | null>(() => {
  const payload = loadedFileContext.value;
  const hunks = selectedStandardPatch.value?.hunks ?? [];
  const last = hunks.at(-1);
  if (!payload || payload.identity !== contextIdentity.value || !last) return null;
  const oldStart = Math.max(1, last.old_start + last.old_count);
  const newStart = Math.max(1, last.new_start + last.new_count);
  const oldEnd = payload.baseLines.length;
  const newEnd = payload.headLines.length;
  if (oldEnd < oldStart && newEnd < newStart) return null;
  return {
    key: `${selectedStandardPatch.value?.filename ?? "file"}:gap:trailing`,
    oldStart,
    oldEnd,
    newStart,
    newEnd,
    direction: "down",
  };
});

const controlledHunks = computed<ControlledDiffHunk[]>(() =>
  (selectedStandardPatch.value?.hunks ?? []).map((hunk, index) => {
    const key = `${selectedStandardPatch.value?.filename ?? "file"}:hunk:${index}`;
    return { key, hunk, rows: pairHunkLines(hunk.lines, key), gapBefore: gapBeforeHunk(index) };
  }),
);

const availableContextGaps = computed(() => [
  ...controlledHunks.value.flatMap((hunk) => (hunk.gapBefore ? [hunk.gapBefore] : [])),
  ...(trailingContextGap.value ? [trailingContextGap.value] : []),
]);
const hasExpandedContext = computed(() => expandedContextGaps.value.size > 0);
const canLoadContext = computed(
  () =>
    Boolean(props.platform && props.owner && props.repo) &&
    Boolean(
      (selectedStandardPatch.value?.old_path && props.baseSha) ||
      (selectedStandardPatch.value?.new_path && props.headSha),
    ),
);
const canExpandContext = computed(
  () =>
    selectedStandardPatch.value?.content_kind === "text" &&
    controlledHunks.value.length > 0 &&
    canLoadContext.value &&
    (loadedFileContext.value === null ||
      availableContextGaps.value.some((gap) => contextGapRowCount(gap) > 0)),
);

function contextGapActions(gap: ControlledContextGap): ContextGapAction[] {
  if (gap.direction === "up") return [{ edge: "end", arrow: "↑" }];
  if (gap.direction === "down") return [{ edge: "start", arrow: "↓" }];
  return [
    { edge: "end", arrow: "↑" },
    { edge: "start", arrow: "↓" },
  ];
}

function contextGapLabel(gap: ControlledContextGap, edge: ContextGapAction["edge"]): string {
  if (gap.direction === "both") {
    return `向${edge === "start" ? "下" : "上"}展开未变更上下文（20 行）`;
  }
  return `展开${edge === "start" ? "下方" : "上方"}未变更上下文（20 行）`;
}

function contextGapRowCount(gap: ControlledContextGap): number {
  const oldCount = Math.max(0, gap.oldEnd - gap.oldStart + 1);
  const newCount = Math.max(0, gap.newEnd - gap.newStart + 1);
  return Math.max(oldCount, newCount);
}

function isContextGapExpanded(gap: ControlledContextGap): boolean {
  const expansion = expandedContextGaps.value.get(gap.key);
  return Boolean(expansion && expansion.fromStart + expansion.fromEnd >= contextGapRowCount(gap));
}

function contextRows(gap: ControlledContextGap): ControlledDiffRow[] {
  const payload = loadedFileContext.value;
  if (!payload || payload.identity !== contextIdentity.value) return [];
  const oldCount = Math.max(0, gap.oldEnd - gap.oldStart + 1);
  const newCount = Math.max(0, gap.newEnd - gap.newStart + 1);
  const rowCount = Math.max(oldCount, newCount);
  return Array.from({ length: rowCount }, (_, index) => {
    const oldLine = index < oldCount ? gap.oldStart + index : null;
    const newLine = index < newCount ? gap.newStart + index : null;
    return {
      key: `${gap.key}:context:${index}`,
      left:
        oldLine === null
          ? null
          : {
              kind: "context",
              content: payload.baseLines[oldLine - 1] ?? "",
              old_line: oldLine,
              new_line: newLine,
            },
      right:
        newLine === null
          ? null
          : {
              kind: "context",
              content: payload.headLines[newLine - 1] ?? "",
              old_line: oldLine,
              new_line: newLine,
            },
    };
  });
}

function contextRowsFromStart(gap: ControlledContextGap): ControlledDiffRow[] {
  const rows = contextRows(gap);
  const expansion = expandedContextGaps.value.get(gap.key);
  if (!expansion) return [];
  return rows.slice(0, Math.min(expansion.fromStart, rows.length - expansion.fromEnd));
}

function contextRowsFromEnd(gap: ControlledContextGap): ControlledDiffRow[] {
  const rows = contextRows(gap);
  const expansion = expandedContextGaps.value.get(gap.key);
  if (!expansion) return [];
  const start = Math.max(expansion.fromStart, rows.length - expansion.fromEnd);
  return rows.slice(start);
}

async function loadSelectedFileContext(): Promise<boolean> {
  const patch = selectedStandardPatch.value;
  const identity = contextIdentity.value;
  if (loadedFileContext.value?.identity === identity) return true;
  if (!patch || !props.platform || !props.owner || !props.repo || !canLoadContext.value) {
    contextError.value = "缺少该 PR 的 base/head revision，无法展开上下文";
    return false;
  }

  const requestSequence = ++contextRequestSequence;
  contextLoading.value = true;
  contextError.value = "";
  try {
    const [base, head] = await Promise.all([
      patch.old_path && props.baseSha
        ? prFileContent(props.platform, props.owner, props.repo, patch.old_path, props.baseSha)
        : Promise.resolve(null),
      patch.new_path && props.headSha
        ? prFileContent(props.platform, props.owner, props.repo, patch.new_path, props.headSha)
        : Promise.resolve(null),
    ]);
    if (requestSequence !== contextRequestSequence || identity !== contextIdentity.value)
      return false;
    if (base?.truncated || head?.truncated) {
      contextError.value = "文件过大，无法展开完整上下文";
      return false;
    }
    if (base?.binary || head?.binary) {
      contextError.value = "二进制文件不支持展开文本上下文";
      return false;
    }
    loadedFileContext.value = {
      identity,
      baseLines: base ? splitFileLines(base.content) : [],
      headLines: head ? splitFileLines(head.content) : [],
    };
    return true;
  } catch (error) {
    if (requestSequence === contextRequestSequence && identity === contextIdentity.value) {
      contextError.value = error instanceof Error ? error.message : String(error);
    }
    return false;
  } finally {
    if (requestSequence === contextRequestSequence) contextLoading.value = false;
  }
}

async function expandContextGap(
  gap: ControlledContextGap,
  edge: ContextGapAction["edge"],
): Promise<void> {
  if (isContextGapExpanded(gap)) return;
  if (!(await loadSelectedFileContext())) return;
  const rowCount = contextGapRowCount(gap);
  const current = expandedContextGaps.value.get(gap.key) ?? { fromStart: 0, fromEnd: 0 };
  const remaining = Math.max(0, rowCount - current.fromStart - current.fromEnd);
  const amount = Math.min(CONTEXT_EXPANSION_STEP, remaining);
  const next = { ...current };
  if (edge === "start") next.fromStart += amount;
  else next.fromEnd += amount;
  expandedContextGaps.value = new Map(expandedContextGaps.value).set(gap.key, next);
}

async function expandAllContext(): Promise<void> {
  if (!(await loadSelectedFileContext())) return;
  expandedContextGaps.value = new Map(
    availableContextGaps.value.map((gap) => [
      gap.key,
      { fromStart: contextGapRowCount(gap), fromEnd: 0 },
    ]),
  );
}

function collapseAllContext(): void {
  expandedContextGaps.value = new Map();
}

const hasControlledPatch = computed(() => selectedStandardPatch.value !== null);
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

const hasDiffContent = computed(() => hasControlledPatch.value || Boolean(diffHtml.value));

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

const SIDE_DIFF_SELECTOR = ".d2h-file-side-diff, .controlled-file-side-diff";

function visibleSideDiffScrollers(): HTMLElement[] {
  return Array.from(
    containerRef.value?.querySelectorAll<HTMLElement>(SIDE_DIFF_SELECTOR) ?? [],
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
  if (!(source instanceof HTMLElement) || !source.matches(SIDE_DIFF_SELECTOR)) return;
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
  const source = eventTarget?.closest<HTMLElement>(SIDE_DIFF_SELECTOR) ?? null;
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
  () => props.diff,
  (next, previous) => {
    if (next !== previous) selectedFilePath.value = firstFilePath(fileTree.value);
  },
);

watch(
  contextIdentity,
  () => {
    contextRequestSequence += 1;
    loadedFileContext.value = null;
    expandedContextGaps.value = new Map();
    contextLoading.value = false;
    contextError.value = "";
  },
  { immediate: true },
);

watch(
  [diffHtml, selectedFilePath, selectedStandardPatch],
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
  let element: HTMLElement | null =
    node.nodeType === Node.ELEMENT_NODE ? (node as HTMLElement) : node.parentElement;
  while (element) {
    if (
      element.classList.contains("controlled-file-wrapper") ||
      element.classList.contains("d2h-file-wrapper") ||
      element.classList.contains("d2h-wrapper")
    ) {
      return element;
    }
    element = element.parentElement;
  }
  return null;
}

function getControlledSelectionRange(
  range: Range,
  file: HTMLElement,
): { path: string; startLine: number; endLine: number; side: "left" | "right" } | null {
  const path = file.dataset.filePath ?? "";
  if (!path) return null;

  const selectedLines = Array.from(
    file.querySelectorAll<HTMLElement>(".controlled-line[data-line][data-side]"),
  ).filter((line) => range.intersectsNode(line));
  const selectedSides = new Set(selectedLines.map((line) => line.dataset.side));
  if (selectedLines.length === 0 || selectedSides.size !== 1) return null;

  const side = selectedLines[0].dataset.side;
  if (side !== "left" && side !== "right") return null;
  const lines = selectedLines
    .map((line) => Number.parseInt(line.dataset.line ?? "", 10))
    .filter((line) => Number.isFinite(line) && line > 0);
  if (lines.length === 0) return null;

  return {
    path,
    startLine: Math.min(...lines),
    endLine: Math.max(...lines),
    side,
  };
}

function getLegacySelectionRange(
  range: Range,
  file: HTMLElement,
): { path: string; startLine: number; endLine: number; side: "left" | "right" } | null {
  const fileNameElement =
    file.querySelector(".d2h-file-name") ||
    file.querySelector(".d2h-file-name-wrapper .d2h-file-name");
  const path = fileNameElement?.textContent?.trim() || "";
  if (!path) return null;

  const lines: number[] = [];
  let side: "left" | "right" | null = null;
  file.querySelectorAll("tr").forEach((row) => {
    if (!range.intersectsNode(row)) return;
    row.querySelectorAll(".d2h-code-side-linenumber, .d2h-code-linenumber").forEach((element) => {
      const line = Number.parseInt((element as HTMLElement).textContent || "0", 10);
      if (!line) return;
      lines.push(line);
      if (side) return;

      const scroller = (element as HTMLElement).closest<HTMLElement>(".d2h-file-side-diff");
      if (!scroller) return;
      const sideDiffs = scroller.parentElement?.querySelectorAll(".d2h-file-side-diff");
      side = sideDiffs && sideDiffs.length > 1 && sideDiffs[0] === scroller ? "left" : "right";
    });
  });

  if (lines.length === 0 || !side) return null;
  return { path, startLine: Math.min(...lines), endLine: Math.max(...lines), side };
}

function getSelectionRange(): {
  path: string;
  startLine: number;
  endLine: number;
  side: "left" | "right";
} | null {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || !selection.toString().trim()) return null;

  const range = selection.getRangeAt(0);
  const startFile = getFileFromNode(range.startContainer);
  const endFile = getFileFromNode(range.endContainer);
  if (!startFile || startFile !== endFile) return null;

  return startFile.classList.contains("controlled-file-wrapper")
    ? getControlledSelectionRange(range, startFile)
    : getLegacySelectionRange(range, startFile);
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
      v-if="hasDiffContent"
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
          <div v-if="hasControlledPatch && canExpandContext" class="context-toolbar-actions">
            <button
              v-if="hasExpandedContext"
              class="context-toolbar-button"
              type="button"
              :disabled="contextLoading"
              @click="collapseAllContext"
            >
              收起全部上下文
            </button>
            <button
              v-else
              class="context-toolbar-button"
              type="button"
              :disabled="contextLoading"
              @click="expandAllContext"
            >
              {{ contextLoading ? "加载上下文中..." : "展开全部上下文" }}
            </button>
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
          <div ref="containerRef" class="diff2html-container">
            <article
              v-if="selectedStandardPatch"
              class="controlled-file-wrapper"
              :data-file-path="selectedStandardPatch.filename"
            >
              <header class="controlled-file-header">
                <span class="controlled-file-paths">
                  {{ selectedStandardPatch.old_path ?? "/dev/null" }}
                  <span aria-hidden="true">→</span>
                  {{ selectedStandardPatch.new_path ?? "/dev/null" }}
                </span>
                <span class="controlled-file-summary">
                  <span class="additions">+{{ selectedStandardPatch.additions }}</span>
                  <span class="deletions">-{{ selectedStandardPatch.deletions }}</span>
                </span>
              </header>
              <p v-if="contextError" class="context-load-error" role="alert">
                {{ contextError }}
              </p>

              <div
                v-if="selectedStandardPatch.content_kind === 'text' && controlledHunks.length > 0"
                class="controlled-side-by-side"
              >
                <div
                  v-for="side in controlledSides"
                  :key="side"
                  class="controlled-file-side-diff"
                  :class="`controlled-side-${side}`"
                  :aria-label="side === 'left' ? '变更前代码' : '变更后代码'"
                >
                  <div class="controlled-side-content">
                    <template
                      v-for="controlledHunk in controlledHunks"
                      :key="`${controlledHunk.key}:${side}`"
                    >
                      <template v-if="controlledHunk.gapBefore">
                        <div
                          v-for="row in contextRowsFromStart(controlledHunk.gapBefore)"
                          :key="`${row.key}:${side}`"
                          class="controlled-line controlled-context-line"
                          :data-side="side"
                          :data-line="side === 'left' ? row.left?.old_line : row.right?.new_line"
                        >
                          <span class="controlled-line-number" aria-hidden="true">
                            {{ side === "left" ? row.left?.old_line : row.right?.new_line }}
                          </span>
                          <span class="controlled-line-marker" aria-hidden="true"> </span>
                          <code class="controlled-code">{{
                            (side === "left" ? row.left : row.right)?.content ?? ""
                          }}</code>
                        </div>
                      </template>
                      <section class="controlled-hunk">
                        <div
                          class="controlled-hunk-header"
                          :aria-label="side === 'left' ? controlledHunk.hunk.header : undefined"
                          :aria-hidden="side === 'right' ? 'true' : undefined"
                        >
                          <template
                            v-if="
                              controlledHunk.gapBefore &&
                              !isContextGapExpanded(controlledHunk.gapBefore)
                            "
                          >
                            <div
                              v-if="side === 'left'"
                              class="context-gap-controls"
                              :class="{
                                'context-gap-controls-both':
                                  contextGapActions(controlledHunk.gapBefore).length > 1,
                              }"
                            >
                              <button
                                v-for="action in contextGapActions(controlledHunk.gapBefore)"
                                :key="action.edge"
                                class="context-gap-button"
                                type="button"
                                :disabled="contextLoading"
                                :aria-label="contextGapLabel(controlledHunk.gapBefore, action.edge)"
                                @click="expandContextGap(controlledHunk.gapBefore, action.edge)"
                              >
                                <span aria-hidden="true">{{ action.arrow }}</span>
                              </button>
                            </div>
                            <span
                              v-else
                              class="context-gap-placeholder"
                              :class="{
                                'context-gap-placeholder-both':
                                  contextGapActions(controlledHunk.gapBefore).length > 1,
                              }"
                              aria-hidden="true"
                            />
                          </template>
                          <span v-else class="controlled-hunk-gutter" aria-hidden="true" />
                          <span v-if="side === 'left'" class="controlled-hunk-header-text">
                            {{ controlledHunk.hunk.header }}
                          </span>
                        </div>
                        <template v-if="controlledHunk.gapBefore">
                          <div
                            v-for="row in contextRowsFromEnd(controlledHunk.gapBefore)"
                            :key="`${row.key}:${side}`"
                            class="controlled-line controlled-context-line"
                            :data-side="side"
                            :data-line="side === 'left' ? row.left?.old_line : row.right?.new_line"
                          >
                            <span class="controlled-line-number" aria-hidden="true">
                              {{ side === "left" ? row.left?.old_line : row.right?.new_line }}
                            </span>
                            <span class="controlled-line-marker" aria-hidden="true"> </span>
                            <code class="controlled-code">{{
                              (side === "left" ? row.left : row.right)?.content ?? ""
                            }}</code>
                          </div>
                        </template>
                        <div
                          v-for="row in controlledHunk.rows"
                          :key="`${row.key}:${side}`"
                          class="controlled-line"
                          :class="`controlled-line-${(side === 'left' ? row.left : row.right)?.kind ?? 'empty'}`"
                          :data-side="
                            (side === 'left' ? row.left?.old_line : row.right?.new_line)
                              ? side
                              : undefined
                          "
                          :data-line="side === 'left' ? row.left?.old_line : row.right?.new_line"
                        >
                          <span class="controlled-line-number" aria-hidden="true">
                            {{ side === "left" ? row.left?.old_line : row.right?.new_line }}
                          </span>
                          <span class="controlled-line-marker" aria-hidden="true">
                            {{
                              (side === "left" ? row.left : row.right)?.kind === "addition"
                                ? "+"
                                : (side === "left" ? row.left : row.right)?.kind === "deletion"
                                  ? "−"
                                  : " "
                            }}
                          </span>
                          <code class="controlled-code">{{
                            (side === "left" ? row.left : row.right)?.content ?? ""
                          }}</code>
                        </div>
                      </section>
                    </template>
                    <template v-if="trailingContextGap">
                      <div
                        v-for="row in contextRowsFromStart(trailingContextGap)"
                        :key="`${row.key}:${side}`"
                        class="controlled-line controlled-context-line"
                        :data-side="side"
                        :data-line="side === 'left' ? row.left?.old_line : row.right?.new_line"
                      >
                        <span class="controlled-line-number" aria-hidden="true">
                          {{ side === "left" ? row.left?.old_line : row.right?.new_line }}
                        </span>
                        <span class="controlled-line-marker" aria-hidden="true"> </span>
                        <code class="controlled-code">{{
                          (side === "left" ? row.left : row.right)?.content ?? ""
                        }}</code>
                      </div>
                      <div
                        v-if="!isContextGapExpanded(trailingContextGap)"
                        class="controlled-context-gap controlled-context-gap-down"
                      >
                        <div v-if="side === 'left'" class="context-gap-controls">
                          <button
                            v-for="action in contextGapActions(trailingContextGap)"
                            :key="action.edge"
                            class="context-gap-button"
                            type="button"
                            :disabled="contextLoading"
                            :aria-label="contextGapLabel(trailingContextGap, action.edge)"
                            @click="expandContextGap(trailingContextGap, action.edge)"
                          >
                            <span aria-hidden="true">{{ action.arrow }}</span>
                          </button>
                        </div>
                        <span v-else class="context-gap-placeholder" aria-hidden="true" />
                      </div>
                      <div
                        v-for="row in contextRowsFromEnd(trailingContextGap)"
                        :key="`${row.key}:${side}`"
                        class="controlled-line controlled-context-line"
                        :data-side="side"
                        :data-line="side === 'left' ? row.left?.old_line : row.right?.new_line"
                      >
                        <span class="controlled-line-number" aria-hidden="true">
                          {{ side === "left" ? row.left?.old_line : row.right?.new_line }}
                        </span>
                        <span class="controlled-line-marker" aria-hidden="true"> </span>
                        <code class="controlled-code">{{
                          (side === "left" ? row.left : row.right)?.content ?? ""
                        }}</code>
                      </div>
                    </template>
                  </div>
                </div>
              </div>

              <div v-else class="controlled-file-message" role="status">
                {{ selectedStandardPatch.message ?? "该文件没有可展示的文本 Diff" }}
              </div>
            </article>
            <div v-else class="legacy-diff" v-html="diffHtml" />
          </div>
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

.controlled-file-wrapper {
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface);
}

.controlled-file-header {
  display: flex;
  min-height: 34px;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface-hover);
  font-family: var(--font-mono);
  font-size: 11px;
}

.controlled-file-paths {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.controlled-file-paths > span {
  padding: 0 var(--space-1);
  color: var(--color-text-tertiary);
}

.controlled-file-summary {
  display: inline-flex;
  flex-shrink: 0;
  gap: var(--space-2);
}

.controlled-side-by-side {
  display: grid;
  min-width: 0;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.controlled-file-side-diff {
  position: relative;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  overscroll-behavior-x: contain;
  scrollbar-width: none;
}

.controlled-file-side-diff::-webkit-scrollbar {
  display: none;
}

.controlled-file-side-diff + .controlled-file-side-diff {
  border-left: 1px solid var(--color-border);
}

.controlled-side-content {
  width: max-content;
  min-width: 100%;
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 20px;
}

.context-toolbar-actions {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  gap: var(--space-1);
}

.context-toolbar-button {
  min-height: 28px;
  padding: 0 var(--space-2);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: var(--color-surface);
  color: var(--color-text-secondary);
  font-size: 11px;
  white-space: nowrap;
}

.context-toolbar-button:hover:not(:disabled) {
  border-color: var(--color-primary);
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.context-toolbar-button:disabled {
  cursor: wait;
  opacity: 0.65;
}

.context-load-error {
  margin: 0;
  padding: 6px var(--space-3);
  border-bottom: 1px solid var(--color-border);
  background: color-mix(in srgb, var(--color-danger-light, #fff1f0) 70%, var(--color-surface));
  color: var(--color-danger, #cf222e);
  font-size: 11px;
}

.controlled-context-gap {
  display: grid;
  width: max-content;
  min-width: 100%;
  min-height: 20px;
  grid-template-columns: 52px 18px minmax(0, 1fr);
  align-items: stretch;
  background: var(--color-surface-hover);
  color: var(--color-text-tertiary);
}

.context-gap-controls,
.context-gap-placeholder {
  position: sticky;
  left: 0;
  z-index: 3;
  display: flex;
  width: 52px;
  min-height: 20px;
  grid-column: 1;
  align-items: stretch;
  border-right: 1px solid var(--color-border-light);
  background: var(--color-surface-hover);
}

.context-gap-controls {
  flex-direction: column;
  background: var(--color-primary-border);
}

.context-gap-button {
  display: inline-flex;
  width: 100%;
  min-height: 20px;
  flex: 1;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--color-primary);
  font-family: var(--font-sans);
  font-size: 12px;
  font-weight: 700;
  line-height: 1;
  cursor: pointer;
}

.context-gap-button + .context-gap-button {
  border-top: 1px solid var(--color-border-light);
}

.context-gap-button:hover,
.context-gap-button:focus-visible {
  background: var(--color-primary);
  color: var(--color-surface);
}

.context-gap-button:active {
  background: var(--color-primary-hover);
  color: var(--color-surface);
}

.context-gap-button:focus-visible {
  z-index: 4;
  outline: 2px solid var(--color-focus);
  outline-offset: -2px;
}

.context-gap-button:disabled {
  cursor: wait;
  opacity: 0.65;
}

.controlled-hunk + .controlled-hunk {
  border-top: 1px solid var(--color-border);
}

.controlled-hunk-header {
  display: grid;
  width: max-content;
  min-width: 100%;
  min-height: 20px;
  grid-template-columns: 52px 18px minmax(0, 1fr);
  align-items: stretch;
  overflow: hidden;
  background: var(--diff-hunk-bg, var(--color-primary-light));
  color: var(--color-text-secondary);
  white-space: nowrap;
}

.controlled-hunk-header .context-gap-controls,
.controlled-hunk-header .context-gap-placeholder,
.controlled-hunk-gutter {
  position: sticky;
  left: 0;
  z-index: 3;
  width: 52px;
  min-height: 20px;
  grid-column: 1;
  border-right: 1px solid var(--color-border-light);
}

.controlled-hunk-header .context-gap-controls {
  background: var(--color-primary-border);
}

.controlled-hunk-header .context-gap-placeholder,
.controlled-hunk-gutter {
  background: inherit;
}

.controlled-hunk-header .context-gap-placeholder-both {
  min-height: 40px;
}

.controlled-hunk-header-text {
  min-width: max-content;
  grid-column: 2 / 4;
  align-self: center;
  padding: 0 10px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.controlled-line {
  display: grid;
  width: max-content;
  min-width: 100%;
  min-height: 20px;
  grid-template-columns: 52px 18px minmax(0, 1fr);
  background: var(--color-surface);
}

.controlled-line-number {
  position: sticky;
  z-index: 2;
  left: 0;
  padding: 0 4px;
  border-right: 1px solid var(--color-border-light);
  background: var(--color-surface-hover);
  color: var(--color-text-tertiary);
  text-align: center;
  user-select: none;
}

.controlled-line-marker {
  color: var(--color-text-tertiary);
  text-align: center;
  user-select: none;
}

.controlled-code {
  min-width: max-content;
  padding-right: var(--space-4);
  color: var(--color-text);
  font: inherit;
  white-space: pre;
}

.controlled-line-addition {
  background: var(--diff-add-bg);
}

.controlled-line-deletion {
  background: var(--diff-remove-bg);
}

.controlled-line-addition .controlled-line-number {
  background: var(--diff-add-line);
  color: var(--color-text);
}

.controlled-line-deletion .controlled-line-number {
  background: var(--diff-remove-line);
  color: var(--color-text);
}

.controlled-line-no_newline {
  color: var(--color-text-tertiary);
  font-style: italic;
}

.controlled-line-empty {
  background: color-mix(in srgb, var(--color-surface-hover) 65%, var(--color-surface));
}

.controlled-file-message {
  display: grid;
  min-height: 180px;
  place-items: center;
  padding: var(--space-6);
  color: var(--color-text-secondary);
  text-align: center;
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
