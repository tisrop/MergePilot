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

// Collect inline comments from the diff viewer
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
        <h2 v-if="pr.currentPr">{{ pr.currentPr.summary.title }}</h2>
        <div class="pr-meta" v-if="pr.currentPr">
          <span class="branch">{{ pr.currentPr.source_branch }} → {{ pr.currentPr.target_branch }}</span>
          <span class="author">by {{ pr.currentPr.summary.author.login }}</span>
        </div>
      </div>
    </template>

    <div v-if="pr.loading" class="loading">加载中...</div>

    <div v-else-if="pr.currentPr" class="pr-detail">
      <div class="tabs">
        <button
          :class="{ active: activeTab === 'diff' }"
          @click="activeTab = 'diff'"
        >
          Diff
        </button>
        <button
          :class="{ active: activeTab === 'reviews' }"
          @click="activeTab = 'reviews'"
        >
          评审意见
        </button>
        <button
          :class="{ active: activeTab === 'ai' }"
          @click="activeTab = 'ai'"
        >
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
  gap: 4px;
}

.pr-meta {
  font-size: 12px;
  color: var(--color-text-secondary);
  display: flex;
  gap: 12px;
}

.loading {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--color-text-secondary);
}

.tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid var(--color-border);
  margin-bottom: 16px;
}

.tabs button {
  padding: 10px 20px;
  border: none;
  background: none;
  font-size: 14px;
  color: var(--color-text-secondary);
  border-bottom: 2px solid transparent;
  transition: all 0.2s;
}

.tabs button.active {
  color: var(--color-primary);
  border-bottom-color: var(--color-primary);
}

.tabs button:hover:not(.active) {
  color: var(--color-text);
}
</style>
