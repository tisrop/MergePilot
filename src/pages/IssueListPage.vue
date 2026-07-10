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
        <router-link to="/issue/new" class="new-btn">+ 新建 Issue</router-link>
      </div>
    </template>

    <div v-if="loading" class="loading">加载中...</div>

    <div v-else-if="!repo.activeRepo" class="empty">
      <p>请先在左侧选择一个仓库</p>
    </div>

    <div v-else-if="issues.length === 0" class="empty">
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

.new-btn {
  padding: 8px 16px;
  background: var(--color-success);
  color: #fff;
  border-radius: 6px;
  font-size: 13px;
}

.loading, .empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--color-text-secondary);
}

.issue-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
