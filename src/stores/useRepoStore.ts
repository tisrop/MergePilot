import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { Platform, RepoSummary, Paginated } from "@/types";
import { repoList } from "@/api";
import { useAuthStore } from "./useAuthStore";

export interface ForkContext {
  upstreamFullName: string | null;
  upstreamOwner: string | null;
  forkOwner: string;
  forkRepo: string;
}

export const useRepoStore = defineStore("repo", () => {
  const reposCache = ref<Record<Platform, RepoSummary[]>>({
    github: [],
    gitlab: [],
    gitee: [],
  });
  const activeRepo = ref<{ owner: string; repo: string } | null>(null);
  const loading = ref(false);
  const forkContext = ref<ForkContext | null>(null);

  const repos = computed(() => {
    const auth = useAuthStore();
    return reposCache.value[auth.activePlatform] ?? [];
  });

  const activeFullName = computed(() => {
    if (!activeRepo.value) return null;
    return `${activeRepo.value.owner}/${activeRepo.value.repo}`;
  });

  /** Whether we are currently viewing the upstream repo of a fork */
  const viewingUpstream = computed(() => {
    if (!forkContext.value || !activeRepo.value || !forkContext.value.upstreamOwner) return false;
    return activeRepo.value.owner === forkContext.value.upstreamOwner;
  });

  /** Whether the fork's upstream info is available */
  const hasUpstreamInfo = computed(() => {
    return !!(forkContext.value?.upstreamFullName && forkContext.value?.upstreamOwner);
  });

  function refreshRepos(platform: Platform) {
    reposCache.value = { ...reposCache.value, [platform]: [] };
    return fetchRepos(platform);
  }

  async function fetchRepos(platform: Platform, page: number = 1) {
    reposCache.value = { ...reposCache.value, [platform]: [] };
    loading.value = true;
    try {
      const result: Paginated<RepoSummary> = await repoList(platform, page);
      reposCache.value = { ...reposCache.value, [platform]: result.items };
    } finally {
      loading.value = false;
    }
  }

  function setActiveRepo(owner: string, repo: string) {
    activeRepo.value = { owner, repo };
  }

  function setForkContext(ctx: ForkContext | null) {
    forkContext.value = ctx;
  }

  /** Switch PR view between upstream and fork repo */
  function switchForkView() {
    if (!forkContext.value) return;
    if (viewingUpstream.value) {
      // Switch to fork repo
      activeRepo.value = { owner: forkContext.value.forkOwner, repo: forkContext.value.forkRepo };
    } else if (forkContext.value.upstreamFullName && forkContext.value.upstreamOwner) {
      // Switch to upstream repo
      activeRepo.value = { owner: forkContext.value.upstreamOwner, repo: forkContext.value.upstreamFullName.split("/").slice(1).join("/") };
    }
  }

  return {
    repos,
    reposCache,
    activeRepo,
    activeFullName,
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
