<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import AppLayout from "@/components/layout/AppLayout.vue";
import IssueForm from "@/components/issue/IssueForm.vue";
import { issueCreate } from "@/api";

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
  } catch (e: any) {
    error.value = e?.toString() || "创建失败";
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <AppLayout>
    <template #header>
      <h2>新建 Issue</h2>
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
