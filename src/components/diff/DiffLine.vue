<script setup lang="ts">
defineProps<{
  line: {
    type: "add" | "del" | "context";
    content: string;
    oldNumber?: number | null;
    newNumber?: number | null;
  };
}>();
</script>

<template>
  <div
    class="diff-line"
    :class="{
      'line-add': line.type === 'add',
      'line-del': line.type === 'del',
      'line-context': line.type === 'context',
    }"
  >
    <span class="line-num-old">{{ line.oldNumber ?? '' }}</span>
    <span class="line-num-new">{{ line.newNumber ?? '' }}</span>
    <span class="line-type">{{ line.type === 'add' ? '+' : line.type === 'del' ? '-' : ' ' }}</span>
    <span class="line-content" v-text="line.content" />
  </div>
</template>

<style scoped>
.diff-line {
  display: flex;
  padding: 0 var(--space-3);
  font-size: 12px;
  line-height: 1.6;
  min-height: 22px;
}

.line-add {
  background: var(--diff-add-bg);
}

.line-del {
  background: var(--diff-remove-bg);
}

.line-num-old,
.line-num-new {
  width: 48px;
  text-align: right;
  padding-right: var(--space-2);
  color: var(--color-text-tertiary);
  user-select: none;
  font-size: 11px;
}

.line-type {
  width: 16px;
  text-align: center;
  color: var(--color-text-tertiary);
  user-select: none;
}

.line-content {
  flex: 1;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
