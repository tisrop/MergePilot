<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import AppLayout from "@/components/layout/AppLayout.vue";
import IssueCard from "@/components/issue/IssueCard.vue";
import type { IssueSummary } from "@/types";
import { issueList } from "@/api";

const auth = useAuthStore();
const repo = useRepoStore();

const issues = ref<IssueSummary[]>([]);
const loading = ref(false);

onMounted(async () => {
  if (!repo.activeRepo) return;
  loading.value = true;
  try {
    const result = await issueList(auth.activePlatform, repo.activeRepo.owner, repo.activeRepo.repo);
    issues.value = result.items;
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <AppLayout>
    <template #header>
      <div class="issue-header">
        <h2>Issues</h2>
        <router-link to="/issue/new" class="btn btn-success btn-sm">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
          新建 Issue
        </router-link>
      </div>
    </template>

    <div v-if="loading" class="loading-skeleton">
      <div class="skeleton skeleton-card" v-for="i in 4" :key="i" />
    </div>

    <div v-else-if="!repo.activeRepo" class="empty-state">
      <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M10 17l5-5-5-5M13 12H3"/></svg>
      <p>请先在左侧选择一个仓库</p>
    </div>

    <div v-else-if="issues.length === 0" class="empty-state">
      <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <p>暂无 Issue</p>
    </div>

    <div v-else class="issue-list">
      <IssueCard
        v-for="item in issues"
        :key="item.number"
        :issue="item"
      />
    </div>
  </AppLayout>
</template>

<style scoped>
.issue-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.issue-header h2 {
  font-size: 20px;
  font-weight: 700;
}

.loading-skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.skeleton-card {
  height: 76px;
  border-radius: var(--radius-lg);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 240px;
  gap: var(--space-3);
  color: var(--color-text-tertiary);
}

.empty-state p {
  font-size: 14px;
}

.empty-state svg {
  opacity: 0.4;
}

.issue-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
</style>
