<script setup lang="ts">
import { computed } from "vue";
import type { PrMergeReadiness, ReadinessState } from "@/types";

const props = defineProps<{
  readiness: PrMergeReadiness | null;
  loading: boolean;
  error: string | null;
}>();
const emit = defineEmits<{ retry: [] }>();

const stateLabels: Record<ReadinessState, string> = {
  ready: "可合并",
  blocked: "已阻断",
  pending: "检查中",
  unknown: "状态未知",
};
const stateIcons: Record<ReadinessState, string> = {
  ready: "✓",
  blocked: "!",
  pending: "…",
  unknown: "?",
};
const state = computed<ReadinessState>(
  () => props.readiness?.status ?? (props.error ? "unknown" : "pending"),
);
const stateLabel = computed(() => stateLabels[state.value]);
const stateIcon = computed(() => stateIcons[state.value]);
const shortSha = computed(() => {
  const sha = props.readiness?.head_sha;
  return sha ? sha.slice(0, 8) : "—";
});

function valueLabel(value: boolean | null, yes: string, no: string): string {
  if (value === true) return yes;
  if (value === false) return no;
  return "未知";
}
</script>

<template>
  <section class="readiness-panel" aria-labelledby="merge-readiness-title">
    <div class="readiness-heading">
      <div>
        <p class="eyebrow">MERGE READINESS</p>
        <h3 id="merge-readiness-title">合并就绪检查</h3>
      </div>
      <button class="refresh-button" type="button" :disabled="loading" @click="emit('retry')">
        {{ loading ? "检查中…" : "重新检查" }}
      </button>
    </div>

    <div v-if="loading && !readiness" class="readiness-loading" role="status">
      正在读取最新合并条件…
    </div>
    <div v-else-if="error && !readiness" class="readiness-error" role="alert">
      <span>{{ error }}</span>
      <button type="button" @click="emit('retry')">重试</button>
    </div>
    <template v-else>
      <div class="readiness-summary" :class="`state-${state}`" role="status">
        <span class="state-icon" aria-hidden="true">{{ stateIcon }}</span>
        <div>
          <strong>{{ stateLabel }}</strong>
          <p v-if="state === 'unknown'">无法确认所有合并条件，请勿据此判断可以合并。</p>
          <p v-else-if="state === 'pending'">部分检查尚未完成，合并按钮将保持禁用。</p>
          <p v-else-if="state === 'blocked'">请处理下方阻塞原因后重新检查。</p>
          <p v-else>服务端已确认当前版本满足可合并条件。</p>
        </div>
      </div>

      <div class="readiness-grid">
        <div class="readiness-item">
          <span>Draft</span><strong>{{ valueLabel(readiness?.draft ?? null, "是", "否") }}</strong>
        </div>
        <div class="readiness-item">
          <span>冲突</span
          ><strong>{{ valueLabel(readiness?.has_conflicts ?? null, "存在", "无") }}</strong>
        </div>
        <div class="readiness-item">
          <span>测试 / CI</span
          ><strong>{{ stateLabels[readiness?.checks_status ?? "unknown"] }}</strong>
        </div>
        <div class="readiness-item">
          <span>审批</span>
          <strong>
            {{ stateLabels[readiness?.approvals_status ?? "unknown"] }}
            <small
              v-if="
                readiness?.approvals_required !== null &&
                readiness?.approvals_required !== undefined
              "
            >
              ({{ readiness.approvals_received ?? 0 }}/{{ readiness.approvals_required }})
            </small>
          </strong>
        </div>
        <div class="readiness-item">
          <span>合并权限</span
          ><strong>{{ valueLabel(readiness?.has_merge_permission ?? null, "有", "无") }}</strong>
        </div>
        <div class="readiness-item">
          <span>分支状态</span
          ><strong>{{ valueLabel(readiness?.branch_behind ?? null, "落后", "最新") }}</strong>
        </div>
        <div class="readiness-item sha-item">
          <span>检查版本</span><code :title="readiness?.head_sha">{{ shortSha }}</code>
        </div>
      </div>

      <ul v-if="readiness?.blocking_reasons.length" class="blocking-list">
        <li v-for="reason in readiness.blocking_reasons" :key="`${reason.code}-${reason.message}`">
          <span aria-hidden="true">•</span>{{ reason.message }}
        </li>
      </ul>
    </template>
  </section>
</template>

<style scoped>
.readiness-panel {
  margin: var(--space-4) 0;
  padding: var(--space-4);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-surface);
}
.readiness-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
  margin-bottom: var(--space-3);
}
.eyebrow {
  margin: 0 0 2px;
  color: var(--color-text-secondary);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.12em;
}
h3 {
  margin: 0;
  color: var(--color-text);
  font-size: 16px;
}
.refresh-button {
  padding: 6px 10px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  color: var(--color-text-secondary);
  background: var(--color-bg);
  font-size: 12px;
  cursor: pointer;
}
.refresh-button:disabled {
  cursor: wait;
  opacity: 0.6;
}
.readiness-summary {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
}
.readiness-summary strong {
  font-size: 14px;
}
.readiness-summary p {
  margin: 3px 0 0;
  color: var(--color-text-secondary);
  font-size: 12px;
}
.state-icon {
  display: grid;
  width: 30px;
  height: 30px;
  place-items: center;
  border-radius: 50%;
  color: currentColor;
  background: color-mix(in srgb, currentColor 14%, transparent);
  font-weight: 800;
}
.state-ready {
  color: var(--color-success);
  background: var(--color-success-light);
  border-color: var(--color-success-border);
}
.state-blocked {
  color: var(--color-danger);
  background: var(--color-danger-light);
  border-color: var(--color-danger-border);
}
.state-pending {
  color: var(--color-focus);
  background: var(--color-primary-light);
  border-color: var(--color-primary-border);
}
.state-unknown {
  color: var(--color-warning);
  background: var(--color-warning-light);
  border-color: var(--color-warning-border);
}
.readiness-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 1px;
  margin-top: var(--space-3);
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-border);
}
.readiness-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  padding: var(--space-2) var(--space-3);
  background: var(--color-surface);
}
.readiness-item span {
  color: var(--color-text-secondary);
  font-size: 11px;
}
.readiness-item strong,
.readiness-item code {
  overflow: hidden;
  color: var(--color-text);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.readiness-item small {
  color: var(--color-text-secondary);
  font-weight: 400;
}
.blocking-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
  margin: var(--space-3) 0 0;
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  color: var(--color-danger);
  background: var(--color-danger-light);
  font-size: 12px;
}
.blocking-list li {
  display: flex;
  gap: 7px;
  list-style: none;
}
.readiness-loading,
.readiness-error {
  padding: var(--space-3);
  color: var(--color-text-secondary);
  font-size: 13px;
}
.readiness-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  color: var(--color-danger);
}
.readiness-error button {
  border: 0;
  color: var(--color-primary);
  background: none;
  cursor: pointer;
}
@media (max-width: 720px) {
  .readiness-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
