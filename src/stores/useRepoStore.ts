import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { Platform, RepoSummary, Paginated } from "@/types";
import { repoList } from "@/api";
import { useAuthStore } from "./useAuthStore";

export interface ForkContext {
  upstreamFullName: string | null;
  upstreamOwner: string | null;
  forkOwner: string;
  forkRepo: string;
}

type RepoSelection = { owner: string; repo: string };

const emptyPlatformRecord = <T>(value: T): Record<Platform, T> => ({
  github: value,
  gitlab: value,
  gitee: value,
});

export const useRepoStore = defineStore("repo", () => {
  const reposCache = ref<Record<Platform, RepoSummary[]>>(emptyPlatformRecord([]));
  const activeRepos = ref<Record<Platform, RepoSelection | null>>(emptyPlatformRecord(null));
  const forkContexts = ref<Record<Platform, ForkContext | null>>(emptyPlatformRecord(null));
  const loading = ref(false);

  const activePlatform = computed(() => useAuthStore().activePlatform);
  const repos = computed(() => reposCache.value[activePlatform.value] ?? []);
  const activeRepo = computed<RepoSelection | null>({
    get: () => activeRepos.value[activePlatform.value],
    set: (value) => {
      activeRepos.value[activePlatform.value] = value;
    },
  });
  const forkContext = computed<ForkContext | null>({
    get: () => forkContexts.value[activePlatform.value],
    set: (value) => {
      forkContexts.value[activePlatform.value] = value;
    },
  });

  const activeFullName = computed(() => {
    if (!activeRepo.value) return null;
    return `${activeRepo.value.owner}/${activeRepo.value.repo}`;
  });

  const viewingUpstream = computed(() => {
    if (!forkContext.value || !activeRepo.value || !forkContext.value.upstreamOwner) return false;
    return activeRepo.value.owner === forkContext.value.upstreamOwner;
  });

  const hasUpstreamInfo = computed(() => {
    return !!(forkContext.value?.upstreamFullName && forkContext.value?.upstreamOwner);
  });

  function refreshRepos(platform: Platform) {
    return fetchRepos(platform);
  }

  async function fetchRepos(platform: Platform, page: number = 1) {
    loading.value = true;
    try {
      const result: Paginated<RepoSummary> = await repoList(platform, page);
      reposCache.value = { ...reposCache.value, [platform]: result.items };
    } finally {
      loading.value = false;
    }
  }

  function setActiveRepo(owner: string, repo: string) {
    activeRepos.value[activePlatform.value] = { owner, repo };
  }

  function setForkContext(ctx: ForkContext | null) {
    forkContexts.value[activePlatform.value] = ctx;
  }

  function switchForkView() {
    const context = forkContext.value;
    if (!context) return;
    if (viewingUpstream.value) {
      activeRepo.value = { owner: context.forkOwner, repo: context.forkRepo };
    } else if (context.upstreamFullName && context.upstreamOwner) {
      activeRepo.value = {
        owner: context.upstreamOwner,
        repo: context.upstreamFullName.split("/").slice(1).join("/"),
      };
    }
  }

  return {
    repos,
    reposCache,
    activeRepos,
    activeRepo,
    activeFullName,
    forkContexts,
    forkContext,
    viewingUpstream,
    hasUpstreamInfo,
    loading,
    fetchRepos,
    refreshRepos,
    setActiveRepo,
    setForkContext,
    switchForkView,
  };
});
