<script setup lang="ts">
const props = defineProps<{
  title: string;
  body: string;
  labels: string[];
  error: string;
  submitting: boolean;
}>();

const emit = defineEmits<{
  "update:title": [value: string];
  "update:body": [value: string];
  "update:labels": [value: string[]];
  submit: [];
}>();

function addLabel(label: string) {
  if (label && !props.labels.includes(label)) {
    emit("update:labels", [...props.labels, label]);
  }
}

function removeLabel(label: string) {
  emit(
    "update:labels",
    props.labels.filter((l) => l !== label),
  );
}
</script>

<template>
  <div class="issue-form">
    <div class="field">
      <label>标题</label>
      <input
        class="input"
        :value="title"
        @input="emit('update:title', ($event.target as HTMLInputElement).value)"
        placeholder="Issue 标题"
      />
    </div>

    <div class="field">
      <label>描述</label>
      <textarea
        class="input"
        :value="body"
        @input="emit('update:body', ($event.target as HTMLTextAreaElement).value)"
        placeholder="详细描述..."
        rows="8"
      />
    </div>

    <div class="field">
      <label>标签</label>
      <div class="labels-input">
        <div class="labels-list">
          <span
            v-for="label in labels"
            :key="label"
            class="label-chip"
          >
            {{ label }}
            <button type="button" @click="removeLabel(label)">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
          </span>
        </div>
        <input
          class="input"
          style="border: none; padding: 4px 6px;"
          placeholder="输入标签后回车"
          @keyup.enter="
            addLabel(($event.target as HTMLInputElement).value);
            ($event.target as HTMLInputElement).value = '';
          "
        />
      </div>
    </div>

    <div v-if="error" class="error">{{ error }}</div>

    <div class="form-actions">
      <button class="btn btn-success" :disabled="submitting || !title.trim()" @click="emit('submit')">
        {{ submitting ? "提交中..." : "创建 Issue" }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.issue-form {
  max-width: 640px;
}

.field {
  margin-bottom: var(--space-4);
}

label {
  display: block;
  font-weight: 600;
  margin-bottom: var(--space-1);
  font-size: 13px;
}

.labels-input {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--space-1);
  transition: border-color var(--transition-fast);
}

.labels-input:focus-within {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px var(--color-primary-light);
}

.labels-list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
  margin-bottom: var(--space-1);
}

.label-chip {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 2px 8px;
  background: var(--color-primary-light);
  color: var(--color-primary);
  border-radius: 999px;
  font-size: 12px;
}

.label-chip button {
  border: none;
  background: none;
  color: var(--color-primary);
  cursor: pointer;
  padding: 0;
  line-height: 1;
  display: flex;
  align-items: center;
}

.error {
  padding: var(--space-2);
  background: var(--color-danger-light);
  color: var(--color-danger);
  border-radius: var(--radius-md);
  margin-bottom: var(--space-4);
  font-size: 13px;
  border: 1px solid var(--color-danger-border);
}
</style>
