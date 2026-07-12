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

const platformRecord = <T>(factory: () => T): Record<Platform, T> => ({
  github: factory(),
  gitlab: factory(),
  gitee: factory(),
});

export const useRepoStore = defineStore("repo", () => {
  const reposCache = ref<Record<Platform, RepoSummary[]>>(platformRecord(() => []));
  const activeRepos = ref<Record<Platform, RepoSelection | null>>(platformRecord(() => null));
  const forkContexts = ref<Record<Platform, ForkContext | null>>(platformRecord(() => null));
  const pages = ref<Record<Platform, number>>(platformRecord(() => 0));
  const totalPagesByPlatform = ref<Record<Platform, number>>(platformRecord(() => 1));
  const loadingByPlatform = ref<Record<Platform, boolean>>(platformRecord(() => false));
  const loadingMoreByPlatform = ref<Record<Platform, boolean>>(platformRecord(() => false));
  const errors = ref<Record<Platform, string | null>>(platformRecord(() => null));
  const failedPages = ref<Record<Platform, number | null>>(platformRecord(() => null));
  const requestSequences: Record<Platform, number> = platformRecord(() => 0);

  const activePlatform = computed(() => useAuthStore().activePlatform);
  const repos = computed(() => reposCache.value[activePlatform.value] ?? []);
  const loading = computed(() => loadingByPlatform.value[activePlatform.value]);
  const loadingMore = computed(() => loadingMoreByPlatform.value[activePlatform.value]);
  const page = computed(() => pages.value[activePlatform.value]);
  const totalPages = computed(() => totalPagesByPlatform.value[activePlatform.value]);
  const hasMore = computed(() => page.value < totalPages.value);
  const error = computed(() => errors.value[activePlatform.value]);
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

  function dedupeRepos(items: RepoSummary[]): RepoSummary[] {
    const seen = new Set<string>();
    return items.filter((item) => {
      const key = `${String(item.id)}\u0000${item.full_name}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }

  async function fetchRepos(platform: Platform, requestedPage: number = 1) {
    const sequence = ++requestSequences[platform];
    const loadingMoreRequest = requestedPage > 1;
    loadingByPlatform.value[platform] = !loadingMoreRequest;
    loadingMoreByPlatform.value[platform] = loadingMoreRequest;
    errors.value[platform] = null;
    failedPages.value[platform] = null;
    try {
      const result: Paginated<RepoSummary> = await repoList(platform, requestedPage);
      if (sequence !== requestSequences[platform]) return;
      reposCache.value[platform] =
        requestedPage === 1
          ? dedupeRepos(result.items)
          : dedupeRepos([...reposCache.value[platform], ...result.items]);
      pages.value[platform] = result.page;
      totalPagesByPlatform.value[platform] = Math.max(result.total_pages, result.page);
    } catch (cause) {
      if (sequence === requestSequences[platform]) {
        errors.value[platform] = typeof cause === "string" ? cause : String(cause);
        failedPages.value[platform] = requestedPage;
      }
    } finally {
      if (sequence === requestSequences[platform]) {
        loadingByPlatform.value[platform] = false;
        loadingMoreByPlatform.value[platform] = false;
      }
    }
  }

  function refreshRepos(platform: Platform) {
    return fetchRepos(platform, 1);
  }

  function loadMore(platform: Platform = activePlatform.value) {
    if (loadingByPlatform.value[platform] || loadingMoreByPlatform.value[platform]) return;
    if (pages.value[platform] >= totalPagesByPlatform.value[platform]) return;
    return fetchRepos(platform, pages.value[platform] + 1);
  }

  function retry(platform: Platform = activePlatform.value) {
    return fetchRepos(platform, failedPages.value[platform] ?? Math.max(pages.value[platform], 1));
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
    loadingMore,
    loadingByPlatform,
    loadingMoreByPlatform,
    page,
    pages,
    totalPages,
    totalPagesByPlatform,
    hasMore,
    error,
    errors,
    fetchRepos,
    refreshRepos,
    loadMore,
    retry,
    setActiveRepo,
    setForkContext,
    switchForkView,
  };
});
