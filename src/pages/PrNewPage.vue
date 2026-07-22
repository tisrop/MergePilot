<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import AppLayout from "@/components/layout/AppLayout.vue";
import DiffViewer from "@/components/diff/DiffViewer.vue";
import AppMultiSelect from "@/components/shared/AppMultiSelect.vue";
import AppSelect from "@/components/shared/AppSelect.vue";
import MarkdownRenderer from "@/components/shared/MarkdownRenderer.vue";
import { prBranches, prCreate, prCreatePreview, prLabels, prParticipantSuggestions } from "@/api";
import { useAuthStore } from "@/stores/useAuthStore";
import { useCapabilityStore } from "@/stores/useCapabilityStore";
import { usePrStore } from "@/stores/usePrStore";
import { useRepoStore } from "@/stores/useRepoStore";
import type {
  Platform,
  PlatformCapabilities,
  PrBranchOptions,
  PrCreatePreview,
  PrLabel,
  User,
} from "@/types";
import { getErrorMessage } from "@/utils/error";
import { persistPrCreateWarnings, PR_CREATE_WARNING_QUERY } from "@/utils/prCreateWarnings";

interface RepositoryRef {
  owner: string;
  repo: string;
  fullName: string;
}

const router = useRouter();
const route = useRoute();
const auth = useAuthStore();
const repoStore = useRepoStore();
const prStore = usePrStore();
const capabilities = useCapabilityStore();

const sourceFullName = ref("");
const targetFullName = ref("");
const repositoriesLoading = ref(false);
const repositoryError = ref("");
const sourceBranch = ref("");
const targetBranch = ref("");
const sourceBranches = ref<string[]>([]);
const targetBranches = ref<string[]>([]);
const branchesLoading = ref(false);
const branchError = ref("");
const preview = ref<PrCreatePreview | null>(null);
const previewLoading = ref(false);
const previewError = ref("");
const previewTab = ref<"commits" | "diff">("commits");
const selectedDiffCommitSha = ref("");
const commitPreview = ref<PrCreatePreview | null>(null);
const commitPreviewLoading = ref(false);
const commitPreviewError = ref("");
const title = ref("");
const body = ref("");
const descriptionMode = ref<"edit" | "preview">("edit");
const draft = ref(false);
const reviewers = ref<string[]>([]);
const assignees = ref<string[]>([]);
const availableParticipants = ref<User[]>([]);
const participantsLoading = ref(false);
const participantsError = ref("");
const labels = ref<string[]>([]);
const availableLabels = ref<PrLabel[]>([]);
const labelsLoading = ref(false);
const labelsError = ref("");
const submitting = ref(false);
const error = ref("");
let branchSequence = 0;
let previewSequence = 0;
let commitPreviewSequence = 0;
let labelsSequence = 0;
let participantsSequence = 0;

function parsePlatform(value: unknown): Platform | null {
  return value === "github" || value === "gitlab" || value === "gitee" ? value : null;
}

const creationPlatform = computed<Platform>(
  () =>
    parsePlatform(route.params.platform) ??
    parsePlatform(route.query.platform) ??
    auth.activePlatform,
);
const creationActiveRepo = computed(() => repoStore.activeRepos[creationPlatform.value]);
const creationForkContext = computed(() => repoStore.forkContexts[creationPlatform.value]);
const creationViewingUpstream = computed(() => {
  const target = targetRepository.value;
  const fork = creationForkContext.value;
  return Boolean(target && fork?.upstreamFullName === target.fullName);
});
const targetRepository = computed<RepositoryRef | null>(() => {
  return parseRepository(targetFullName.value);
});
const platformCapabilities = computed<PlatformCapabilities | null>(
  () => capabilities.values[creationPlatform.value],
);
const isGitee = computed(() => creationPlatform.value === "gitee");
const requestType = computed(() => (creationPlatform.value === "gitlab" ? "MR" : "PR"));
const createLabel = computed(() => "创建 " + requestType.value);
const participantLabels = computed(() =>
  isGitee.value
    ? { reviewers: "评审者", assignees: "测试者" }
    : { reviewers: "Reviewers", assignees: "Assignees" },
);

function parseRepository(fullName: string): RepositoryRef | null {
  const parts = fullName.split("/").filter(Boolean);
  if (parts.length < 2) return null;
  return { owner: parts[0], repo: parts.slice(1).join("/"), fullName: parts.join("/") };
}

const routeTarget = computed(() =>
  typeof route.query.target === "string" ? parseRepository(route.query.target) : null,
);
const isGlobalCreation = computed(() => !routeTarget.value);
const targetRepositories = computed<RepositoryRef[]>(() => {
  const references: RepositoryRef[] = [];
  const add = (reference: RepositoryRef | null) => {
    if (reference && !references.some((item) => item.fullName === reference.fullName)) {
      references.push(reference);
    }
  };
  add(
    creationActiveRepo.value
      ? {
          ...creationActiveRepo.value,
          fullName: `${creationActiveRepo.value.owner}/${creationActiveRepo.value.repo}`,
        }
      : null,
  );
  for (const item of repoStore.reposCache[creationPlatform.value]) {
    add(parseRepository(item.full_name));
    if (item.fork && item.parent_full_name) {
      add(parseRepository(item.parent_full_name));
    }
  }
  return references;
});
const targetRepositoryOptions = computed(() =>
  targetRepositories.value.map((item) => ({ value: item.fullName, label: item.fullName })),
);
const hasMoreRepositories = computed(() => {
  const platform = creationPlatform.value;
  return repoStore.pages[platform] < repoStore.totalPagesByPlatform[platform];
});
const repositoriesLoadingMore = computed(() => {
  const platform = creationPlatform.value;
  return repositoriesLoading.value || repoStore.loadingMoreByPlatform[platform];
});
const sourceRepositories = computed<RepositoryRef[]>(() => {
  const references: RepositoryRef[] = [];
  const add = (reference: RepositoryRef | null) => {
    if (reference && !references.some((item) => item.fullName === reference.fullName)) {
      references.push(reference);
    }
  };
  const target = targetRepository.value;
  add(target);
  if (target) {
    for (const item of repoStore.reposCache[creationPlatform.value]) {
      const isTarget = item.full_name === target.fullName;
      const isTargetFork = item.fork && item.parent_full_name === target.fullName;
      if (isTarget || isTargetFork) add(parseRepository(item.full_name));
    }
  }
  const fork = creationForkContext.value;
  if (fork && target && fork.upstreamFullName === target.fullName) {
    add({
      owner: fork.forkOwner,
      repo: fork.forkRepo,
      fullName: `${fork.forkOwner}/${fork.forkRepo}`,
    });
  }
  return references;
});
const sourceRepository = computed(() =>
  sourceRepositories.value.find((item) => item.fullName === sourceFullName.value),
);
const sourceRepositoryOptions = computed(() =>
  sourceRepositories.value.map((item) => ({ value: item.fullName, label: item.fullName })),
);
const sourceBranchOptions = computed(() =>
  sourceBranches.value.map((branch) => ({ value: branch, label: branch })),
);
const targetBranchOptions = computed(() =>
  targetBranches.value.map((branch) => ({ value: branch, label: branch })),
);

function labelColor(value: string | null): string | undefined {
  const color = value?.trim();
  if (!color || !/^#?[0-9a-f]{6}$/i.test(color)) return undefined;
  return color.startsWith("#") ? color : `#${color}`;
}

const labelOptions = computed(() =>
  availableLabels.value.map((label) => ({
    value: label.name,
    label: label.name,
    color: labelColor(label.color),
    description: label.description,
  })),
);
const participantOptions = computed(() =>
  availableParticipants.value.map((participant) => ({
    value: participant.login,
    label: participant.login,
    description:
      participant.name && participant.name !== participant.login ? participant.name : null,
    avatarUrl: participant.avatar_url,
  })),
);
const previewAdditions = computed(() =>
  preview.value?.diff.files.reduce((total, file) => total + file.additions, 0),
);
const previewDeletions = computed(() =>
  preview.value?.diff.files.reduce((total, file) => total + file.deletions, 0),
);
const diffCommitOptions = computed(() => [
  {
    value: "",
    label: `全部提交 (${preview.value?.commits.length ?? 0})`,
  },
  ...(preview.value?.commits.map((commit) => ({
    value: commit.sha,
    label: `${shortCommitSha(commit.sha)} · ${commit.title || "无标题提交"}`,
  })) ?? []),
]);
const displayedPreview = computed(() =>
  selectedDiffCommitSha.value ? commitPreview.value : preview.value,
);
const displayedDiff = computed(() => displayedPreview.value?.diff ?? null);
const displayedPreviewIncomplete = computed(() => displayedPreview.value?.incomplete === true);
const displayedPreviewWarning = computed(() => {
  const reasons = displayedPreview.value?.incomplete_reasons ?? [];
  if (reasons.includes("pagination_failed")) {
    return `后续分页加载失败，当前仅展示已获取的 Commit 和 Diff，不影响创建 ${requestType.value}。`;
  }
  if (reasons.includes("pagination_limit")) {
    return `变更超过客户端分页安全上限，当前仅展示前 10,000 个 Commit，不影响创建 ${requestType.value}。`;
  }
  return `平台 API 仅返回了部分 Commit 或 Diff，不影响创建 ${requestType.value}。`;
});

function normalizedBranches(options: PrBranchOptions): string[] {
  return Array.from(
    new Set(
      options.default_branch ? [options.default_branch, ...options.branches] : options.branches,
    ),
  );
}

function preferredTargetBranch(options: PrBranchOptions, branches: string[]): string {
  return (
    (options.default_branch && branches.includes(options.default_branch)
      ? options.default_branch
      : null) ??
    branches.find((branch) => branch === "main") ??
    branches.find((branch) => branch === "master") ??
    branches[0] ??
    ""
  );
}

function shortCommitSha(sha: string): string {
  return sha.slice(0, 8);
}

function commitDate(value: string): string {
  if (!value) return "";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

async function loadPreview(): Promise<void> {
  const target = targetRepository.value;
  const source = sourceRepository.value;
  const platform = creationPlatform.value;
  const nextSourceBranch = sourceBranch.value;
  const nextTargetBranch = targetBranch.value;
  const sequence = ++previewSequence;
  commitPreviewSequence += 1;
  preview.value = null;
  selectedDiffCommitSha.value = "";
  commitPreview.value = null;
  commitPreviewLoading.value = false;
  commitPreviewError.value = "";
  previewError.value = "";
  if (
    !target ||
    !source ||
    !nextSourceBranch ||
    !nextTargetBranch ||
    (source.fullName === target.fullName && nextSourceBranch === nextTargetBranch)
  ) {
    previewLoading.value = false;
    return;
  }
  previewLoading.value = true;
  try {
    const result = await prCreatePreview(platform, target.owner, target.repo, {
      source_owner: source.owner,
      source_repo: source.repo,
      source_branch: nextSourceBranch,
      target_branch: nextTargetBranch,
    });
    if (sequence !== previewSequence) return;
    preview.value = result;
  } catch (cause) {
    if (sequence !== previewSequence) return;
    previewError.value = getErrorMessage(cause, "无法生成 " + requestType.value + " 预览");
  } finally {
    if (sequence === previewSequence) previewLoading.value = false;
  }
}

async function loadCommitPreview(): Promise<void> {
  const target = targetRepository.value;
  const source = sourceRepository.value;
  const platform = creationPlatform.value;
  const commitSha = selectedDiffCommitSha.value;
  const nextSourceBranch = sourceBranch.value;
  const nextTargetBranch = targetBranch.value;
  const sequence = ++commitPreviewSequence;
  commitPreview.value = null;
  commitPreviewError.value = "";
  if (!target || !source || !commitSha || !nextSourceBranch || !nextTargetBranch) {
    commitPreviewLoading.value = false;
    return;
  }
  commitPreviewLoading.value = true;
  try {
    const result = await prCreatePreview(platform, target.owner, target.repo, {
      source_owner: source.owner,
      source_repo: source.repo,
      source_branch: nextSourceBranch,
      target_branch: nextTargetBranch,
      commit_sha: commitSha,
    });
    if (sequence !== commitPreviewSequence) return;
    commitPreview.value = result;
  } catch (cause) {
    if (sequence !== commitPreviewSequence) return;
    commitPreviewError.value = getErrorMessage(cause, "无法读取该提交的 Diff");
  } finally {
    if (sequence === commitPreviewSequence) commitPreviewLoading.value = false;
  }
}

async function loadBranches(): Promise<void> {
  const target = targetRepository.value;
  const source = sourceRepository.value;
  if (!target || !source) return;
  const platform = creationPlatform.value;
  const sequence = ++branchSequence;
  branchesLoading.value = true;
  branchError.value = "";
  try {
    const targetRequest = prBranches(platform, target.owner, target.repo);
    const sourceRequest =
      source.fullName === target.fullName
        ? targetRequest
        : prBranches(platform, source.owner, source.repo);
    const [targetOptions, sourceOptions] = await Promise.all([targetRequest, sourceRequest]);
    if (sequence !== branchSequence) return;
    targetBranches.value = normalizedBranches(targetOptions);
    sourceBranches.value = normalizedBranches(sourceOptions);
    if (!targetBranches.value.includes(targetBranch.value)) {
      targetBranch.value = preferredTargetBranch(targetOptions, targetBranches.value);
    }
    if (!sourceBranches.value.includes(sourceBranch.value)) {
      sourceBranch.value =
        sourceBranches.value.find(
          (branch) => source.fullName !== target.fullName || branch !== targetBranch.value,
        ) ??
        sourceBranches.value[0] ??
        "";
    }
  } catch (cause) {
    if (sequence !== branchSequence) return;
    sourceBranches.value = [];
    targetBranches.value = [];
    branchError.value = getErrorMessage(cause, "无法读取仓库分支");
  } finally {
    if (sequence === branchSequence) branchesLoading.value = false;
  }
}

async function loadLabels(): Promise<void> {
  const target = targetRepository.value;
  const platform = creationPlatform.value;
  const sequence = ++labelsSequence;
  labels.value = [];
  availableLabels.value = [];
  labelsError.value = "";
  if (!target || platformCapabilities.value?.supports_pr_label_management === false) {
    labelsLoading.value = false;
    return;
  }
  labelsLoading.value = true;
  try {
    const result = await prLabels(platform, target.owner, target.repo);
    if (sequence !== labelsSequence) return;
    const seen = new Set<string>();
    availableLabels.value = result.filter((label) => {
      const name = label.name.trim();
      const key = name.toLocaleLowerCase();
      if (!name || seen.has(key)) return false;
      seen.add(key);
      label.name = name;
      return true;
    });
  } catch (cause) {
    if (sequence !== labelsSequence) return;
    labelsError.value = getErrorMessage(cause, "无法读取目标仓库标签");
  } finally {
    if (sequence === labelsSequence) labelsLoading.value = false;
  }
}

async function loadParticipantSuggestions(): Promise<void> {
  const target = targetRepository.value;
  const platform = creationPlatform.value;
  const sequence = ++participantsSequence;
  reviewers.value = [];
  assignees.value = [];
  availableParticipants.value = [];
  participantsError.value = "";
  if (
    !target ||
    (platformCapabilities.value?.supports_pr_reviewer_management === false &&
      platformCapabilities.value?.supports_pr_assignee_management === false)
  ) {
    participantsLoading.value = false;
    return;
  }
  participantsLoading.value = true;
  try {
    const result = await prParticipantSuggestions(platform, target.owner, target.repo);
    if (sequence !== participantsSequence) return;
    const seen = new Set<string>();
    availableParticipants.value = result.filter((participant) => {
      const login = participant.login.trim();
      const key = login.toLocaleLowerCase();
      if (!login || seen.has(key)) return false;
      seen.add(key);
      participant.login = login;
      return true;
    });
  } catch (cause) {
    if (sequence !== participantsSequence) return;
    participantsError.value = getErrorMessage(cause, "无法读取目标仓库成员");
  } finally {
    if (sequence === participantsSequence) participantsLoading.value = false;
  }
}

function selectInitialSource(): void {
  const target = targetRepository.value;
  if (!target) {
    sourceFullName.value = "";
    return;
  }
  const fork = creationForkContext.value;
  sourceFullName.value =
    creationViewingUpstream.value && fork ? `${fork.forkOwner}/${fork.forkRepo}` : target.fullName;
}

function selectInitialTarget(): void {
  const preferred = routeTarget.value ?? targetRepositories.value[0] ?? null;
  targetFullName.value = preferred?.fullName ?? "";
}

async function loadInitialRepositories(platform: Platform): Promise<void> {
  repositoriesLoading.value = true;
  repositoryError.value = "";
  try {
    await repoStore.ensureRepos(platform);
    if (platform === creationPlatform.value) {
      repositoryError.value = repoStore.errors[platform] ?? "";
    }
  } finally {
    repositoriesLoading.value = false;
  }
}

async function loadMoreRepositories(): Promise<void> {
  const platform = creationPlatform.value;
  if (!isGlobalCreation.value || repositoriesLoadingMore.value) return;
  repositoryError.value = "";
  await repoStore.loadMore(platform);
  if (platform === creationPlatform.value) {
    repositoryError.value = repoStore.errors[platform] ?? "";
  }
}

const canSubmit = computed(() => {
  const target = targetRepository.value;
  const source = sourceRepository.value;
  if (!target || !source || !platformCapabilities.value?.supports_pr_creation) return false;
  if (!title.value.trim() || !sourceBranch.value || !targetBranch.value) return false;
  if (
    branchesLoading.value ||
    previewLoading.value ||
    submitting.value ||
    branchError.value ||
    previewError.value ||
    !preview.value
  )
    return false;
  if (
    !preview.value.incomplete &&
    preview.value.commits.length === 0 &&
    preview.value.diff.files.length === 0
  )
    return false;
  return !(source.fullName === target.fullName && sourceBranch.value === targetBranch.value);
});

async function handleSubmit(): Promise<void> {
  const target = targetRepository.value;
  const source = sourceRepository.value;
  if (!target || !source || !canSubmit.value) return;
  const platform = creationPlatform.value;
  submitting.value = true;
  error.value = "";
  try {
    const outcome = await prCreate(platform, target.owner, target.repo, {
      source_owner: source.owner,
      source_repo: source.repo,
      source_branch: sourceBranch.value,
      target_branch: targetBranch.value,
      title: title.value.trim(),
      body: body.value,
      draft: platformCapabilities.value?.supports_pr_draft_toggle ? draft.value : false,
      reviewers: platformCapabilities.value?.supports_pr_reviewer_management ? reviewers.value : [],
      assignees: platformCapabilities.value?.supports_pr_assignee_management ? assignees.value : [],
      labels: platformCapabilities.value?.supports_pr_label_management ? labels.value : [],
    });
    repoStore.setActiveRepo(target.owner, target.repo, platform);
    repoStore.setForkContext(null, platform);
    prStore.clearContext();
    const createWarnings = outcome.failures.map((failure) => failure.message);
    persistPrCreateWarnings(platform, target.owner, target.repo, outcome.number, createWarnings);
    await router.push({
      name: "pr-detail",
      params: {
        platform,
        owner: target.owner,
        repo: target.repo,
        number: outcome.number,
      },
      query: createWarnings.length > 0 ? { [PR_CREATE_WARNING_QUERY]: "1" } : undefined,
    });
  } catch (cause) {
    error.value = getErrorMessage(cause, "创建 " + requestType.value + " 失败");
  } finally {
    submitting.value = false;
  }
}

onMounted(async () => {
  const platform = creationPlatform.value;
  try {
    await capabilities.load(platform);
  } catch {
    // Capability store exposes the loading error below.
  }
  if (isGlobalCreation.value) await loadInitialRepositories(platform);
  selectInitialTarget();
  const previousSource = sourceFullName.value;
  selectInitialSource();
  if (sourceFullName.value === previousSource) await loadBranches();
});

watch(sourceFullName, () => {
  sourceBranches.value = [];
  sourceBranch.value = "";
  void loadBranches();
});
watch(
  () => [
    creationPlatform.value,
    targetRepository.value?.fullName,
    sourceRepository.value?.fullName,
    sourceBranch.value,
    targetBranch.value,
  ],
  () => void loadPreview(),
);
watch(selectedDiffCommitSha, () => void loadCommitPreview());
watch(
  () => [creationPlatform.value, targetRepository.value?.fullName] as const,
  async () => {
    const sequence = ++branchSequence;
    void loadLabels();
    void loadParticipantSuggestions();
    sourceBranches.value = [];
    targetBranches.value = [];
    sourceBranch.value = "";
    targetBranch.value = "";
    branchError.value = "";
    if (!targetRepositories.value.some((item) => item.fullName === targetFullName.value)) {
      selectInitialTarget();
    }
    try {
      await capabilities.load(creationPlatform.value);
    } catch {
      // Capability store exposes the loading error below.
    }
    if (sequence !== branchSequence) return;
    const previousSource = sourceFullName.value;
    selectInitialSource();
    if (sourceFullName.value === previousSource) await loadBranches();
  },
);
onUnmounted(() => {
  branchSequence += 1;
  previewSequence += 1;
  commitPreviewSequence += 1;
  labelsSequence += 1;
  participantsSequence += 1;
});
</script>

<template>
  <AppLayout compact-sidebar>
    <template #header>
      <div class="pr-new-header">
        <div>
          <h2>{{ createLabel }}</h2>
          <p v-if="targetRepository">目标仓库：{{ targetRepository.fullName }}</p>
          <p v-else>请先选择目标仓库</p>
        </div>
        <RouterLink class="btn btn-sm" to="/pr">返回列表</RouterLink>
      </div>
    </template>

    <form class="pr-create-form" @submit.prevent="handleSubmit">
      <section class="form-section">
        <div class="section-heading">
          <span>01</span>
          <div>
            <h3>选择变更来源</h3>
            <p>源仓库可以选择当前仓库或已加载的 Fork。</p>
          </div>
        </div>
        <div v-if="isGlobalCreation" class="target-repository-field field">
          <span>目标仓库</span>
          <AppSelect
            v-model="targetFullName"
            :options="targetRepositoryOptions"
            :placeholder="
              repositoriesLoading
                ? '加载中…'
                : targetRepositories.length
                  ? '选择目标仓库'
                  : '暂无可用仓库'
            "
            searchable
            search-placeholder="搜索目标仓库"
            aria-label="目标仓库"
            :has-more="hasMoreRepositories"
            :loading-more="repositoriesLoadingMore"
            :load-more-text="repositoryError ? '重试加载仓库' : '加载更多仓库'"
            @load-more="loadMoreRepositories"
          />
          <p v-if="repositoryError" class="error-msg" role="alert">{{ repositoryError }}</p>
        </div>
        <div class="branch-grid">
          <div class="field">
            <span>源仓库</span>
            <AppSelect
              v-model="sourceFullName"
              :options="sourceRepositoryOptions"
              searchable
              search-placeholder="搜索仓库"
              aria-label="源仓库"
            />
          </div>
          <div class="field">
            <span>源分支</span>
            <AppSelect
              v-model="sourceBranch"
              :options="sourceBranchOptions"
              :placeholder="branchesLoading ? '加载中…' : '选择源分支'"
              searchable
              search-placeholder="搜索源分支"
              aria-label="源分支"
            />
          </div>
          <div class="branch-arrow" aria-hidden="true">→</div>
          <div class="field">
            <span>目标分支</span>
            <AppSelect
              v-model="targetBranch"
              :options="targetBranchOptions"
              :placeholder="branchesLoading ? '加载中…' : '选择目标分支'"
              searchable
              search-placeholder="搜索目标分支"
              aria-label="目标分支"
            />
          </div>
        </div>
        <p v-if="branchError" class="error-msg" role="alert">{{ branchError }}</p>
        <p
          v-else-if="
            sourceRepository?.fullName === targetRepository?.fullName &&
            sourceBranch === targetBranch
          "
          class="validation-note"
        >
          同一仓库的源分支与目标分支必须不同。
        </p>
      </section>

      <section
        v-if="
          sourceBranch &&
          targetBranch &&
          !(
            sourceRepository?.fullName === targetRepository?.fullName &&
            sourceBranch === targetBranch
          )
        "
        class="form-section preview-section"
      >
        <div class="section-heading preview-heading">
          <span>02</span>
          <div>
            <h3>变更预览</h3>
            <p>
              {{ sourceRepository?.fullName }}:{{ sourceBranch }} →
              {{ targetRepository?.fullName }}:{{ targetBranch }}
            </p>
          </div>
          <div v-if="preview" class="preview-summary" aria-label="变更统计">
            <span>{{ preview.commits.length }} 个提交</span>
            <span>{{ preview.diff.files.length }} 个文件</span>
            <strong class="additions">+{{ previewAdditions }}</strong>
            <strong class="deletions">-{{ previewDeletions }}</strong>
          </div>
        </div>

        <div v-if="displayedPreviewIncomplete" class="preview-warning" role="alert">
          <strong>预览不完整</strong>
          <span>{{ displayedPreviewWarning }}</span>
        </div>

        <div class="preview-tabs" role="tablist" aria-label="创建预览">
          <button
            type="button"
            role="tab"
            :aria-selected="previewTab === 'commits'"
            :class="{ active: previewTab === 'commits' }"
            @click="previewTab = 'commits'"
          >
            Commit<span v-if="preview" class="tab-count">{{ preview.commits.length }}</span>
          </button>
          <button
            type="button"
            role="tab"
            :aria-selected="previewTab === 'diff'"
            :class="{ active: previewTab === 'diff' }"
            @click="previewTab = 'diff'"
          >
            Diff<span v-if="preview" class="tab-count">{{ preview.diff.files.length }}</span>
          </button>
        </div>

        <div v-if="previewLoading" class="preview-loading" role="status">正在比较分支…</div>
        <div v-else-if="previewError" class="preview-error" role="alert">
          <span>{{ previewError }}</span>
          <button class="btn btn-sm" type="button" @click="loadPreview">重试</button>
        </div>
        <template v-else-if="preview">
          <ol v-if="previewTab === 'commits'" class="commit-list">
            <li v-for="commit in preview.commits" :key="commit.sha" class="commit-row">
              <code>{{ shortCommitSha(commit.sha) }}</code>
              <div>
                <strong>{{ commit.title || "无标题提交" }}</strong>
                <span>
                  {{ commit.author_name || "未知作者" }}
                  <time v-if="commit.authored_at" :datetime="commit.authored_at">
                    {{ commitDate(commit.authored_at) }}
                  </time>
                </span>
              </div>
            </li>
            <li v-if="preview.commits.length === 0" class="preview-empty">没有待合并提交</li>
          </ol>
          <div v-else class="diff-preview-panel">
            <div class="diff-preview-toolbar">
              <span>Diff 范围</span>
              <AppSelect
                v-model="selectedDiffCommitSha"
                :options="diffCommitOptions"
                size="sm"
                aria-label="Diff 范围"
              />
            </div>
            <div v-if="commitPreviewLoading" class="preview-loading" role="status">
              正在读取提交 Diff…
            </div>
            <div v-else-if="commitPreviewError" class="preview-error" role="alert">
              <span>{{ commitPreviewError }}</span>
              <button class="btn btn-sm" type="button" @click="loadCommitPreview">重试</button>
            </div>
            <DiffViewer v-else-if="displayedDiff" :diff="displayedDiff" read-only />
          </div>
        </template>
      </section>

      <section class="form-section">
        <div class="section-heading">
          <span>03</span>
          <div>
            <h3>说明变更内容</h3>
            <p>创建后仍可在详情页继续修改标题、描述和 Draft 状态。</p>
          </div>
        </div>
        <label class="field field-wide">
          <span>标题</span>
          <input v-model="title" type="text" maxlength="1024" placeholder="简要说明这次变更" />
        </label>
        <div class="field field-wide description-field">
          <div class="description-toolbar">
            <span>描述</span>
            <div class="description-tabs" role="tablist" aria-label="Markdown 描述模式">
              <button
                type="button"
                role="tab"
                :aria-selected="descriptionMode === 'edit'"
                :class="{ active: descriptionMode === 'edit' }"
                @click="descriptionMode = 'edit'"
              >
                编辑
              </button>
              <button
                type="button"
                role="tab"
                :aria-selected="descriptionMode === 'preview'"
                :class="{ active: descriptionMode === 'preview' }"
                @click="descriptionMode = 'preview'"
              >
                预览
              </button>
            </div>
          </div>
          <textarea
            v-if="descriptionMode === 'edit'"
            v-model="body"
            rows="10"
            aria-label="Markdown 描述"
            placeholder="说明背景、实现方式和验证结果…"
          />
          <div v-else class="description-preview" role="tabpanel">
            <MarkdownRenderer v-if="body.trim()" :content="body" />
            <p v-else class="description-preview-empty">暂无预览内容</p>
          </div>
        </div>
        <label v-if="platformCapabilities?.supports_pr_draft_toggle" class="draft-option">
          <input v-model="draft" type="checkbox" />
          <span>
            <strong>创建为 Draft</strong>
            <small>尚未准备好正式评审时使用。</small>
          </span>
        </label>
      </section>

      <section class="form-section">
        <div class="section-heading">
          <span>04</span>
          <div>
            <h3>参与者与分类</h3>
            <p>参与者和标签候选均从目标仓库加载。</p>
          </div>
        </div>
        <div class="metadata-grid">
          <label v-if="platformCapabilities?.supports_pr_reviewer_management" class="field">
            <span>{{ participantLabels.reviewers }}</span>
            <AppMultiSelect
              v-model="reviewers"
              :options="participantOptions"
              :placeholder="participantsLoading ? '加载中…' : `选择${participantLabels.reviewers}`"
              :search-placeholder="`搜索${participantLabels.reviewers}`"
              empty-text="仓库暂无成员"
              empty-search-text="没有匹配成员"
              :aria-label="participantLabels.reviewers"
              :disabled="participantsLoading || Boolean(participantsError)"
            />
          </label>
          <label v-if="platformCapabilities?.supports_pr_assignee_management" class="field">
            <span>{{ participantLabels.assignees }}</span>
            <AppMultiSelect
              v-model="assignees"
              :options="participantOptions"
              :placeholder="participantsLoading ? '加载中…' : `选择${participantLabels.assignees}`"
              :search-placeholder="`搜索${participantLabels.assignees}`"
              empty-text="仓库暂无成员"
              empty-search-text="没有匹配成员"
              :aria-label="participantLabels.assignees"
              :disabled="participantsLoading || Boolean(participantsError)"
            />
          </label>
          <label v-if="platformCapabilities?.supports_pr_label_management" class="field">
            <span>{{ isGitee ? "标签" : "Labels" }}</span>
            <AppMultiSelect
              v-model="labels"
              :options="labelOptions"
              :placeholder="labelsLoading ? '加载中…' : '选择标签'"
              search-placeholder="搜索标签"
              empty-text="仓库暂无标签"
              empty-search-text="没有匹配标签"
              aria-label="Labels"
              :disabled="labelsLoading || Boolean(labelsError)"
            />
          </label>
        </div>
        <p v-if="participantsError" class="error-msg" role="alert">{{ participantsError }}</p>
        <p v-if="labelsError" class="error-msg" role="alert">{{ labelsError }}</p>
      </section>

      <p v-if="capabilities.errors[creationPlatform]" class="error-msg" role="alert">
        {{ capabilities.errors[creationPlatform] }}
      </p>
      <p v-if="error" class="error-msg" role="alert">{{ error }}</p>
      <p
        v-if="platformCapabilities && !platformCapabilities.supports_pr_creation"
        class="validation-note"
        role="status"
      >
        当前平台不支持创建 {{ requestType }}。
      </p>
      <div class="form-actions">
        <span>不会执行本地 checkout、commit 或 push。</span>
        <button class="btn btn-primary" type="submit" :disabled="!canSubmit">
          {{ submitting ? "正在创建…" : createLabel }}
        </button>
      </div>
    </form>
  </AppLayout>
</template>

<style scoped>
.pr-new-header,
.form-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.pr-new-header h2 {
  font-size: 20px;
  letter-spacing: -0.02em;
}

.pr-new-header p {
  margin-top: 2px;
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
  font-size: 11px;
}

.pr-create-form {
  display: flex;
  width: 100%;
  max-width: 1280px;
  flex-direction: column;
  gap: var(--space-4);
  margin: 0 auto;
}

.form-section {
  padding: var(--space-5);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-surface);
  box-shadow: var(--shadow-sm);
}

.section-heading {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  margin-bottom: var(--space-4);
}

.section-heading > span {
  display: grid;
  width: 28px;
  height: 28px;
  place-items: center;
  border-radius: var(--radius-md);
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
}

.section-heading h3 {
  font-size: 15px;
}

.preview-heading {
  align-items: center;
}

.preview-heading > div:nth-child(2) {
  min-width: 0;
  flex: 1;
}

.preview-heading p {
  overflow: hidden;
  font-family: var(--font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.preview-summary {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
  color: var(--color-text-tertiary);
  font-size: 11px;
}

.preview-summary strong {
  font-family: var(--font-mono);
}

.additions {
  color: var(--color-success);
}

.deletions {
  color: var(--color-danger);
}

.preview-tabs {
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
}

.preview-tabs button {
  min-width: 84px;
  padding: 6px 10px;
  border: 0;
  border-radius: calc(var(--radius-md) - 2px);
  background: transparent;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: 12px;
  font-weight: 650;
  cursor: pointer;
}

.preview-tabs button.active {
  background: var(--color-surface);
  box-shadow: var(--shadow-sm);
  color: var(--color-text);
}

.preview-tabs .tab-count {
  margin-left: 6px;
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: 11px;
}

.preview-loading,
.preview-empty {
  padding: var(--space-6);
  color: var(--color-text-tertiary);
  text-align: center;
}

.preview-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) 0;
  color: var(--color-danger);
  font-size: 12px;
}

.preview-warning {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin-top: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border-left: 3px solid var(--color-warning);
  background: var(--color-warning-light);
  color: var(--color-text);
  font-size: 12px;
}

.preview-warning strong {
  flex: 0 0 auto;
  color: var(--color-warning);
}

.commit-list {
  margin-top: var(--space-3);
  border-top: 1px solid var(--color-border);
  list-style: none;
}

.commit-row {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: var(--space-3);
  align-items: start;
  padding: 10px 2px;
  border-bottom: 1px solid var(--color-border);
}

.commit-row > code {
  color: var(--color-primary);
  font-size: 11px;
}

.commit-row > div,
.commit-row strong,
.commit-row span {
  display: block;
  min-width: 0;
}

.commit-row strong {
  overflow-wrap: anywhere;
  font-size: 13px;
  font-weight: 650;
}

.commit-row span {
  margin-top: 3px;
  color: var(--color-text-tertiary);
  font-size: 11px;
}

.commit-row time::before {
  content: " · ";
}

.preview-section :deep(.diff-viewer-wrapper) {
  margin-top: var(--space-3);
}

.diff-preview-toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: var(--space-2);
  margin-top: var(--space-3);
}

.diff-preview-toolbar > span {
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 650;
}

.diff-preview-toolbar :deep(.app-select-wrap) {
  width: min(100%, 420px);
}

.section-heading p,
.form-actions > span,
.validation-note {
  margin-top: 2px;
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.branch-grid {
  display: grid;
  grid-template-columns: minmax(180px, 1.4fr) minmax(160px, 1fr) auto minmax(160px, 1fr);
  align-items: end;
  gap: var(--space-3);
}

.target-repository-field {
  max-width: 520px;
  margin-bottom: var(--space-3);
}

.branch-arrow {
  padding-bottom: 10px;
  color: var(--color-text-tertiary);
  font-size: 18px;
}

.metadata-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
}

.field {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: var(--space-1);
}

.field-wide {
  margin-top: var(--space-3);
}

.field > span,
.description-toolbar > span {
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 650;
}

.description-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.description-tabs {
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
}

.description-tabs button {
  min-width: 56px;
  padding: 4px 8px;
  border: 0;
  border-radius: calc(var(--radius-md) - 2px);
  background: transparent;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: 11px;
  font-weight: 650;
  cursor: pointer;
}

.description-tabs button:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: 1px;
}

.description-tabs button.active {
  background: var(--color-surface);
  box-shadow: var(--shadow-sm);
  color: var(--color-text);
}

.field input,
.field textarea {
  width: 100%;
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
  color: var(--color-text);
  font: inherit;
}

.field textarea {
  resize: vertical;
  line-height: 1.6;
}

.description-preview {
  min-height: 226px;
  padding: 10px 12px;
  overflow-wrap: anywhere;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.6;
}

.description-preview :deep(h1),
.description-preview :deep(h2),
.description-preview :deep(h3),
.description-preview :deep(h4) {
  margin: 0.75em 0 0.35em;
  color: var(--color-text);
  font-size: 15px;
}

.description-preview :deep(h1:first-child),
.description-preview :deep(h2:first-child),
.description-preview :deep(h3:first-child),
.description-preview :deep(h4:first-child),
.description-preview :deep(p:first-child) {
  margin-top: 0;
}

.description-preview :deep(p) {
  margin: 0.4em 0;
}

.description-preview :deep(ul),
.description-preview :deep(ol) {
  margin: 0.4em 0;
  padding-left: 1.6em;
}

.description-preview :deep(code) {
  padding: 1px 4px;
  border-radius: var(--radius-sm);
  background: var(--color-surface);
  font-family: var(--font-mono);
  font-size: 12px;
}

.description-preview :deep(pre) {
  padding: var(--space-3);
  overflow-x: auto;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface);
}

.description-preview :deep(pre code) {
  padding: 0;
  background: none;
}

.description-preview :deep(a) {
  color: var(--color-primary);
  text-decoration: underline;
}

.description-preview :deep(blockquote) {
  margin: 0.5em 0;
  padding-left: var(--space-3);
  border-left: 3px solid var(--color-border);
  color: var(--color-text-tertiary);
}

.description-preview :deep(table) {
  width: 100%;
  border-collapse: collapse;
}

.description-preview :deep(th),
.description-preview :deep(td) {
  padding: 5px 8px;
  border: 1px solid var(--color-border);
  text-align: left;
}

.description-preview :deep(img) {
  max-width: 100%;
}

.description-preview-empty {
  margin: 0;
  color: var(--color-text-tertiary);
}

.field input:focus-visible,
.field textarea:focus-visible {
  outline: 2px solid transparent;
  outline-offset: 0;
  border-color: var(--color-focus);
  background: var(--color-surface);
  box-shadow: var(--shadow-control-focus);
}

.draft-option {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  margin-top: var(--space-3);
  color: var(--color-text-secondary);
  font-size: 13px;
}

.draft-option input {
  margin-top: 2px;
  accent-color: var(--color-primary);
}

.draft-option strong,
.draft-option small {
  display: block;
}

.draft-option small {
  margin-top: 2px;
  color: var(--color-text-tertiary);
  font-size: 11px;
}

.error-msg {
  margin: var(--space-2) 0 0;
  color: var(--color-danger);
  font-size: 12px;
}

.form-actions {
  padding: var(--space-3) 0 var(--space-6);
}

@media (max-width: 820px) {
  .branch-grid,
  .metadata-grid {
    grid-template-columns: 1fr;
  }

  .branch-arrow {
    display: none;
  }

  .preview-heading {
    align-items: flex-start;
    flex-wrap: wrap;
  }

  .preview-summary {
    width: 100%;
    justify-content: flex-start;
    padding-left: 40px;
  }
}
</style>
