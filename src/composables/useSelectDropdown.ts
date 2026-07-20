import { computed, nextTick, onMounted, onUnmounted, ref, watch, type ComputedRef } from "vue";

interface DropdownOptionLike {
  disabled?: boolean;
}

interface SelectDropdownOptions<T extends DropdownOptionLike> {
  options: ComputedRef<readonly T[]>;
  searchText: (option: T) => string;
  isSelected: (option: T) => boolean;
  onSelect: (option: T) => void;
  searchable?: () => boolean;
  disabled?: () => boolean;
  closeOnSelect?: boolean;
  optionSelector: string;
}

export function useSelectDropdown<T extends DropdownOptionLike>(config: SelectDropdownOptions<T>) {
  const open = ref(false);
  const searchQuery = ref("");
  const highlightIndex = ref(-1);
  const wrapperRef = ref<HTMLElement | null>(null);
  const triggerRef = ref<HTMLElement | null>(null);
  const searchInputRef = ref<HTMLInputElement | null>(null);
  const listRef = ref<HTMLElement | null>(null);

  const filteredOptions = computed(() => {
    const query = searchQuery.value.trim().toLocaleLowerCase();
    if (!query) return config.options.value;
    return config.options.value.filter((option) =>
      config.searchText(option).toLocaleLowerCase().includes(query),
    );
  });

  function firstEnabledIndex(): number {
    return filteredOptions.value.findIndex((option) => !option.disabled);
  }

  function selectedEnabledIndex(): number {
    return filteredOptions.value.findIndex(
      (option) => config.isSelected(option) && !option.disabled,
    );
  }

  function resetHighlight(preferSelected: boolean): void {
    const selectedIndex = preferSelected ? selectedEnabledIndex() : -1;
    highlightIndex.value = selectedIndex >= 0 ? selectedIndex : firstEnabledIndex();
  }

  function findEnabledIndex(start: number, direction: 1 | -1): number {
    let index = start;
    while (index >= 0 && index < filteredOptions.value.length) {
      if (!filteredOptions.value[index].disabled) return index;
      index += direction;
    }
    return -1;
  }

  function closeDropdown(restoreFocus = false): void {
    open.value = false;
    searchQuery.value = "";
    if (restoreFocus) {
      searchInputRef.value?.blur();
      triggerRef.value?.focus();
    }
  }

  function openDropdown(): void {
    if (config.disabled?.()) return;
    searchQuery.value = "";
    open.value = true;
    resetHighlight(true);
    if (config.searchable?.() !== false) {
      void nextTick(() => searchInputRef.value?.focus());
    }
  }

  function toggleDropdown(): void {
    if (config.disabled?.()) return;
    if (open.value) closeDropdown();
    else openDropdown();
  }

  function selectOption(option: T): void {
    if (option.disabled) return;
    config.onSelect(option);
    if (config.closeOnSelect) closeDropdown();
  }

  function selectHighlighted(): void {
    const option = filteredOptions.value[highlightIndex.value];
    if (option) selectOption(option);
  }

  function onTriggerKeydown(event: KeyboardEvent): void {
    if (!open.value) {
      if (event.key === "Enter" || event.key === " " || event.key === "ArrowDown") {
        event.preventDefault();
        openDropdown();
      }
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      closeDropdown();
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const direction = event.key === "ArrowDown" ? 1 : -1;
      highlightIndex.value = findEnabledIndex(highlightIndex.value + direction, direction);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectHighlighted();
    }
  }

  function onSearchKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      closeDropdown(true);
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextIndex = findEnabledIndex(highlightIndex.value + direction, direction);
      highlightIndex.value = nextIndex >= 0 ? nextIndex : firstEnabledIndex();
      return;
    }
    if (event.key === "Enter") {
      if (event.isComposing || event.keyCode === 229) return;
      event.preventDefault();
      selectHighlighted();
    }
  }

  function onClickOutside(event: MouseEvent): void {
    if (!open.value || !wrapperRef.value?.contains(event.target as Node)) {
      if (open.value) closeDropdown();
    }
  }

  watch(highlightIndex, async () => {
    await nextTick();
    const option = listRef.value?.querySelectorAll<HTMLElement>(config.optionSelector)[
      highlightIndex.value
    ];
    option?.scrollIntoView?.({ block: "nearest" });
  });

  watch(
    searchQuery,
    () => {
      if (open.value) resetHighlight(false);
    },
    { flush: "sync" },
  );

  watch(config.options, () => {
    if (open.value) resetHighlight(true);
  });

  onMounted(() => document.addEventListener("click", onClickOutside, true));
  onUnmounted(() => document.removeEventListener("click", onClickOutside, true));

  return {
    open,
    searchQuery,
    highlightIndex,
    wrapperRef,
    triggerRef,
    searchInputRef,
    listRef,
    filteredOptions,
    openDropdown,
    closeDropdown,
    toggleDropdown,
    selectOption,
    onTriggerKeydown,
    onSearchKeydown,
  };
}
