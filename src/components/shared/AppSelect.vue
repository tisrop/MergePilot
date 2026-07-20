<script setup lang="ts">
import { computed } from "vue";
import { useSelectDropdown } from "@/composables/useSelectDropdown";

type SelectOption = { value: string; label: string; disabled?: boolean };

const props = withDefaults(
  defineProps<{
    id?: string;
    modelValue: string;
    options: SelectOption[];
    placeholder?: string;
    size?: "sm" | "md";
    ariaLabel?: string;
    searchable?: boolean;
    searchPlaceholder?: string;
    hasMore?: boolean;
    loadingMore?: boolean;
    loadMoreText?: string;
  }>(),
  {
    placeholder: "请选择",
    size: "md",
    searchable: false,
    searchPlaceholder: "搜索选项",
    hasMore: false,
    loadingMore: false,
    loadMoreText: "加载更多",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  "load-more": [];
}>();

const selectedLabel = computed(() => {
  const match = props.options.find((o) => o.value === props.modelValue);
  return match ? match.label : "";
});
const optionSource = computed(() => props.options);
const {
  open,
  searchQuery,
  highlightIndex,
  wrapperRef,
  triggerRef,
  searchInputRef,
  listRef,
  filteredOptions,
  toggleDropdown,
  selectOption,
  onTriggerKeydown,
  onSearchKeydown,
} = useSelectDropdown<SelectOption>({
  options: optionSource,
  searchText: (option) => `${option.label} ${option.value}`,
  isSelected: (option) => option.value === props.modelValue,
  onSelect: (option) => emit("update:modelValue", option.value),
  searchable: () => props.searchable,
  closeOnSelect: true,
  optionSelector: ".dropdown-option",
});
</script>

<template>
  <div
    ref="wrapperRef"
    class="app-select-wrap"
    :class="[size === 'sm' ? 'app-select-wrap-sm' : '']"
  >
    <div
      ref="triggerRef"
      :id="id"
      class="app-select"
      tabindex="0"
      role="combobox"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-label="ariaLabel"
      @click="toggleDropdown"
      @keydown="onTriggerKeydown"
    >
      <span class="app-select-value" :class="{ placeholder: !selectedLabel }">
        {{ selectedLabel || placeholder }}
      </span>
      <svg
        class="app-select-chevron"
        :class="{ open }"
        width="12"
        height="12"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <polyline points="6 9 12 15 18 9" />
      </svg>
    </div>

    <div v-if="open" class="dropdown-panel">
      <input
        v-if="searchable"
        ref="searchInputRef"
        v-model="searchQuery"
        class="dropdown-search"
        type="search"
        :placeholder="searchPlaceholder"
        :aria-label="searchPlaceholder"
        @keydown="onSearchKeydown"
      />
      <div ref="listRef" class="dropdown-options" role="listbox">
        <button
          v-for="(opt, i) in filteredOptions"
          :key="opt.value"
          :data-value="opt.value"
          class="dropdown-option"
          :class="{ selected: opt.value === modelValue, highlighted: i === highlightIndex }"
          :disabled="opt.disabled"
          role="option"
          :aria-selected="opt.value === modelValue"
          @click.stop="selectOption(opt)"
          @mouseenter="!opt.disabled && (highlightIndex = i)"
          type="button"
        >
          {{ opt.label }}
        </button>
        <div v-if="filteredOptions.length === 0" class="dropdown-empty">
          {{ searchQuery ? "没有匹配选项" : "无选项" }}
        </div>
      </div>
      <button
        v-if="hasMore"
        class="dropdown-load-more"
        type="button"
        :disabled="loadingMore"
        @click.stop="emit('load-more')"
      >
        {{ loadingMore ? "加载中…" : loadMoreText }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.app-select-wrap {
  position: relative;
  width: 100%;
}

.app-select-wrap-sm {
  width: auto;
  display: inline-block;
}

.app-select {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface);
  color: var(--color-text);
  font-size: 14px;
  font-family: inherit;
  cursor: pointer;
  transition:
    border-color var(--transition-fast),
    box-shadow var(--transition-fast);
  user-select: none;
}

.app-select:hover,
.app-select[aria-expanded="true"] {
  border-color: var(--color-primary-border);
}

.app-select:focus-visible {
  outline: 2px solid transparent;
  outline-offset: 0;
  border-color: var(--color-focus);
  box-shadow: var(--shadow-control-focus);
}

.app-select-wrap-sm .app-select {
  padding: 5px 8px;
  font-size: 12px;
}

.app-select-value {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-select-value.placeholder {
  color: var(--color-text-tertiary);
}

.app-select-chevron {
  flex-shrink: 0;
  color: var(--color-text-tertiary);
  transition: transform var(--transition-fast);
}

.app-select-chevron.open {
  transform: rotate(180deg);
}

.dropdown-panel {
  position: absolute;
  z-index: 50;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  max-height: 280px;
  overflow: hidden;
  padding: 4px;
  display: flex;
  flex-direction: column;
}

.dropdown-search {
  display: block;
  box-sizing: border-box;
  width: 100%;
  margin-bottom: 4px;
  padding: 7px 9px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: var(--color-bg);
  color: var(--color-text);
  font: inherit;
  font-size: 12px;
}

.dropdown-search:focus-visible {
  outline: 2px solid transparent;
  outline-offset: 0;
  border-color: var(--color-focus);
  background: var(--color-surface);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.dropdown-options {
  min-height: 0;
  overflow-y: auto;
}

.dropdown-load-more {
  flex: 0 0 auto;
  width: 100%;
  margin-top: 4px;
  padding: 7px 10px;
  border: 0;
  border-top: 1px solid var(--color-border);
  background: transparent;
  color: var(--color-primary);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.dropdown-load-more:hover:not(:disabled) {
  background: var(--color-surface-hover);
}

.dropdown-load-more:disabled {
  color: var(--color-text-tertiary);
  cursor: wait;
}

.dropdown-option {
  display: flex;
  align-items: center;
  width: 100%;
  padding: 7px 10px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  font-size: 13px;
  color: var(--color-text);
  cursor: pointer;
  text-align: left;
  transition: background var(--transition-fast);
}

.dropdown-option:hover {
  background: var(--color-surface-hover);
}

.dropdown-option.highlighted {
  background: var(--color-control-highlight);
}

.dropdown-option.selected {
  background: var(--color-control-selected);
  box-shadow: inset 2px 0 0 var(--color-focus);
  color: var(--color-primary);
  font-weight: 600;
}

.dropdown-option:disabled {
  color: var(--color-text-tertiary);
  cursor: not-allowed;
  opacity: 0.65;
}

.dropdown-option:disabled:hover {
  background: none;
}

.dropdown-empty {
  padding: var(--space-2);
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 12px;
}
</style>
