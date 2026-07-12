<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted, nextTick } from "vue";
import { html } from "diff2html";
import "diff2html/bundles/css/diff2html.min.css";
import type { DiffResult } from "@/types";
import AppSelect from "@/components/shared/AppSelect.vue";

const props = defineProps<{
  diff: DiffResult | null;
}>();

const emit = defineEmits<{
  addComment: [
    path: string,
    startLine: number,
    endLine: number,
    side: "left" | "right",
    body: string,
  ];
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
    <div v-if="diffHtml" ref="containerRef" class="diff2html-container" v-html="diffHtml" />

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
.diff-viewer-wrapper {
  position: relative;
}

.diff2html-container :deep(.d2h-file-wrapper) {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  margin-bottom: var(--space-3);
  overflow: hidden;
}

.diff2html-container :deep(.d2h-file-header) {
  background: var(--color-surface-hover);
  border-bottom: 1px solid var(--color-border);
}

.diff2html-container :deep(.d2h-ins) {
  background: var(--diff-add-bg);
}

.diff2html-container :deep(.d2h-del) {
  background: var(--diff-remove-bg);
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
