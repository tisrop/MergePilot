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
  <div class="issue-form card">
    <div class="field">
      <label for="issue-title">标题 <span class="required">必填</span></label>
      <input
        id="issue-title"
        class="input"
        :value="title"
        @input="emit('update:title', ($event.target as HTMLInputElement).value)"
        placeholder="Issue 标题"
      />
    </div>

    <div class="field">
      <label for="issue-body">描述</label>
      <textarea
        id="issue-body"
        class="input"
        :value="body"
        @input="emit('update:body', ($event.target as HTMLTextAreaElement).value)"
        placeholder="详细描述..."
        rows="8"
      />
    </div>

    <div class="field">
      <label for="issue-labels">标签</label>
      <div class="labels-input">
        <div class="labels-list">
          <span v-for="label in labels" :key="label" class="label-chip">
            {{ label }}
            <button type="button" @click="removeLabel(label)">
              <svg
                width="12"
                height="12"
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
          </span>
        </div>
        <input
          id="issue-labels"
          class="input"
          style="border: none; padding: 4px 6px"
          placeholder="输入标签后回车"
          @keyup.enter="
            addLabel(($event.target as HTMLInputElement).value);
            ($event.target as HTMLInputElement).value = '';
          "
        />
      </div>
    </div>

    <div v-if="error" class="error" role="alert">{{ error }}</div>

    <div class="form-actions">
      <router-link to="/issue" class="btn">取消</router-link>
      <button
        class="btn btn-success"
        :disabled="submitting || !title.trim()"
        @click="emit('submit')"
      >
        {{ submitting ? "提交中..." : "创建 Issue" }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.issue-form {
  max-width: 720px;
  padding: var(--space-6);
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

.required {
  margin-left: var(--space-1);
  color: var(--color-danger);
  font-size: 10px;
  font-weight: 500;
}

.labels-input {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--space-1);
  transition: border-color var(--transition-fast);
}

.labels-input:focus-within {
  border-color: var(--color-focus);
  box-shadow: var(--shadow-control-focus);
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
  width: 22px;
  height: 22px;
  justify-content: center;
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

.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding-top: var(--space-2);
}
</style>
