<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { usePrStore } from "@/stores/usePrStore";
import { reviewCommentAdd } from "@/api";
import AppLayout from "@/components/layout/AppLayout.vue";
import DiffViewer from "@/components/diff/DiffViewer.vue";
import ReviewForm from "@/components/review/ReviewForm.vue";
import ReviewList from "@/components/review/ReviewList.vue";
import AiReviewPanel from "@/components/ai/AiReviewPanel.vue";
import type { Platform, ReviewCommentPosition } from "@/types";

const route = useRoute();
const auth = useAuthStore();
const pr = usePrStore();

const platform = route.params.platform as Platform;
const owner = route.params.owner as string;
const repo = route.params.repo as string;
const number = Number(route.params.number);

const activeTab = ref<"diff" | "reviews" | "ai">("diff");

const inlineComments = ref<ReviewCommentPosition[]>([]);

async function handleAddComment(path: string, startLine: number, endLine: number, _side: string, body?: string) {
  if (!pr.currentPr?.head_sha || !body) return;
  try {
    await reviewCommentAdd(platform, owner, repo, number, pr.currentPr.head_sha, path, endLine, body);
  } catch (e) {
    console.error("Failed to add review comment:", e);
  }
}

onMounted(async () => {
  await Promise.all([
    pr.fetchPrDetail(platform, owner, repo, number),
    pr.fetchPrDiff(platform, owner, repo, number),
  ]);
});
</script>

<template>
  <AppLayout>
    <template #header>
      <div class="pr-header">
        <div class="pr-header-top">
          <h2 v-if="pr.currentPr">{{ pr.currentPr.summary.title }}</h2>
          <div class="pr-header-skeleton" v-else>
            <div class="skeleton skeleton-title" />
            <div class="skeleton skeleton-subtitle" />
          </div>
        </div>
        <div class="pr-meta" v-if="pr.currentPr">
          <span class="branch">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="6" r="3"/><circle cx="18" cy="18" r="3"/></svg>
            {{ pr.currentPr.source_branch }} → {{ pr.currentPr.target_branch }}
          </span>
          <span class="author">by {{ pr.currentPr.summary.author.login }}</span>
        </div>
      </div>
    </template>

    <div v-if="pr.loading" class="loading-state">
      <div class="skeleton skeleton-tabs" />
      <div class="skeleton skeleton-content" />
    </div>

    <div v-else-if="pr.currentPr" class="pr-detail">
      <div class="tabs">
        <button
          :class="{ active: activeTab === 'diff' }"
          @click="activeTab = 'diff'"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v18M3 12h18"/></svg>
          Diff
        </button>
        <button
          :class="{ active: activeTab === 'reviews' }"
          @click="activeTab = 'reviews'"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
          评审意见
        </button>
        <button
          :class="{ active: activeTab === 'ai' }"
          @click="activeTab = 'ai'"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a4 4 0 0 1 4 4c0 2-2 4-4 4s-4-2-4-4a4 4 0 0 1 4-4z"/><path d="M12 14c-4.42 0-8 1.79-8 4v2h16v-2c0-2.21-3.58-4-8-4z"/></svg>
          AI 评审
        </button>
      </div>

      <div class="tab-content">
        <div v-if="activeTab === 'diff'">
          <DiffViewer :diff="pr.diff" @add-comment="handleAddComment" />
          <ReviewForm
            :platform="platform"
            :owner="owner"
            :repo="repo"
            :pr-number="number"
          />
        </div>
        <div v-else-if="activeTab === 'reviews'">
          <ReviewList
            :platform="platform"
            :owner="owner"
            :repo="repo"
            :pr-number="number"
          />
        </div>
        <div v-else-if="activeTab === 'ai'">
          <AiReviewPanel
            :platform="platform"
            :owner="owner"
            :repo="repo"
            :pr-number="number"
            :diff="pr.diff?.diff ?? ''"
            :context="pr.currentPr ? { title: pr.currentPr.summary.title, body: pr.currentPr.body } : null"
          />
        </div>
      </div>
    </div>
  </AppLayout>
</template>

<style scoped>
.pr-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.pr-header-top h2 {
  font-size: 18px;
  font-weight: 700;
}

.pr-meta {
  font-size: 12px;
  color: var(--color-text-secondary);
  display: flex;
  gap: var(--space-3);
  align-items: center;
}

.branch {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-family: var(--font-mono);
  font-size: 12px;
}

.loading-state {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.skeleton-tabs {
  height: 40px;
  border-radius: var(--radius-md);
}

.skeleton-content {
  height: 400px;
  border-radius: var(--radius-lg);
}

.pr-header-skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.skeleton-title {
  height: 24px;
  width: 60%;
  border-radius: var(--radius-sm);
}

.skeleton-subtitle {
  height: 16px;
  width: 40%;
  border-radius: var(--radius-sm);
}

.tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid var(--color-border);
  margin-bottom: var(--space-4);
}

.tabs button {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-5);
  border: none;
  background: none;
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-secondary);
  border-bottom: 2px solid transparent;
  transition: all var(--transition-fast);
}

.tabs button.active {
  color: var(--color-primary);
  border-bottom-color: var(--color-primary);
}

.tabs button:hover:not(.active) {
  color: var(--color-text);
  background: var(--color-surface-hover);
}
</style>
