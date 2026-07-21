import { defineStore } from "pinia";
import { ref } from "vue";
import type {
  Platform,
  PrSummary,
  PrDetail,
  DiffResult,
  PrState,
  MergeStrategy,
  PrMergeReadiness,
  PrMetadataUpdate,
  PrMetadataUpdateOutcome,
} from "@/types";
import {
  prList,
  prDetail,
  prDiff,
  prMerge,
  prMergeReadiness,
  prClose,
  prReopen,
  prMetadataUpdate,
} from "@/api";

const PAGE_SIZES = [10, 20, 50, 100] as const;

export const usePrStore = defineStore("pr", () => {
  const list = ref<PrSummary[]>([]);
  const currentPr = ref<PrDetail | null>(null);
  const diff = ref<DiffResult | null>(null);
  const mergeReadiness = ref<PrMergeReadiness | null>(null);
  const readinessLoading = ref(false);
  const readinessError = ref<string | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const totalPages = ref(1);
  const perPage = ref<number>(20);
  const filters = ref<{ state: PrState; page: number }>({
    state: "open",
    page: 1,
  });
  const stateCounts = ref<Record<PrState, number>>({
    open: 0,
    closed: 0,
    merged: 0,
    all: 0,
  });
  let listRequestSequence = 0;
  let detailRequestSequence = 0;
  let diffRequestSequence = 0;
  let countsRequestSequence = 0;
  let readinessRequestSequence = 0;
  let metadataRequestSequence = 0;
  let listContextKey = "";
  let detailContextKey = "";

  function clearContext() {
    listRequestSequence++;
    detailRequestSequence++;
    diffRequestSequence++;
    countsRequestSequence++;
    readinessRequestSequence++;
    metadataRequestSequence++;
    listContextKey = "";
    detailContextKey = "";
    list.value = [];
    currentPr.value = null;
    diff.value = null;
    mergeReadiness.value = null;
    readinessError.value = null;
    error.value = null;
    totalPages.value = 1;
    stateCounts.value = { open: 0, closed: 0, merged: 0, all: 0 };
  }

  function nextPage() {
    if (filters.value.page < totalPages.value) {
      filters.value.page++;
    }
  }

  function prevPage() {
    if (filters.value.page > 1) {
      filters.value.page--;
    }
  }

  function setPerPage(n: number) {
    perPage.value = n;
    filters.value.page = 1;
  }

  async function fetchPrList(platform: Platform, owner: string, repo: string) {
    const sequence = ++listRequestSequence;
    const contextKey = `${platform}:${owner}/${repo}`;
    if (listContextKey !== contextKey) {
      list.value = [];
      totalPages.value = 1;
    }
    listContextKey = contextKey;
    loading.value = true;
    error.value = null;
    try {
      const result = await prList(
        platform,
        owner,
        repo,
        filters.value.state,
        filters.value.page,
        perPage.value,
      );
      if (sequence !== listRequestSequence) return;
      list.value = result.items;
      totalPages.value = result.total_pages;
    } catch (e) {
      if (sequence !== listRequestSequence) return;
      error.value = typeof e === "string" ? e : String(e);
      list.value = [];
      totalPages.value = 1;
    } finally {
      if (sequence === listRequestSequence) loading.value = false;
    }
  }

  async function fetchPrDetail(
    platform: Platform,
    owner: string,
    repo: string,
    number: number,
  ): Promise<boolean> {
    const sequence = ++detailRequestSequence;
    const contextKey = `${platform}:${owner}/${repo}:${number}`;
    if (detailContextKey !== contextKey) {
      currentPr.value = null;
      diff.value = null;
      mergeReadiness.value = null;
      readinessError.value = null;
    }
    detailContextKey = contextKey;
    loading.value = true;
    error.value = null;
    try {
      const result = await prDetail(platform, owner, repo, number);
      if (sequence !== detailRequestSequence || detailContextKey !== contextKey) return false;
      currentPr.value = result;
      return true;
    } catch (requestError) {
      if (sequence !== detailRequestSequence || detailContextKey !== contextKey) return false;
      currentPr.value = null;
      const message = typeof requestError === "string" ? requestError : String(requestError);
      error.value = /\b404\b|not found/i.test(message)
        ? `找不到 ${owner}/${repo} #${number}，该 PR / MR 可能不存在，或当前 Token 无权访问。`
        : message;
      return false;
    } finally {
      if (sequence === detailRequestSequence) loading.value = false;
    }
  }

  async function fetchPrDiff(platform: Platform, owner: string, repo: string, number: number) {
    const sequence = ++diffRequestSequence;
    loading.value = true;
    try {
      const result = await prDiff(platform, owner, repo, number);
      if (sequence === diffRequestSequence) diff.value = result;
    } finally {
      if (sequence === diffRequestSequence) loading.value = false;
    }
  }

  async function fetchMergeReadiness(
    platform: Platform,
    owner: string,
    repo: string,
    number: number,
  ) {
    const sequence = ++readinessRequestSequence;
    readinessLoading.value = true;
    readinessError.value = null;
    try {
      const result = await prMergeReadiness(platform, owner, repo, number);
      if (sequence === readinessRequestSequence) mergeReadiness.value = result;
    } catch (e) {
      if (sequence !== readinessRequestSequence) return;
      readinessError.value = typeof e === "string" ? e : String(e);
      mergeReadiness.value = null;
    } finally {
      if (sequence === readinessRequestSequence) readinessLoading.value = false;
    }
  }

  function setFilter(state: PrState) {
    filters.value.state = state;
    filters.value.page = 1;
  }

  async function fetchStateCounts(platform: Platform, owner: string, repo: string) {
    const sequence = ++countsRequestSequence;
    const states: PrState[] = ["open", "closed", "merged", "all"];
    const results = await Promise.allSettled(
      states.map((state) => prList(platform, owner, repo, state, 1, 1)),
    );
    if (sequence !== countsRequestSequence) return;
    results.forEach((result, index) => {
      if (result.status === "fulfilled") {
        stateCounts.value[states[index]] = result.value.total_count;
      }
    });
  }

  async function updateMetadata(
    platform: Platform,
    owner: string,
    repo: string,
    number: number,
    update: PrMetadataUpdate,
  ): Promise<PrMetadataUpdateOutcome | null> {
    const sequence = ++metadataRequestSequence;
    const contextKey = `${platform}:${owner}/${repo}:${number}`;
    error.value = null;
    try {
      const outcome = await prMetadataUpdate(platform, owner, repo, number, update);
      if (sequence !== metadataRequestSequence || detailContextKey !== contextKey) return null;
      if (outcome.detail) {
        currentPr.value = outcome.detail;
        if (listContextKey === `${platform}:${owner}/${repo}`) {
          const index = list.value.findIndex((item) => item.number === number);
          if (index >= 0) list.value[index] = outcome.detail.summary;
        }
      }
      if (outcome.failures.length > 0) {
        error.value = outcome.failures.map((failure) => failure.message).join("；");
      }
      return outcome;
    } catch (requestError) {
      if (sequence !== metadataRequestSequence || detailContextKey !== contextKey) return null;
      throw requestError;
    }
  }

  async function mergePr(
    platform: Platform,
    owner: string,
    repo: string,
    number: number,
    strategy: MergeStrategy,
    commitTitle?: string,
    commitMessage?: string,
    closeIssues?: boolean,
  ) {
    error.value = null;
    try {
      const result = await prMerge(
        platform,
        owner,
        repo,
        number,
        strategy,
        commitTitle,
        commitMessage,
        closeIssues,
      );
      currentPr.value = await prDetail(platform, owner, repo, number);
      await fetchMergeReadiness(platform, owner, repo, number);
      return result;
    } catch (e) {
      error.value = typeof e === "string" ? e : String(e);
      throw e;
    }
  }

  async function closePr(platform: Platform, owner: string, repo: string, number: number) {
    error.value = null;
    try {
      await prClose(platform, owner, repo, number);
      currentPr.value = await prDetail(platform, owner, repo, number);
      await fetchMergeReadiness(platform, owner, repo, number);
    } catch (e) {
      error.value = typeof e === "string" ? e : String(e);
      throw e;
    }
  }

  async function reopenPr(platform: Platform, owner: string, repo: string, number: number) {
    error.value = null;
    try {
      await prReopen(platform, owner, repo, number);
      currentPr.value = await prDetail(platform, owner, repo, number);
      await fetchMergeReadiness(platform, owner, repo, number);
    } catch (e) {
      error.value = typeof e === "string" ? e : String(e);
      throw e;
    }
  }

  return {
    list,
    currentPr,
    diff,
    mergeReadiness,
    readinessLoading,
    readinessError,
    loading,
    error,
    totalPages,
    perPage,
    pageSizes: PAGE_SIZES,
    filters,
    nextPage,
    prevPage,
    setPerPage,
    stateCounts,
    clearContext,
    fetchPrList,
    fetchPrDetail,
    fetchPrDiff,
    fetchMergeReadiness,
    updateMetadata,
    fetchStateCounts,
    setFilter,
    mergePr,
    closePr,
    reopenPr,
  };
});
