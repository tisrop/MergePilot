import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { reviewInboxList } from "@/api";
import type { Paginated, Platform, ReviewInboxCategory, ReviewInboxItem } from "@/types";

const PLATFORMS: Platform[] = ["github", "gitlab", "gitee"];
const PER_PAGE = 20;

function platformRecord<T>(factory: () => T): Record<Platform, T> {
  return {
    github: factory(),
    gitlab: factory(),
    gitee: factory(),
  };
}

function dedupeItems(items: ReviewInboxItem[]): ReviewInboxItem[] {
  const merged = new Map<string, ReviewInboxItem>();
  for (const item of items) {
    const key = `${item.platform}\u0000${item.repository_full_name}\u0000${item.summary.number}`;
    const existing = merged.get(key);
    if (!existing) {
      merged.set(key, item);
      continue;
    }
    merged.set(key, {
      ...existing,
      categories: Array.from(new Set([...existing.categories, ...item.categories])),
    });
  }
  return Array.from(merged.values());
}

export const useReviewInboxStore = defineStore("review-inbox", () => {
  const itemsByPlatform = ref<Record<Platform, ReviewInboxItem[]>>(platformRecord(() => []));
  const pages = ref<Record<Platform, number>>(platformRecord(() => 0));
  const totalPages = ref<Record<Platform, number>>(platformRecord(() => 1));
  const loadingByPlatform = ref<Record<Platform, boolean>>(platformRecord(() => false));
  const errors = ref<Record<Platform, string | null>>(platformRecord(() => null));
  const failedPages = ref<Record<Platform, number | null>>(platformRecord(() => null));
  const filters = ref<{
    category: ReviewInboxCategory;
    platform: "all" | Platform;
    repository: string;
  }>({
    category: "review_requested",
    platform: "all",
    repository: "",
  });
  const loggedInPlatforms = ref<Platform[]>([]);
  const requestSequences: Record<Platform, number> = platformRecord(() => 0);
  let contextSequence = 0;

  const visiblePlatforms = computed(() => {
    if (filters.value.platform === "all") return loggedInPlatforms.value;
    return loggedInPlatforms.value.includes(filters.value.platform) ? [filters.value.platform] : [];
  });

  const items = computed(() => {
    const repositoryQuery = filters.value.repository.trim().toLocaleLowerCase();
    return dedupeItems(
      visiblePlatforms.value.flatMap((platform) => itemsByPlatform.value[platform]),
    )
      .filter(
        (item) =>
          !repositoryQuery ||
          item.repository_full_name.toLocaleLowerCase().includes(repositoryQuery),
      )
      .sort((left, right) => right.summary.updated_at.localeCompare(left.summary.updated_at));
  });

  const loading = computed(() =>
    loggedInPlatforms.value.some((platform) => loadingByPlatform.value[platform]),
  );
  const hasMore = computed(() =>
    visiblePlatforms.value.some((platform) => pages.value[platform] < totalPages.value[platform]),
  );

  async function fetchPlatform(
    platform: Platform,
    requestedPage: number,
    category: ReviewInboxCategory,
    expectedContext: number,
  ): Promise<void> {
    const requestSequence = ++requestSequences[platform];
    loadingByPlatform.value[platform] = true;
    errors.value[platform] = null;
    failedPages.value[platform] = null;
    try {
      const result: Paginated<ReviewInboxItem> = await reviewInboxList(
        platform,
        category,
        requestedPage,
        PER_PAGE,
      );
      if (
        expectedContext !== contextSequence ||
        requestSequence !== requestSequences[platform] ||
        filters.value.category !== category
      ) {
        return;
      }
      itemsByPlatform.value[platform] =
        requestedPage === 1
          ? dedupeItems(result.items)
          : dedupeItems([...itemsByPlatform.value[platform], ...result.items]);
      pages.value[platform] = result.page;
      totalPages.value[platform] = Math.max(result.page, result.total_pages);
    } catch (cause) {
      if (
        expectedContext === contextSequence &&
        requestSequence === requestSequences[platform] &&
        filters.value.category === category
      ) {
        errors.value[platform] = typeof cause === "string" ? cause : String(cause);
        failedPages.value[platform] = requestedPage;
      }
    } finally {
      if (expectedContext === contextSequence && requestSequence === requestSequences[platform]) {
        loadingByPlatform.value[platform] = false;
      }
    }
  }

  async function refresh(platforms: Platform[]): Promise<void> {
    const expectedContext = ++contextSequence;
    loggedInPlatforms.value = PLATFORMS.filter((platform) => platforms.includes(platform));
    const requestedPlatforms = [...visiblePlatforms.value];
    const category = filters.value.category;

    for (const platform of PLATFORMS) {
      requestSequences[platform] += 1;
      loadingByPlatform.value[platform] = false;
      errors.value[platform] = null;
      failedPages.value[platform] = null;
      itemsByPlatform.value[platform] = [];
      pages.value[platform] = 0;
      totalPages.value[platform] = 1;
    }

    await Promise.all(
      requestedPlatforms.map((platform) => fetchPlatform(platform, 1, category, expectedContext)),
    );
  }

  async function loadMore(): Promise<void> {
    const expectedContext = contextSequence;
    const category = filters.value.category;
    const pending = visiblePlatforms.value
      .filter(
        (platform) =>
          !loadingByPlatform.value[platform] && pages.value[platform] < totalPages.value[platform],
      )
      .map((platform) =>
        fetchPlatform(platform, pages.value[platform] + 1, category, expectedContext),
      );
    await Promise.all(pending);
  }

  function retry(platform: Platform): Promise<void> {
    if (!visiblePlatforms.value.includes(platform)) return Promise.resolve();
    const requestedPage = failedPages.value[platform] ?? Math.max(pages.value[platform], 1);
    return fetchPlatform(platform, requestedPage, filters.value.category, contextSequence);
  }

  function clear(): void {
    contextSequence += 1;
    loggedInPlatforms.value = [];
    for (const platform of PLATFORMS) {
      requestSequences[platform] += 1;
      itemsByPlatform.value[platform] = [];
      pages.value[platform] = 0;
      totalPages.value[platform] = 1;
      loadingByPlatform.value[platform] = false;
      errors.value[platform] = null;
      failedPages.value[platform] = null;
    }
  }

  return {
    itemsByPlatform,
    pages,
    totalPages,
    loadingByPlatform,
    errors,
    filters,
    loggedInPlatforms,
    visiblePlatforms,
    items,
    loading,
    hasMore,
    refresh,
    loadMore,
    retry,
    clear,
  };
});
