<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from "vue";

type SelectOption = { value: string; label: string; disabled?: boolean };

const props = withDefaults(
  defineProps<{
    id?: string;
    modelValue: string;
    options: SelectOption[];
    placeholder?: string;
    size?: "sm" | "md";
    ariaLabel?: string;
  }>(),
  {
    placeholder: "请选择",
    size: "md",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const open = ref(false);
const triggerRef = ref<HTMLElement | null>(null);
const listRef = ref<HTMLElement | null>(null);
const highlightIndex = ref(-1);

const selectedLabel = computed(() => {
  const match = props.options.find((o) => o.value === props.modelValue);
  return match ? match.label : "";
});

function firstEnabledIndex() {
  return props.options.findIndex((option) => !option.disabled);
}

function findEnabledIndex(start: number, direction: 1 | -1) {
  let index = start;
  while (index >= 0 && index < props.options.length) {
    if (!props.options[index].disabled) return index;
    index += direction;
  }
  return -1;
}

function toggle() {
  open.value = !open.value;
  if (open.value) {
    const selectedIndex = props.options.findIndex((o) => o.value === props.modelValue);
    highlightIndex.value =
      selectedIndex >= 0 && !props.options[selectedIndex].disabled
        ? selectedIndex
        : firstEnabledIndex();
  }
}

function select(option: SelectOption) {
  if (option.disabled) return;
  emit("update:modelValue", option.value);
  open.value = false;
}

function onKeydown(e: KeyboardEvent) {
  if (!open.value) {
    if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown") {
      e.preventDefault();
      open.value = true;
      const selectedIndex = props.options.findIndex((o) => o.value === props.modelValue);
      highlightIndex.value =
        selectedIndex >= 0 && !props.options[selectedIndex].disabled
          ? selectedIndex
          : firstEnabledIndex();
    }
    return;
  }

  switch (e.key) {
    case "Escape":
      open.value = false;
      break;
    case "ArrowDown":
      e.preventDefault();
      highlightIndex.value = findEnabledIndex(highlightIndex.value + 1, 1);
      break;
    case "ArrowUp":
      e.preventDefault();
      highlightIndex.value = findEnabledIndex(highlightIndex.value - 1, -1);
      break;
    case "Enter":
    case " ":
      e.preventDefault();
      if (highlightIndex.value >= 0 && highlightIndex.value < props.options.length) {
        select(props.options[highlightIndex.value]);
      }
      break;
  }
}

function onClickOutside(e: MouseEvent) {
  if (!open.value) return;
  const target = e.target as Node;
  if (triggerRef.value && !triggerRef.value.contains(target)) {
    open.value = false;
  }
}

watch(highlightIndex, async () => {
  await nextTick();
  const el = listRef.value?.children[highlightIndex.value] as HTMLElement | undefined;
  if (el && typeof el.scrollIntoView === "function") {
    el.scrollIntoView({ block: "nearest" });
  }
});

onMounted(() => {
  document.addEventListener("click", onClickOutside, true);
});

onUnmounted(() => {
  document.removeEventListener("click", onClickOutside, true);
});
</script>

<template>
  <div class="app-select-wrap" :class="[size === 'sm' ? 'app-select-wrap-sm' : '']">
    <div
      ref="triggerRef"
      :id="id"
      class="app-select"
      tabindex="0"
      role="combobox"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-label="ariaLabel"
      @click="toggle"
      @keydown="onKeydown"
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

    <div v-if="open" ref="listRef" class="dropdown-panel" role="listbox">
      <button
        v-for="(opt, i) in options"
        :key="opt.value"
        :data-value="opt.value"
        class="dropdown-option"
        :class="{ selected: opt.value === modelValue }"
        :disabled="opt.disabled"
        role="option"
        :aria-selected="opt.value === modelValue"
        @click.stop="select(opt)"
        @mouseenter="!opt.disabled && (highlightIndex = i)"
        type="button"
      >
        {{ opt.label }}
      </button>
      <div v-if="options.length === 0" class="dropdown-empty">无选项</div>
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

.app-select:focus {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px var(--color-primary-light);
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
  max-height: 240px;
  overflow-y: auto;
  padding: 4px;
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

.dropdown-option.selected {
  background: var(--color-primary-light);
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
