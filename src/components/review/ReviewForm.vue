<script setup lang="ts">
import { ref } from "vue";
import type { Platform, ReviewEvent } from "@/types";
import { reviewSubmit } from "@/api";

const props = defineProps<{
  platform: Platform;
  owner: string;
  repo: string;
  prNumber: number;
}>();

const body = ref("");
const event = ref<ReviewEvent>("comment");
const submitting = ref(false);
const error = ref("");
const success = ref(false);

const events: { value: ReviewEvent; label: string }[] = [
  { value: "comment", label: "评论" },
  { value: "approve", label: "批准" },
  { value: "request_changes", label: "请求修改" },
];

async function handleSubmit() {
  if (!body.value.trim()) return;
  submitting.value = true;
  error.value = "";
  success.value = false;
  try {
    await reviewSubmit(
      props.platform,
      props.owner,
      props.repo,
      props.prNumber,
      body.value,
      event.value,
      [],
    );
    success.value = true;
    body.value = "";
  } catch (e: any) {
    error.value = e?.toString() || "提交失败";
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <div class="review-form">
    <h4>提交评审意见</h4>

    <div class="event-select">
      <button
        v-for="ev in events"
        :key="ev.value"
        :class="{ active: event === ev.value }"
        @click="event = ev.value"
      >
        {{ ev.label }}
      </button>
    </div>

    <textarea
      v-model="body"
      class="input"
      placeholder="输入你的评审意见..."
      rows="5"
    />

    <div class="form-actions">
      <button class="btn btn-primary" :disabled="submitting || !body.trim()" @click="handleSubmit">
        {{ submitting ? "提交中..." : "提交评审" }}
      </button>
      <span v-if="success" class="success-msg">✓ 评审已提交</span>
      <span v-if="error" class="error-msg">{{ error }}</span>
    </div>
  </div>
</template>

<style scoped>
.review-form {
  margin-top: var(--space-6);
  padding: var(--space-5);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
}

h4 {
  margin-bottom: var(--space-3);
  font-size: 15px;
  font-weight: 600;
}

.event-select {
  display: flex;
  gap: var(--space-1);
  margin-bottom: var(--space-3);
}

.event-select button {
  padding: 6px 14px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: none;
  font-size: 13px;
  font-weight: 500;
  transition: all var(--transition-fast);
  color: var(--color-text-secondary);
}

.event-select button.active {
  border-color: var(--color-primary);
  color: var(--color-primary);
  background: var(--color-primary-light);
}

.event-select button:hover:not(.active) {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

textarea {
  width: 100%;
  resize: vertical;
  min-height: 100px;
}

.form-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin-top: var(--space-3);
}

.success-msg { color: var(--color-success); font-size: 13px; font-weight: 500; }
.error-msg { color: var(--color-danger); font-size: 13px; }
</style>
