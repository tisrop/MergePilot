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
        :value="title"
        @input="emit('update:title', ($event.target as HTMLInputElement).value)"
        placeholder="Issue 标题"
      />
    </div>

    <div class="field">
      <label>描述</label>
      <textarea
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
            <button @click="removeLabel(label)">×</button>
          </span>
        </div>
        <input
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
      <button :disabled="submitting || !title.trim()" @click="emit('submit')">
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
  margin-bottom: 16px;
}

label {
  display: block;
  font-weight: 600;
  margin-bottom: 6px;
}

input, textarea {
  width: 100%;
  padding: 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  font-size: 14px;
  font-family: inherit;
  outline: none;
}

input:focus, textarea:focus {
  border-color: var(--color-primary);
}

.labels-input {
  border: 1px solid var(--color-border);
  border-radius: 6px;
  padding: 6px;
}

.labels-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-bottom: 4px;
}

.label-chip {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 2px 8px;
  background: #e8f0fe;
  color: var(--color-primary);
  border-radius: 12px;
  font-size: 12px;
}

.label-chip button {
  border: none;
  background: none;
  font-size: 14px;
  color: var(--color-primary);
  cursor: pointer;
  padding: 0;
  line-height: 1;
}

.labels-input input {
  border: none;
  padding: 4px 6px;
  font-size: 13px;
}

.error {
  padding: 10px;
  background: #ffeaea;
  color: var(--color-danger);
  border-radius: 6px;
  margin-bottom: 16px;
  font-size: 13px;
}

.form-actions button {
  padding: 10px 24px;
  background: var(--color-success);
  color: #fff;
  border: none;
  border-radius: 6px;
  font-size: 14px;
}

.form-actions button:disabled {
  opacity: 0.5;
}
</style>
