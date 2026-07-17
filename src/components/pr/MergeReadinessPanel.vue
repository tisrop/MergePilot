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

const statusDetails = computed(() => {
  if (props.error && !props.readiness) return [props.error];
  if (!props.readiness) return [props.loading ? "正在读取最新合并条件" : "尚未获取合并状态"];

  const readiness = props.readiness;
  const details = readiness.blocking_reasons.map((reason) => reason.message);
  if (details.length > 0) return details;

  if (readiness.status === "ready") return ["所有合并条件均已满足"];
  if (readiness.status === "pending") return ["平台检查尚未全部完成"];
  if (readiness.status === "unknown") {
    return ["平台未返回完整合并信息；仍可尝试合并，平台会在提交时执行最终校验"];
  }
  return ["存在未满足的合并条件"];
});
</script>

<template>
  <div class="readiness-control">
    <div
      class="readiness-status"
      :class="`state-${state}`"
      role="status"
      tabindex="0"
      :aria-describedby="'merge-readiness-details'"
    >
      <span class="state-icon" aria-hidden="true">{{ stateIcon }}</span>
      <span>{{ stateLabel }}</span>
    </div>

    <button
      class="refresh-button"
      type="button"
      :disabled="loading"
      :aria-label="loading ? '正在刷新合并状态' : '刷新合并状态'"
      :title="loading ? '正在刷新…' : '刷新合并状态'"
      @click="emit('retry')"
    >
      <svg
        :class="{ spinning: loading }"
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M20 11a8 8 0 1 0-2.34 5.66" />
        <path d="M20 4v7h-7" />
      </svg>
    </button>

    <div id="merge-readiness-details" class="readiness-tooltip" role="tooltip">
      <ul>
        <li v-for="detail in statusDetails" :key="detail">{{ detail }}</li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.readiness-control {
  position: relative;
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 4px;
}

.readiness-status {
  display: inline-flex;
  min-height: 34px;
  align-items: center;
  gap: 7px;
  padding: 6px 10px;
  border: 1px solid var(--color-warning-border);
  border-radius: var(--radius-md);
  color: var(--color-warning);
  background: var(--color-warning-light);
  font-size: 11px;
  font-weight: 650;
  white-space: nowrap;
}

.state-icon {
  display: grid;
  width: 17px;
  height: 17px;
  place-items: center;
  border-radius: 50%;
  color: currentColor;
  background: color-mix(in srgb, currentColor 14%, transparent);
  font-size: 11px;
  font-weight: 800;
}

.state-ready {
  color: var(--color-success);
  border-color: var(--color-success-border);
  background: var(--color-success-light);
}

.state-blocked {
  color: var(--color-danger);
  border-color: var(--color-danger-border);
  background: var(--color-danger-light);
}

.state-pending {
  color: var(--color-focus);
  border-color: var(--color-primary-border);
  background: var(--color-primary-light);
}

.refresh-button {
  display: grid;
  width: 30px;
  height: 30px;
  padding: 0;
  place-items: center;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  color: var(--color-text-secondary);
  background: var(--color-surface);
  cursor: pointer;
}

.refresh-button:hover:not(:disabled) {
  color: var(--color-primary);
  border-color: var(--color-primary-border);
  background: var(--color-primary-light);
}

.refresh-button:disabled {
  cursor: wait;
  opacity: 0.65;
}

.refresh-button svg.spinning {
  animation: readiness-spin 0.8s linear infinite;
}

.readiness-tooltip {
  position: absolute;
  z-index: 120;
  top: calc(100% + 7px);
  left: 0;
  width: max-content;
  min-width: 260px;
  max-width: 360px;
  padding: var(--space-3);
  visibility: hidden;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  color: var(--color-text);
  background: var(--color-surface-raised);
  box-shadow: var(--shadow-md);
  font-size: 12px;
  opacity: 0;
  pointer-events: none;
  transform: translateY(-3px);
  transition:
    opacity var(--transition-fast),
    transform var(--transition-fast),
    visibility var(--transition-fast);
}

.readiness-control:hover .readiness-tooltip,
.readiness-control:focus-within .readiness-tooltip {
  visibility: visible;
  opacity: 1;
  transform: translateY(0);
}

.readiness-tooltip ul {
  margin: 0;
  padding-left: 18px;
  color: var(--color-text-secondary);
}

@keyframes readiness-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .refresh-button svg.spinning {
    animation: none;
  }

  .readiness-tooltip {
    transition: none;
  }
}
</style>
