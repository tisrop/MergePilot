<script setup lang="ts">
import { ref } from "vue";

const props = defineProps<{
  path: string;
  position: number;
  endPosition?: number;
}>();

const emit = defineEmits<{
  close: [];
  submit: [body: string];
}>();

const body = ref("");
const submitting = ref(false);

function handleSubmit() {
  if (!body.value.trim()) return;
  emit("submit", body.value.trim());
  body.value = "";
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
    handleSubmit();
  }
  if (e.key === "Escape") {
    emit("close");
  }
}
</script>

<template>
  <div class="inline-comment" @keydown="handleKeydown">
    <div class="comment-header">
      <span class="file-ref">
        {{ path }}:{{ position }}{{ props.endPosition && props.endPosition !== props.position ? '-' + props.endPosition : '' }}
      </span>
      <button class="close-btn" @click="emit('close')" title="关闭 (Esc)">×</button>
    </div>

    <textarea
      v-model="body"
      placeholder="输入行评论... (⌘+Enter 提交, Esc 取消)"
      rows="3"
      autofocus
    />

    <div class="comment-actions">
      <span class="hint">⌘+Enter 提交</span>
      <div class="buttons">
        <button class="cancel" @click="emit('close')">取消</button>
        <button
          class="submit"
          :disabled="!body.trim() || submitting"
          @click="handleSubmit"
        >
          {{ submitting ? "提交中..." : "提交评论" }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.inline-comment {
  padding: 12px;
  background: var(--color-surface);
  border-radius: 8px;
}

.comment-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.file-ref {
  font-size: 12px;
  font-family: monospace;
  color: var(--color-text-secondary);
  background: #f6f8fa;
  padding: 2px 8px;
  border-radius: 4px;
}

.close-btn {
  border: none;
  background: none;
  font-size: 18px;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}

.close-btn:hover {
  color: var(--color-text);
}

textarea {
  width: 100%;
  padding: 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  font-size: 13px;
  font-family: inherit;
  resize: vertical;
  outline: none;
  min-height: 72px;
}

textarea:focus {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px rgba(13, 110, 253, 0.15);
}

.comment-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 8px;
}

.hint {
  font-size: 11px;
  color: var(--color-text-secondary);
}

.buttons {
  display: flex;
  gap: 6px;
}

.buttons button {
  padding: 6px 14px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
}

.cancel {
  background: none;
  border: 1px solid var(--color-border);
  color: var(--color-text-secondary);
}

.cancel:hover {
  background: #f0f0f0;
}

.submit {
  background: var(--color-primary);
  color: #fff;
  border: none;
}

.submit:hover:not(:disabled) {
  background: var(--color-primary-hover);
}

.submit:disabled {
  opacity: 0.5;
}
</style>
