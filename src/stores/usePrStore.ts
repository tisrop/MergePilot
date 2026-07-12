import { defineStore } from "pinia";
import { ref } from "vue";
import type { Platform, PrSummary, PrDetail, DiffResult, PrState, MergeStrategy } from "@/types";
import { prList, prDetail, prDiff, prMerge, prClose, prReopen } from "@/api";

const PAGE_SIZES = [10, 20, 50, 100] as const;

export const usePrStore = defineStore("pr", () => {
  const list = ref<PrSummary[]>([]);
  const currentPr = ref<PrDetail | null>(null);
  const diff = ref<DiffResult | null>(null);
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

  function clearContext() {
    listRequestSequence++;
    detailRequestSequence++;
    diffRequestSequence++;
    countsRequestSequence++;
    list.value = [];
    currentPr.value = null;
    diff.value = null;
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

  async function fetchPrDetail(platform: Platform, owner: string, repo: string, number: number) {
    const sequence = ++detailRequestSequence;
    loading.value = true;
    try {
      const result = await prDetail(platform, owner, repo, number);
      if (sequence === detailRequestSequence) currentPr.value = result;
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
    } catch (e) {
      error.value = typeof e === "string" ? e : String(e);
      throw e;
    }
  }

  return {
    list,
    currentPr,
    diff,
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
    fetchStateCounts,
    setFilter,
    mergePr,
    closePr,
    reopenPr,
  };
});
