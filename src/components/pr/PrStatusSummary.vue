<script setup lang="ts">
import type { PrStatusSummary, ReadinessState } from "@/types";

const props = defineProps<{
  status: PrStatusSummary;
}>();

const overallLabels: Record<ReadinessState, string> = {
  ready: "可合并",
  blocked: "被阻塞",
  pending: "检查中",
  unknown: "状态未知",
};

const approvalLabels: Record<ReadinessState, string> = {
  ready: "审批已满足",
  blocked: "审批未完成",
  pending: "审批进行中",
  unknown: "审批未知",
};

const checksLabels: Record<ReadinessState, string> = {
  ready: "CI/测试通过",
  blocked: "CI/测试失败",
  pending: "CI/测试中",
  unknown: "CI/测试未知",
};

function blockingReasonText(): string {
  const reasons = props.status.blocking_reasons.map((reason) => reason.message);
  return reasons.length > 0 ? reasons.join("；") : overallLabels[props.status.status];
}
</script>

<template>
  <span
    class="status-summary"
    :title="blockingReasonText()"
    :aria-label="`合并状态：${blockingReasonText()}`"
  >
    <span class="status-chip overall-status" :class="`status-${props.status.status}`">
      {{ overallLabels[props.status.status] }}
    </span>
    <span class="status-chip" :class="`status-${props.status.approvals_status}`">
      {{ approvalLabels[props.status.approvals_status] }}
    </span>
    <span class="status-chip" :class="`status-${props.status.checks_status}`">
      {{ checksLabels[props.status.checks_status] }}
    </span>
    <span v-if="props.status.draft" class="status-chip status-draft">Draft</span>
    <span v-if="props.status.has_conflicts" class="status-chip status-blocked">存在冲突</span>
  </span>
</template>

<style scoped>
.status-summary {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.status-chip {
  flex: 0 0 auto;
  padding: 2px 7px;
  border: 1px solid var(--color-border-light);
  border-radius: 999px;
  background: var(--color-surface-hover);
  color: var(--color-text-secondary);
  font-size: 11px;
  font-weight: 600;
  line-height: 16px;
}

.overall-status {
  font-weight: 700;
}

.status-ready {
  border-color: var(--color-success-border);
  background: var(--color-success-light);
  color: var(--color-success);
}

.status-blocked {
  border-color: var(--color-danger-border);
  background: var(--color-danger-light);
  color: var(--color-danger);
}

.status-pending,
.status-draft {
  border-color: var(--color-warning-border);
  background: var(--color-warning-light);
  color: var(--color-warning);
}

.status-unknown {
  color: var(--color-text-tertiary);
}
</style>
