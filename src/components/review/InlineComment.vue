<script setup lang="ts">
import type { ReviewCommentPosition } from "@/types";

defineProps<{
  comment: ReviewCommentPosition;
}>();
</script>

<template>
  <div class="inline-comment">
    <div class="comment-header">
      <span class="comment-location font-mono">
        {{ comment.path }}:{{ comment.position }}
      </span>
    </div>
    <p class="comment-body">{{ comment.body }}</p>
  </div>
</template>

<style scoped>
.inline-comment {
  padding: var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-left: 3px solid var(--color-primary);
  border-radius: var(--radius-md);
  margin: var(--space-2) 0;
}

.comment-header {
  margin-bottom: var(--space-1);
}

.comment-location {
  font-size: 11px;
  color: var(--color-text-tertiary);
}

.comment-body {
  font-size: 13px;
  line-height: 1.5;
  white-space: pre-wrap;
}
</style>
