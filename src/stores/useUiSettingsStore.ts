import { defineStore } from "pinia";
import { ref } from "vue";

const DIFF_SYNC_SCROLL_KEY = "mergebeacon:diff-sync-scroll";
const DIFF_SIDEBAR_EXPANDED_KEY = "mergebeacon:diff-sidebar-expanded";

function readBooleanSetting(key: string, defaultValue: boolean): boolean {
  try {
    const value = localStorage.getItem(key);
    if (value === null) return defaultValue;
    return value !== "false";
  } catch {
    return defaultValue;
  }
}

function writeBooleanSetting(key: string, value: boolean): void {
  try {
    localStorage.setItem(key, String(value));
  } catch {
    // Hardened webviews may disable storage; the setting remains valid for this session.
  }
}

export const useUiSettingsStore = defineStore("ui-settings", () => {
  const isDiffSyncScrollEnabled = ref(readBooleanSetting(DIFF_SYNC_SCROLL_KEY, true));
  const isDiffSidebarExpanded = ref(readBooleanSetting(DIFF_SIDEBAR_EXPANDED_KEY, false));

  function setDiffSyncScrollEnabled(enabled: boolean): void {
    isDiffSyncScrollEnabled.value = enabled;
    writeBooleanSetting(DIFF_SYNC_SCROLL_KEY, enabled);
  }

  function setDiffSidebarExpanded(expanded: boolean): void {
    isDiffSidebarExpanded.value = expanded;
    writeBooleanSetting(DIFF_SIDEBAR_EXPANDED_KEY, expanded);
  }

  return {
    isDiffSyncScrollEnabled,
    isDiffSidebarExpanded,
    setDiffSyncScrollEnabled,
    setDiffSidebarExpanded,
  };
});
