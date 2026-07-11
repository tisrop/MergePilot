import { defineStore } from "pinia";
import { ref } from "vue";
import type { Platform, PrSummary, PrDetail, DiffResult, PrState } from "@/types";
import { prList, prDetail, prDiff } from "@/api";

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
      list.value = result.items;
      totalPages.value = result.total_pages;
    } catch (e) {
      error.value = typeof e === "string" ? e : String(e);
      list.value = [];
      totalPages.value = 1;
    } finally {
      loading.value = false;
    }
  }

  async function fetchPrDetail(platform: Platform, owner: string, repo: string, number: number) {
    loading.value = true;
    try {
      currentPr.value = await prDetail(platform, owner, repo, number);
    } finally {
      loading.value = false;
    }
  }

  async function fetchPrDiff(platform: Platform, owner: string, repo: string, number: number) {
    loading.value = true;
    try {
      diff.value = await prDiff(platform, owner, repo, number);
    } finally {
      loading.value = false;
    }
  }

  function setFilter(state: PrState) {
    filters.value.state = state;
    filters.value.page = 1;
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
    fetchPrList,
    fetchPrDetail,
    fetchPrDiff,
    setFilter,
  };
});
