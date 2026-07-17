<script setup lang="ts">
import PrStatusSummary from "@/components/pr/PrStatusSummary.vue";
import type { ReviewInboxItem, ReviewInboxRelationship } from "@/types";

const props = defineProps<{
  item: ReviewInboxItem;
}>();

defineEmits<{
  click: [];
}>();

const platformLabels = {
  github: "GitHub",
  gitlab: "GitLab",
  gitee: "Gitee",
} as const;

const relationshipLabels: Record<ReviewInboxRelationship, string> = {
  reviewer: "评审人",
  assignee: "负责人",
  tester: "测试人",
  author: "我创建的",
};

function formatUpdatedAt(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "更新时间未知" : date.toLocaleDateString("zh-CN");
}
</script>

<template>
  <button type="button" class="inbox-card" @click="$emit('click')">
    <span class="pr-icon" aria-hidden="true">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor">
        <circle cx="6" cy="5" r="2.5" />
        <circle cx="18" cy="19" r="2.5" />
        <path d="M6 7.5V16a3 3 0 0 0 3 3h6.5" />
        <path d="M12 5h3a3 3 0 0 1 3 3v8.5" />
      </svg>
    </span>
    <span class="card-content">
      <span class="card-context">
        <span class="platform-badge" :class="`platform-${props.item.platform}`">
          {{ platformLabels[props.item.platform] }}
        </span>
        <span class="repository-name">{{ props.item.repository_full_name }}</span>
      </span>
      <span class="card-title">{{ props.item.summary.title }}</span>
      <span class="card-meta">
        <span class="pr-number">#{{ props.item.summary.number }}</span>
        <span>{{ props.item.summary.author.login }} 创建</span>
        <span>{{ formatUpdatedAt(props.item.summary.updated_at) }} 更新</span>
        <span
          v-for="relationship in props.item.relationships"
          :key="relationship"
          class="relationship-badge"
        >
          {{ relationshipLabels[relationship] }}
        </span>
      </span>
      <PrStatusSummary :status="props.item.status" />
    </span>
    <svg
      class="chevron"
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      aria-hidden="true"
    >
      <path d="m9 18 6-6-6-6" />
    </svg>
  </button>
</template>

<style scoped>
.inbox-card {
  display: flex;
  width: 100%;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-surface);
  color: var(--color-text);
  text-align: left;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition:
    border-color var(--transition-base),
    box-shadow var(--transition-base),
    transform var(--transition-base);
}

.inbox-card:hover {
  border-color: var(--color-primary-border);
  box-shadow: var(--shadow-md);
  transform: translateY(-1px);
}

.inbox-card:active {
  transform: translateY(0);
}

.pr-icon {
  display: inline-flex;
  width: 34px;
  height: 34px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--color-success-border);
  border-radius: var(--radius-md);
  background: var(--color-success-light);
  color: var(--color-success);
}

.pr-icon svg,
.chevron {
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.card-content {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  gap: var(--space-2);
}

.card-context,
.card-meta {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: var(--space-2);
}

.repository-name {
  overflow: hidden;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-title {
  overflow: hidden;
  font-size: 15px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-meta {
  flex-wrap: wrap;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.pr-number {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
}

.platform-badge,
.relationship-badge {
  flex: 0 0 auto;
  padding: 2px 7px;
  border: 1px solid var(--color-border-light);
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  line-height: 16px;
}

.platform-github {
  background: #f6f8fa;
  color: #1f2328;
}

.platform-gitlab {
  border-color: #e7c5b8;
  background: #fff4ef;
  color: #9f2f13;
}

.platform-gitee {
  border-color: #f2c2c2;
  background: #fff3f3;
  color: #c71d23;
}

.relationship-badge {
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.chevron {
  flex: 0 0 auto;
  color: var(--color-text-tertiary);
  transition:
    color var(--transition-fast),
    transform var(--transition-fast);
}

.inbox-card:hover .chevron {
  color: var(--color-primary);
  transform: translateX(2px);
}

@media (max-width: 760px) {
  .card-meta > span:not(.pr-number):not(.relationship-badge) {
    display: none;
  }
}
</style>
