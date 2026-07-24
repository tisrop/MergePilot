<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { prLabels, prParticipantSuggestions } from "@/api";
import AppMultiSelect from "@/components/shared/AppMultiSelect.vue";
import MarkdownRenderer from "@/components/shared/MarkdownRenderer.vue";
import type {
  Platform,
  PlatformCapabilities,
  PrDetail,
  PrLabel,
  PrMetadataPermissions,
  PrMetadataUpdate,
  User,
} from "@/types";
import { getErrorMessage } from "@/utils/error";

const props = defineProps<{
  detail: PrDetail;
  platform: Platform;
  owner: string;
  repo: string;
  capabilities: PlatformCapabilities | null;
  saving: boolean;
  statusMessage?: string;
  errorMessage?: string;
}>();

const emit = defineEmits<{
  save: [update: PrMetadataUpdate];
}>();

const editing = ref(false);
const title = ref("");
const body = ref("");
const draft = ref(false);
const reviewers = ref<string[]>([]);
const assignees = ref<string[]>([]);
const labels = ref<string[]>([]);
const milestone = ref("");
const validationError = ref("");
const availableParticipants = ref<User[]>([]);
const availableLabels = ref<PrLabel[]>([]);
const participantsLoading = ref(false);
const labelsLoading = ref(false);
const participantsError = ref("");
const labelsError = ref("");
let optionsSequence = 0;

const permissions = computed<PrMetadataPermissions>(() => props.detail.metadata_permissions);
const canUse = (supported: boolean | undefined, permission: boolean | null): boolean =>
  supported === true && permission !== false;

const canEditTitleBody = computed(() =>
  canUse(props.capabilities?.supports_pr_title_body_edit, permissions.value.can_edit_title_body),
);
const canToggleDraft = computed(() =>
  canUse(props.capabilities?.supports_pr_draft_toggle, permissions.value.can_toggle_draft),
);
const canManageReviewers = computed(() =>
  canUse(
    props.capabilities?.supports_pr_reviewer_management,
    permissions.value.can_manage_reviewers,
  ),
);
const canManageAssignees = computed(() =>
  canUse(
    props.capabilities?.supports_pr_assignee_management,
    permissions.value.can_manage_assignees,
  ),
);
const canManageLabels = computed(() =>
  canUse(props.capabilities?.supports_pr_label_management, permissions.value.can_manage_labels),
);
const canManageMilestone = computed(() =>
  canUse(
    props.capabilities?.supports_pr_milestone_management,
    permissions.value.can_manage_milestone,
  ),
);
const isGitee = computed(() => props.capabilities?.platform === "gitee");
const participantLabels = computed(() =>
  isGitee.value
    ? { reviewers: "评审者", assignees: "测试者" }
    : { reviewers: "Reviewers", assignees: "Assignees" },
);
const categoryLabels = computed(() =>
  isGitee.value
    ? { labels: "标签", milestone: "里程碑" }
    : { labels: "Labels", milestone: "Milestone" },
);

function labelColor(value: string | null): string | undefined {
  const color = value?.trim();
  if (!color || !/^#?[0-9a-f]{6}$/i.test(color)) return undefined;
  return color.startsWith("#") ? color : `#${color}`;
}

const participantOptions = computed(() => {
  const options = new Map<
    string,
    {
      value: string;
      label: string;
      description?: string | null;
      avatarUrl?: string | null;
    }
  >();
  for (const participant of [
    ...props.detail.reviewers,
    ...props.detail.assignees,
    ...availableParticipants.value,
  ]) {
    const login = participant.login.trim();
    const key = login.toLocaleLowerCase();
    if (!login || options.has(key)) continue;
    options.set(key, {
      value: login,
      label: login,
      description: participant.name && participant.name !== login ? participant.name : null,
      avatarUrl: participant.avatar_url,
    });
  }
  return [...options.values()];
});

const labelOptions = computed(() => {
  const options = new Map<
    string,
    {
      value: string;
      label: string;
      color?: string;
      description?: string | null;
    }
  >();
  for (const label of [
    ...props.detail.summary.labels.map((name) => ({ name, color: null, description: null })),
    ...availableLabels.value,
  ]) {
    const name = label.name.trim();
    const key = name.toLocaleLowerCase();
    if (!name) continue;
    const existing = options.get(key);
    if (existing) {
      existing.color ||= labelColor(label.color);
      existing.description ||= label.description;
      continue;
    }
    options.set(key, {
      value: name,
      label: name,
      color: labelColor(label.color),
      description: label.description,
    });
  }
  return [...options.values()];
});

const hasEditableField = computed(
  () =>
    canEditTitleBody.value ||
    canToggleDraft.value ||
    canManageReviewers.value ||
    canManageAssignees.value ||
    canManageLabels.value ||
    canManageMilestone.value,
);
const hasUnknownPermission = computed(() =>
  [
    props.capabilities?.supports_pr_title_body_edit ? permissions.value.can_edit_title_body : false,
    props.capabilities?.supports_pr_draft_toggle ? permissions.value.can_toggle_draft : false,
    props.capabilities?.supports_pr_reviewer_management
      ? permissions.value.can_manage_reviewers
      : false,
    props.capabilities?.supports_pr_assignee_management
      ? permissions.value.can_manage_assignees
      : false,
    props.capabilities?.supports_pr_label_management ? permissions.value.can_manage_labels : false,
    props.capabilities?.supports_pr_milestone_management
      ? permissions.value.can_manage_milestone
      : false,
  ].some((value) => value == null),
);

function resetForm(): void {
  title.value = props.detail.summary.title;
  body.value = props.detail.body;
  draft.value = props.detail.draft ?? false;
  reviewers.value = props.detail.reviewers.map((user) => user.login).filter(Boolean);
  assignees.value = props.detail.assignees.map((user) => user.login).filter(Boolean);
  labels.value = [...props.detail.summary.labels];
  milestone.value = props.detail.milestone?.title ?? "";
  validationError.value = "";
}

function invalidateOptions(): void {
  optionsSequence += 1;
  participantsLoading.value = false;
  labelsLoading.value = false;
}

watch(
  () => [props.detail, props.platform, props.owner, props.repo] as const,
  () => {
    invalidateOptions();
    resetForm();
    availableParticipants.value = [];
    availableLabels.value = [];
    editing.value = false;
  },
  { immediate: true },
);

async function loadParticipantOptions(sequence: number): Promise<void> {
  if (!canManageReviewers.value && !canManageAssignees.value) return;
  participantsLoading.value = true;
  try {
    const result = await prParticipantSuggestions(props.platform, props.owner, props.repo);
    if (sequence !== optionsSequence) return;
    const seen = new Set<string>();
    availableParticipants.value = result.filter((participant) => {
      const login = participant.login.trim();
      const key = login.toLocaleLowerCase();
      if (!login || seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  } catch (cause) {
    if (sequence === optionsSequence) {
      participantsError.value = getErrorMessage(cause, "无法读取目标仓库成员");
    }
  } finally {
    if (sequence === optionsSequence) participantsLoading.value = false;
  }
}

async function loadLabelOptions(sequence: number): Promise<void> {
  if (!canManageLabels.value) return;
  labelsLoading.value = true;
  try {
    const result = await prLabels(props.platform, props.owner, props.repo);
    if (sequence !== optionsSequence) return;
    const seen = new Set<string>();
    availableLabels.value = result.filter((label) => {
      const name = label.name.trim();
      const key = name.toLocaleLowerCase();
      if (!name || seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  } catch (cause) {
    if (sequence === optionsSequence) {
      labelsError.value = getErrorMessage(cause, "无法读取目标仓库标签");
    }
  } finally {
    if (sequence === optionsSequence) labelsLoading.value = false;
  }
}

function loadOptions(): void {
  const sequence = ++optionsSequence;
  availableParticipants.value = [];
  availableLabels.value = [];
  participantsError.value = "";
  labelsError.value = "";
  void Promise.all([loadParticipantOptions(sequence), loadLabelOptions(sequence)]);
}

function normalizeSelection(value: string[]): string[] {
  const seen = new Set<string>();
  return value
    .map((item) => item.trim())
    .filter((item) => {
      const key = item.toLocaleLowerCase();
      if (!item || seen.has(key)) return false;
      seen.add(key);
      return true;
    });
}

function startEditing(): void {
  resetForm();
  editing.value = true;
  loadOptions();
}

function cancelEditing(): void {
  invalidateOptions();
  resetForm();
  editing.value = false;
}

function submit(): void {
  const normalizedTitle = title.value.trim();
  if (!normalizedTitle) {
    validationError.value = "PR 标题不能为空";
    return;
  }
  validationError.value = "";
  emit("save", {
    title: canEditTitleBody.value ? normalizedTitle : props.detail.summary.title,
    body: canEditTitleBody.value ? body.value : props.detail.body,
    draft: props.capabilities?.supports_pr_draft_toggle
      ? canToggleDraft.value
        ? draft.value
        : props.detail.draft
      : null,
    reviewers: canManageReviewers.value
      ? normalizeSelection(reviewers.value)
      : props.detail.reviewers.map((user) => user.login),
    assignees: canManageAssignees.value
      ? normalizeSelection(assignees.value)
      : props.detail.assignees.map((user) => user.login),
    labels: canManageLabels.value ? normalizeSelection(labels.value) : props.detail.summary.labels,
    milestone: canManageMilestone.value
      ? milestone.value.trim() || null
      : (props.detail.milestone?.title ?? null),
    expected_updated_at: props.detail.summary.updated_at,
  });
}

onUnmounted(invalidateOptions);
</script>

<template>
  <section class="metadata-panel" aria-labelledby="pr-metadata-heading">
    <div class="metadata-heading-row">
      <div>
        <p class="metadata-eyebrow">PR / MR 元数据</p>
        <h3 id="pr-metadata-heading">参与者与分类</h3>
      </div>
      <button
        v-if="!editing"
        class="btn btn-sm btn-outline"
        type="button"
        :disabled="!hasEditableField || saving"
        :title="hasEditableField ? '编辑 PR / MR 元数据' : '当前 Token 没有可用的元数据编辑权限'"
        data-testid="edit-pr-metadata"
        @click="startEditing"
      >
        编辑元数据
      </button>
    </div>

    <div v-if="!editing" class="metadata-summary">
      <div class="metadata-item">
        <span class="metadata-label">状态</span>
        <span class="metadata-value">{{
          detail.draft == null ? "平台未提供" : detail.draft ? "Draft" : "Ready"
        }}</span>
      </div>
      <div class="metadata-item">
        <span class="metadata-label">{{ participantLabels.reviewers }}</span>
        <span class="metadata-value">
          {{ detail.reviewers.map((user) => user.login).join("、") || "未指定" }}
        </span>
      </div>
      <div v-if="capabilities?.supports_pr_assignee_management" class="metadata-item">
        <span class="metadata-label">{{ participantLabels.assignees }}</span>
        <span class="metadata-value">
          {{ detail.assignees.map((user) => user.login).join("、") || "未指定" }}
        </span>
      </div>
      <div class="metadata-item">
        <span class="metadata-label">{{ categoryLabels.labels }}</span>
        <span class="metadata-value metadata-tags">
          <span v-for="label in detail.summary.labels" :key="label" class="metadata-tag">
            {{ label }}
          </span>
          <span v-if="detail.summary.labels.length === 0">未指定</span>
        </span>
      </div>
      <div class="metadata-item">
        <span class="metadata-label">{{ categoryLabels.milestone }}</span>
        <span class="metadata-value">{{ detail.milestone?.title || "未指定" }}</span>
      </div>
      <MarkdownRenderer v-if="detail.body" :content="detail.body" class="metadata-description" />
      <p v-else class="metadata-description metadata-description-empty">暂无描述</p>
    </div>

    <form v-else class="metadata-form" @submit.prevent="submit">
      <label class="field field-wide">
        <span>标题</span>
        <input
          v-model="title"
          data-testid="metadata-title"
          type="text"
          :disabled="!canEditTitleBody || saving"
        />
      </label>
      <label class="field field-wide">
        <span>描述</span>
        <textarea
          v-model="body"
          data-testid="metadata-body"
          rows="5"
          :disabled="!canEditTitleBody || saving"
        />
      </label>
      <label v-if="capabilities?.supports_pr_draft_toggle" class="draft-control">
        <input
          v-model="draft"
          data-testid="metadata-draft"
          type="checkbox"
          :disabled="!canToggleDraft || saving"
        />
        <span>标记为 Draft</span>
      </label>
      <label v-if="capabilities?.supports_pr_reviewer_management" class="field">
        <span>{{ participantLabels.reviewers }}</span>
        <AppMultiSelect
          v-model="reviewers"
          :options="participantOptions"
          :placeholder="participantsLoading ? '加载中…' : `选择${participantLabels.reviewers}`"
          :search-placeholder="`搜索${participantLabels.reviewers}`"
          empty-text="仓库暂无成员"
          empty-search-text="没有匹配成员"
          :aria-label="participantLabels.reviewers"
          :disabled="!canManageReviewers || saving || participantsLoading"
          data-testid="metadata-reviewers"
        />
      </label>
      <label v-if="capabilities?.supports_pr_assignee_management" class="field">
        <span>{{ participantLabels.assignees }}</span>
        <AppMultiSelect
          v-model="assignees"
          :options="participantOptions"
          :placeholder="participantsLoading ? '加载中…' : `选择${participantLabels.assignees}`"
          :search-placeholder="`搜索${participantLabels.assignees}`"
          empty-text="仓库暂无成员"
          empty-search-text="没有匹配成员"
          :aria-label="participantLabels.assignees"
          :disabled="!canManageAssignees || saving || participantsLoading"
          data-testid="metadata-assignees"
        />
      </label>
      <label v-if="capabilities?.supports_pr_label_management" class="field">
        <span>{{ categoryLabels.labels }}</span>
        <AppMultiSelect
          v-model="labels"
          :options="labelOptions"
          :placeholder="labelsLoading ? '加载中…' : '选择标签'"
          search-placeholder="搜索标签"
          empty-text="仓库暂无标签"
          empty-search-text="没有匹配标签"
          :aria-label="categoryLabels.labels"
          :disabled="!canManageLabels || saving || labelsLoading"
          data-testid="metadata-labels"
        />
      </label>
      <label v-if="capabilities?.supports_pr_milestone_management" class="field">
        <span>{{ categoryLabels.milestone }}</span>
        <input
          v-model="milestone"
          data-testid="metadata-milestone"
          type="text"
          :disabled="!canManageMilestone || saving"
          placeholder="留空表示移除 Milestone"
        />
      </label>
      <p v-if="hasUnknownPermission" class="permission-note">
        部分权限无法预先确认；保存时会由平台 API 使用当前 Token 再次校验。
      </p>
      <div v-if="participantsError || labelsError" class="options-error" role="alert">
        <p class="error-msg">
          {{ [participantsError, labelsError].filter(Boolean).join("；") }}
        </p>
        <button
          class="btn btn-sm btn-outline"
          type="button"
          :disabled="saving || participantsLoading || labelsLoading"
          data-testid="metadata-options-retry"
          @click="loadOptions"
        >
          重新加载候选项
        </button>
      </div>
      <p v-if="validationError" class="error-msg" role="alert">{{ validationError }}</p>
      <div class="metadata-form-actions">
        <button class="btn btn-sm" type="button" :disabled="saving" @click="cancelEditing">
          取消
        </button>
        <button class="btn btn-sm btn-primary" type="submit" :disabled="saving">
          {{ saving ? "正在保存…" : "保存元数据" }}
        </button>
      </div>
    </form>

    <p v-if="statusMessage" class="metadata-status success-msg" role="status">
      {{ statusMessage }}
    </p>
    <p v-if="errorMessage" class="metadata-status error-msg" role="alert">
      {{ errorMessage }}
    </p>
  </section>
</template>

<style scoped>
.metadata-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-surface);
  box-shadow: var(--shadow-sm);
}

.metadata-heading-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

.metadata-eyebrow {
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.metadata-heading-row h3 {
  margin-top: 2px;
  font-size: 15px;
}

.metadata-summary {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: var(--space-3);
}

.metadata-item {
  min-width: 0;
}

.metadata-label,
.field > span {
  display: block;
  margin-bottom: var(--space-1);
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 700;
}

.metadata-value {
  display: block;
  overflow-wrap: anywhere;
  color: var(--color-text);
  font-size: 13px;
}

.metadata-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
}

.metadata-tag {
  padding: 2px 7px;
  border: 1px solid var(--color-primary-border);
  border-radius: var(--radius-full, 999px);
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-size: 11px;
  font-weight: 600;
}

.metadata-description {
  grid-column: 1 / -1;
  max-height: 200px;
  overflow: auto;
  padding-top: var(--space-3);
  border-top: 1px solid var(--color-border-subtle);
  overflow-wrap: anywhere;
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.6;
}
.metadata-description :deep(h1),
.metadata-description :deep(h2),
.metadata-description :deep(h3),
.metadata-description :deep(h4) {
  margin: 0.5em 0 0.25em;
  font-weight: 600;
  color: var(--color-text-primary);
  font-size: 14px;
}
.metadata-description :deep(p) {
  margin: 0.25em 0;
}
.metadata-description :deep(ul),
.metadata-description :deep(ol) {
  margin: 0.25em 0;
  padding-left: 1.5em;
}
.metadata-description :deep(li) {
  margin: 0.1em 0;
}
.metadata-description :deep(code) {
  background: var(--color-bg-tertiary);
  padding: 1px 4px;
  border-radius: 3px;
  font-size: 12px;
}
.metadata-description :deep(pre) {
  background: var(--color-bg-tertiary);
  padding: var(--space-2);
  border-radius: 4px;
  overflow-x: auto;
  font-size: 12px;
}
.metadata-description :deep(pre code) {
  background: none;
  padding: 0;
}
.metadata-description :deep(a) {
  color: var(--color-primary);
  text-decoration: underline;
}
.metadata-description :deep(blockquote) {
  border-left: 3px solid var(--color-border);
  padding-left: var(--space-2);
  margin: 0.25em 0;
  color: var(--color-text-tertiary);
}
.metadata-description :deep(img) {
  max-width: 100%;
  border-radius: 4px;
}
.metadata-description :deep(hr) {
  border: none;
  border-top: 1px solid var(--color-border-subtle);
  margin: var(--space-2) 0;
}

.metadata-description-empty {
  color: var(--color-text-tertiary);
  font-style: italic;
}

.metadata-form {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.field {
  min-width: 0;
}

.field-wide,
.draft-control,
.permission-note,
.options-error,
.metadata-form-actions,
.metadata-form > .error-msg {
  grid-column: 1 / -1;
}

.options-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.options-error .error-msg {
  margin: 0;
}

.field input,
.field textarea {
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
  color: var(--color-text);
  font: inherit;
  font-size: 13px;
  transition:
    border-color var(--transition-fast),
    box-shadow var(--transition-fast);
}

.field textarea {
  resize: vertical;
  line-height: 1.55;
}

.field input:focus-visible,
.field textarea:focus-visible {
  outline: 2px solid transparent;
  outline-offset: 0;
  border-color: var(--color-focus);
  background: var(--color-surface);
  box-shadow: var(--shadow-control-focus);
}

.field input:disabled,
.field textarea:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.draft-control {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  width: fit-content;
  color: var(--color-text-secondary);
  font-size: 13px;
}

.permission-note {
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.metadata-form-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}

.metadata-status {
  margin: 0;
}

@media (max-width: 760px) {
  .metadata-form {
    grid-template-columns: 1fr;
  }
}
</style>
