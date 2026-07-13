<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import AppLayout from "@/components/layout/AppLayout.vue";
import IssueForm from "@/components/issue/IssueForm.vue";
import { issueCreate } from "@/api";
import { getErrorMessage } from "@/utils/error";

const router = useRouter();
const auth = useAuthStore();
const repo = useRepoStore();

const title = ref("");
const body = ref("");
const labels = ref<string[]>([]);
const submitting = ref(false);
const error = ref("");

async function handleSubmit() {
  if (!repo.activeRepo) return;
  if (!title.value.trim()) {
    error.value = "请输入标题";
    return;
  }

  submitting.value = true;
  error.value = "";
  try {
    await issueCreate(
      auth.activePlatform,
      repo.activeRepo.owner,
      repo.activeRepo.repo,
      title.value,
      body.value,
      labels.value,
    );
    router.push("/issue");
  } catch (e) {
    error.value = getErrorMessage(e, "创建失败");
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <AppLayout>
    <template #header>
      <div class="issue-new-header">
        <h2>新建 Issue</h2>
        <p v-if="repo.activeFullName">将在 {{ repo.activeFullName }} 中创建</p>
        <p v-else>请先选择目标仓库</p>
      </div>
    </template>

    <IssueForm
      v-model:title="title"
      v-model:body="body"
      v-model:labels="labels"
      :error="error"
      :submitting="submitting"
      @submit="handleSubmit"
    />
  </AppLayout>
</template>

<style scoped>
.issue-new-header h2 {
  font-size: 20px;
  letter-spacing: -0.02em;
}

.issue-new-header p {
  margin-top: 2px;
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
  font-size: 11px;
}
</style>
