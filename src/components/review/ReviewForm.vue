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
      placeholder="输入你的评审意见..."
      rows="5"
    />

    <div class="form-actions">
      <button :disabled="submitting || !body.trim()" @click="handleSubmit">
        {{ submitting ? "提交中..." : "提交评审" }}
      </button>
      <span v-if="success" class="success-msg">✓ 评审已提交</span>
      <span v-if="error" class="error-msg">{{ error }}</span>
    </div>
  </div>
</template>

<style scoped>
.review-form {
  margin-top: 24px;
  padding: 20px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
}

h4 {
  margin-bottom: 12px;
}

.event-select {
  display: flex;
  gap: 6px;
  margin-bottom: 12px;
}

.event-select button {
  padding: 6px 14px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: none;
  font-size: 13px;
}

.event-select button.active {
  border-color: var(--color-primary);
  color: var(--color-primary);
  background: #e8f0fe;
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
}

textarea:focus {
  border-color: var(--color-primary);
}

.form-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 12px;
}

.form-actions button {
  padding: 8px 20px;
  background: var(--color-primary);
  color: #fff;
  border: none;
  border-radius: 6px;
  font-size: 14px;
}

.form-actions button:disabled {
  opacity: 0.5;
}

.success-msg { color: var(--color-success); font-size: 13px; }
.error-msg { color: var(--color-danger); font-size: 13px; }
</style>
