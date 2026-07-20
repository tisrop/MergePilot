<script setup lang="ts">
import { computed } from "vue";
import { useSelectDropdown } from "@/composables/useSelectDropdown";

export interface MultiSelectOption {
  value: string;
  label: string;
  color?: string | null;
  description?: string | null;
  avatarUrl?: string | null;
  disabled?: boolean;
}

const props = withDefaults(
  defineProps<{
    modelValue: string[];
    options: MultiSelectOption[];
    placeholder?: string;
    searchPlaceholder?: string;
    emptyText?: string;
    emptySearchText?: string;
    ariaLabel?: string;
    disabled?: boolean;
  }>(),
  {
    placeholder: "请选择",
    searchPlaceholder: "搜索选项",
    emptyText: "暂无选项",
    emptySearchText: "没有匹配选项",
    disabled: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string[]];
}>();

const selectedSet = computed(() => new Set(props.modelValue));
const selectedLabels = computed(() =>
  props.options
    .filter((option) => selectedSet.value.has(option.value))
    .map((option) => option.label),
);
const selectedText = computed(() => selectedLabels.value.join(", "));

function toggleOption(option: MultiSelectOption): void {
  if (option.disabled) return;
  const next = new Set(props.modelValue);
  if (next.has(option.value)) next.delete(option.value);
  else next.add(option.value);
  emit(
    "update:modelValue",
    props.options.filter((item) => next.has(item.value)).map((item) => item.value),
  );
}
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
} = useSelectDropdown<MultiSelectOption>({
  options: optionSource,
  searchText: (option) => `${option.label} ${option.value} ${option.description ?? ""}`,
  isSelected: (option) => selectedSet.value.has(option.value),
  onSelect: toggleOption,
  disabled: () => props.disabled,
  closeOnSelect: false,
  optionSelector: ".multi-select-option",
});
</script>

<template>
  <div ref="wrapperRef" class="app-multi-select-wrap">
    <div
      ref="triggerRef"
      class="app-multi-select"
      :class="{ disabled: props.disabled }"
      role="combobox"
      tabindex="0"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-label="ariaLabel"
      @click="toggleDropdown"
      @keydown="onTriggerKeydown"
    >
      <span class="app-multi-select-value" :class="{ placeholder: !selectedText }">
        {{ selectedText || placeholder }}
      </span>
      <span v-if="modelValue.length" class="app-multi-select-count">{{ modelValue.length }}</span>
      <span class="app-multi-select-chevron" :class="{ open }" aria-hidden="true">⌄</span>
    </div>

    <div v-if="open" class="multi-select-dropdown">
      <input
        ref="searchInputRef"
        v-model="searchQuery"
        class="multi-select-search"
        type="search"
        :placeholder="searchPlaceholder"
        :aria-label="searchPlaceholder"
        @keydown="onSearchKeydown"
      />
      <div ref="listRef" class="multi-select-options" role="listbox" aria-multiselectable="true">
        <button
          v-for="(option, index) in filteredOptions"
          :key="option.value"
          type="button"
          class="multi-select-option"
          :class="{
            highlighted: index === highlightIndex,
            selected: selectedSet.has(option.value),
          }"
          :data-value="option.value"
          :disabled="option.disabled"
          role="option"
          :aria-selected="selectedSet.has(option.value)"
          @click.stop="selectOption(option)"
          @mouseenter="!option.disabled && (highlightIndex = index)"
        >
          <img v-if="option.avatarUrl" class="multi-select-avatar" :src="option.avatarUrl" alt="" />
          <span
            v-else-if="option.color"
            class="multi-select-swatch"
            :style="{ backgroundColor: option.color }"
            aria-hidden="true"
          />
          <span class="multi-select-check" aria-hidden="true" />
          <span class="multi-select-option-copy">
            <span>{{ option.label }}</span>
            <small v-if="option.description">{{ option.description }}</small>
          </span>
        </button>
        <div v-if="filteredOptions.length === 0" class="multi-select-empty">
          {{ searchQuery ? emptySearchText : emptyText }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app-multi-select-wrap {
  position: relative;
  width: 100%;
}

.app-multi-select {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: 37px;
  width: 100%;
  box-sizing: border-box;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
  color: var(--color-text);
  font-size: 14px;
  cursor: pointer;
  user-select: none;
}

.app-multi-select:hover:not(.disabled),
.app-multi-select[aria-expanded="true"] {
  border-color: var(--color-primary-border);
}

.app-multi-select:focus-visible {
  outline: 2px solid transparent;
  outline-offset: 0;
  border-color: var(--color-focus);
  box-shadow: var(--shadow-control-focus);
}

.app-multi-select.disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.app-multi-select-value {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-multi-select-value.placeholder {
  color: var(--color-text-tertiary);
}

.app-multi-select-count {
  flex: none;
  min-width: 20px;
  padding: 1px 5px;
  border-radius: var(--radius-sm);
  background: var(--color-control-selected);
  color: var(--color-primary);
  font-family: var(--font-mono);
  font-size: 11px;
  text-align: center;
}

.app-multi-select-chevron {
  flex: none;
  color: var(--color-text-tertiary);
  font-size: 16px;
  line-height: 1;
  transition: transform var(--transition-fast);
}

.app-multi-select-chevron.open {
  transform: rotate(180deg);
}

.multi-select-dropdown {
  position: absolute;
  z-index: 50;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  overflow: hidden;
  padding: 4px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface);
  box-shadow: var(--shadow-lg);
}

.multi-select-search {
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

.multi-select-search:focus-visible {
  outline: 2px solid transparent;
  outline-offset: 0;
  border-color: var(--color-focus);
  background: var(--color-surface);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.multi-select-options {
  max-height: 220px;
  overflow-y: auto;
}

.multi-select-option {
  display: flex;
  align-items: center;
  width: 100%;
  gap: var(--space-2);
  padding: 7px 8px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
}

.multi-select-swatch {
  display: block;
  flex: none;
  width: 10px;
  height: 10px;
  border: 1px solid var(--color-border);
  border-radius: 50%;
}

.multi-select-avatar {
  display: block;
  flex: none;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  object-fit: cover;
}

.multi-select-option:hover,
.multi-select-option.highlighted {
  background: var(--color-control-highlight);
}

.multi-select-option.selected {
  background: var(--color-control-selected);
}

.multi-select-option:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.multi-select-check {
  display: grid;
  flex: none;
  width: 16px;
  height: 16px;
  place-items: center;
  border: 1px solid var(--color-border-strong, var(--color-border));
  border-radius: var(--radius-sm);
}

.multi-select-check::after {
  content: "";
  width: 4px;
  height: 7px;
  border: solid #ffffff;
  border-width: 0 1.5px 1.5px 0;
  opacity: 0;
  transform: translateY(-1px) rotate(45deg);
}

.multi-select-option-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 1px;
}

.multi-select-option-copy small {
  overflow: hidden;
  color: var(--color-text-tertiary);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.multi-select-option.selected .multi-select-check {
  border-color: var(--color-focus);
  background: var(--color-focus);
}

.multi-select-option.selected .multi-select-check::after {
  opacity: 1;
}

.multi-select-empty {
  padding: var(--space-3);
  color: var(--color-text-tertiary);
  font-size: 12px;
  text-align: center;
}
</style>
