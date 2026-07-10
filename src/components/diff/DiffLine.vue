<script setup lang="ts">
interface ParsedLine {
  type: "add" | "remove" | "context" | "header";
  content: string;
  oldLine: number | null;
  newLine: number | null;
}

const props = defineProps<{
  line: ParsedLine;
}>();

const emit = defineEmits<{
  comment: [line: ParsedLine];
}>();
</script>

<template>
  <div
    class="diff-line"
    :class="`diff-line-${line.type}`"
  >
    <!-- Hunk header line -->
    <template v-if="line.type === 'header'">
      <span class="line-num old"></span>
      <span class="line-num new"></span>
      <span class="line-content header-content">{{ line.content }}</span>
    </template>

    <!-- Regular diff line -->
    <template v-else>
      <span class="line-num old">{{ line.oldLine ?? "" }}</span>
      <span class="line-num new">{{ line.newLine ?? "" }}</span>
      <span class="line-content">{{ line.content }}</span>
      <button
        v-if="line.type !== 'context'"
        class="comment-btn"
        title="添加行评论"
        @click="emit('comment', line)"
      >
        💬
      </button>
    </template>
  </div>
</template>

<style scoped>
.diff-line {
  display: flex;
  align-items: stretch;
  min-height: 20px;
  position: relative;
}

.diff-line:hover {
  background: rgba(0, 0, 0, 0.03);
}

.line-num {
  width: 50px;
  min-width: 50px;
  text-align: right;
  padding: 0 8px;
  color: var(--color-text-secondary);
  user-select: none;
  border-right: 1px solid var(--color-border);
}

.line-content {
  flex: 1;
  padding: 0 8px;
  white-space: pre-wrap;
  word-break: break-all;
}

/* Add line */
.diff-line-add .line-content {
  background: var(--diff-add-bg);
}
.diff-line-add .line-num.new {
  background: var(--diff-add-line);
}

/* Remove line */
.diff-line-remove .line-content {
  background: var(--diff-remove-bg);
}
.diff-line-remove .line-num.old {
  background: var(--diff-remove-line);
}

/* Header line */
.header-content {
  color: #6a737d;
  background: #f1f8ff;
}

.comment-btn {
  position: absolute;
  right: 4px;
  top: 0;
  border: none;
  background: none;
  font-size: 12px;
  opacity: 0;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 3px;
  transition: opacity 0.15s;
}

.diff-line:hover .comment-btn {
  opacity: 0.6;
}

.diff-line .comment-btn:hover {
  opacity: 1;
  background: #e8e8e8;
}
</style>
