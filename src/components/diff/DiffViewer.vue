<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted, nextTick } from "vue";
import { html } from "diff2html";
import "diff2html/bundles/css/diff2html.min.css";
import type { DiffResult } from "@/types";

const props = defineProps<{
  diff: DiffResult | null;
}>();

const emit = defineEmits<{
  addComment: [path: string, startLine: number, endLine: number, side: "left" | "right", body: string];
}>();

const containerRef = ref<HTMLElement | null>(null);

const diffHtml = computed(() => {
  if (!props.diff?.diff) return "";
  return html(props.diff.diff, {
    drawFileList: true,
    matching: "lines",
    outputFormat: "side-by-side",
    renderNothingWhenEmpty: false,
  });
});

// ── Quick comment popup state ──
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
  logic: "逻辑类", security: "安全类", performance: "性能类", style: "代码风格类", log: "日志类",
};

// ── Get file wrapper from a DOM node ──
function getFileFromNode(node: Node): HTMLElement | null {
  let el: HTMLElement | null =
    node.nodeType === Node.ELEMENT_NODE
      ? (node as HTMLElement)
      : (node.parentElement as HTMLElement);
  while (el) {
    const cls = el.classList;
    if (cls.contains("d2h-file-wrapper") || cls.contains("d2h-wrapper")) return el;
    // In side-by-side mode, file names may be inside .d2h-file-name-wrapper
    if (cls.contains("d2h-files-diff") || cls.contains("d2h-file-side-diff")) {
      // Walk up to find the .d2h-file-wrapper parent
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

// ── Extract file/line range from text selection ──
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

  // Walk up from selection endpoints to find the file wrapper
  const startFile = getFileFromNode(range.startContainer);
  const endFile = getFileFromNode(range.endContainer);
  if (!startFile || startFile !== endFile) return null;

  // Find file path
  const fileNameEl = startFile.querySelector(".d2h-file-name") || startFile.querySelector(".d2h-file-name-wrapper .d2h-file-name");
  const filePath = fileNameEl?.textContent?.trim() || "";
  if (!filePath) return null;

  // Collect line numbers from all rows that intersect the selection range
  const lines: number[] = [];
  let side: "left" | "right" | null = null;

  startFile.querySelectorAll("tr").forEach((row) => {
    if (!range.intersectsNode(row)) return;
    // In side-by-side mode, line numbers use d2h-code-side-linenumber
    // In unified mode, they use d2h-code-linenumber
    const lnEls = row.querySelectorAll(".d2h-code-side-linenumber, .d2h-code-linenumber");
    lnEls.forEach((el) => {
      const num = parseInt((el as HTMLElement).textContent || "0", 10);
      if (!num) return;
      lines.push(num);
      if (!side) {
        // Determine side: walk up to d2h-file-side-diff and check order
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

// ── Handlers ──

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
  const main = categoryLabels[quickCategory.value] || quickCategory.value;
  const sub = quickSubCategory.value ? `-${quickSubCategory.value}` : "";
  emit(
    "addComment",
    quickComment.value.path,
    quickComment.value.startLine,
    quickComment.value.endLine,
    quickComment.value.side,
    `【${main}${sub}】${quickBody.value.trim()}`,
  );
  quickSubmitting.value = true;
  await new Promise((r) => setTimeout(r, 200));
  quickComment.value = null;
  quickBody.value = "";
  quickSubmitting.value = false;
}

function onSubCategoryChange() {
  if (quickSubCategory.value) {
    const main = categoryLabels[quickCategory.value] || quickCategory.value;
    quickBody.value = '【' + main + '-' + quickSubCategory.value + '】';
  }
}

function handleQuickKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) submitQuickComment();
  if (e.key === "Escape") quickComment.value = null;
}

// Auto-focus textarea when popup appears
watch(quickComment, async (val) => {
  if (val) {
    await nextTick();
    (document.querySelector(".quick-comment-textarea") as HTMLTextAreaElement)?.focus();
  }
});

onMounted(() => {
  document.addEventListener("contextmenu", handleContextMenu, true);
  document.addEventListener("click", handleDocClick);
});

onUnmounted(() => {
  document.removeEventListener("contextmenu", handleContextMenu, true);
  document.removeEventListener("click", handleDocClick);
});
</script>

<template>
  <div class="diff-viewer-wrapper">
    <div
      v-if="diffHtml"
      ref="containerRef"
      class="diff2html-container"
      v-html="diffHtml"
    />

    <div v-else class="diff-empty">暂无 diff 数据</div>

    <!-- Quick comment popup -->
    <Teleport to="body">
      <div
        v-if="quickComment"
        class="quick-comment-popup"
        :style="{ left: quickComment.x + 'px', top: (quickComment.y - 8) + 'px' }"
        @click.stop
        @keydown="handleQuickKeydown"
      >
        <div class="popup-header">
          <span class="file-ref">
            {{ quickComment.path.split('/').pop() }}:L{{ quickComment.startLine
            }}{{ quickComment.endLine !== quickComment.startLine ? '-L' + quickComment.endLine : '' }}
          </span>
          <button class="close-btn" @click="quickComment = null">×</button>
        </div>
        <pre v-if="quickComment.selectedText" class="selected-code">{{ quickComment.selectedText }}</pre>

        <div class="popup-category">
          <select v-model="quickCategory" class="category-select" @change="quickSubCategory = ''">
            <option v-for="(label, key) in categoryLabels" :key="key" :value="key">{{ label }}</option>
          </select>
          <select
            v-if="categories[quickCategory]"
            v-model="quickSubCategory"
            class="category-select"
            @change="onSubCategoryChange"
          >
            <option value="">-- 二级分类 --</option>
            <option v-for="sub in categories[quickCategory]" :key="sub" :value="sub">{{ sub }}</option>
          </select>
        </div>

        <textarea
          v-model="quickBody"
          class="quick-comment-textarea"
          placeholder="输入评审意见... (⌘+Enter 提交, Esc 取消)"
          rows="3"
        />
        <div class="popup-actions">
          <button class="cancel-btn" @click="quickComment = null">取消</button>
          <button
            class="submit-btn"
            :disabled="!quickBody.trim() || quickSubmitting"
            @click="submitQuickComment"
          >
            {{ quickSubmitting ? '提交中...' : '提交' }}
          </button>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.diff-viewer-wrapper {
  position: relative;
}

.diff2html-container :deep(.d2h-file-wrapper) {
  border: 1px solid var(--color-border);
  border-radius: 8px;
  margin-bottom: 12px;
  overflow: hidden;
}

.diff2html-container :deep(.d2h-file-header) {
  background: #f6f8fa;
  border-bottom: 1px solid var(--color-border);
}

.diff2html-container :deep(.d2h-ins) {
  background: var(--diff-add-bg);
}

.diff2html-container :deep(.d2h-del) {
  background: var(--diff-remove-bg);
}

.diff-empty {
  padding: 40px;
  text-align: center;
  color: var(--color-text-secondary);
}

/* ── Quick comment popup ── */
.quick-comment-popup {
  position: fixed;
  z-index: 10000;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.15);
  padding: 10px 12px;
  min-width: 300px;
  max-width: 420px;
}

.popup-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.file-ref {
  font-size: 11px;
  font-family: monospace;
  color: var(--color-text-secondary);
  background: #f6f8fa;
  padding: 2px 6px;
  border-radius: 4px;
}

.close-btn {
  border: none;
  background: none;
  font-size: 16px;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: 0 2px;
  line-height: 1;
}

.selected-code {
  margin: 0 0 8px 0;
  padding: 6px 8px;
  background: #f6f8fa;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  font-size: 11px;
  font-family: monospace;
  line-height: 1.4;
  max-height: 120px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--color-text);
}

.quick-comment-textarea {
  width: 100%;
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  font-size: 13px;
  font-family: inherit;
  resize: vertical;
  outline: none;
  min-height: 60px;
  box-sizing: border-box;
}

.quick-comment-textarea:focus {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px rgba(13, 110, 253, 0.15);
}

.popup-category {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}

.category-select {
  padding: 4px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  font-size: 12px;
  color: var(--color-text);
  background: var(--color-surface);
  margin-bottom: 6px;
}

.popup-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 8px;
}

.popup-actions button {
  padding: 5px 14px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
}

.cancel-btn {
  background: none;
  border: 1px solid var(--color-border);
  color: var(--color-text-secondary);
}

.cancel-btn:hover {
  background: #f0f0f0;
}

.submit-btn {
  background: var(--color-primary);
  color: #fff;
  border: none;
}

.submit-btn:hover:not(:disabled) {
  background: var(--color-primary-hover);
}

.submit-btn:disabled {
  opacity: 0.5;
}
</style>
