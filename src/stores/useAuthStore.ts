import { defineStore } from "pinia";
import { ref, computed, watch } from "vue";
import type { Platform, User } from "@/types";
import { authLogin, authLogout, authCheck } from "@/api";

export const useAuthStore = defineStore("auth", () => {
  // each platform has independent auth state
  const platforms = ref<
    Record<
      Platform,
      {
        user: User | null;
        isLoggedIn: boolean;
      }
    >
  >({
    github: { user: null, isLoggedIn: false },
    gitlab: { user: null, isLoggedIn: false },
    gitee: { user: null, isLoggedIn: false },
  });

  const defaultVisibility: Record<Platform, boolean> = {
    github: true,
    gitlab: true,
    gitee: true,
  };
  const platformVisibility = ref<Record<Platform, boolean>>({
    ...defaultVisibility,
    ...JSON.parse(localStorage.getItem("mergepilot:platformVisibility") ?? "null"),
  });
  watch(platformVisibility, (val) => {
    localStorage.setItem("mergepilot:platformVisibility", JSON.stringify(val));
  }, { deep: true });

  const activePlatform = ref<Platform>("github");

  const activeUser = computed(() => platforms.value[activePlatform.value].user);
  const isLoggedIn = computed(() => platforms.value[activePlatform.value].isLoggedIn);

  async function login(platform: Platform, token: string) {
    const user = await authLogin(platform, token);
    platforms.value[platform] = { user, isLoggedIn: true };
    activePlatform.value = platform;
  }

  async function logout(platform: Platform) {
    await authLogout(platform);
    platforms.value[platform] = { user: null, isLoggedIn: false };
  }

  async function checkAuth(platform: Platform) {
    try {
      const user = await authCheck(platform);
      if (user) {
        platforms.value[platform] = { user, isLoggedIn: true };
      }
    } catch {
      platforms.value[platform] = { user: null, isLoggedIn: false };
    }
  }

  function setActivePlatform(platform: Platform) {
    activePlatform.value = platform;
  }

  function setPlatformVisibility(platform: Platform, visible: boolean) {
    if (!visible) {
      const visibleCount = Object.values(platformVisibility.value).filter(Boolean).length;
      if (visibleCount <= 1) return;
    }
    platformVisibility.value = { ...platformVisibility.value, [platform]: visible };
    if (!visible && activePlatform.value === platform) {
      const firstVisible = (Object.entries(platformVisibility.value) as [Platform, boolean][])
        .find(([, v]) => v)?.[0];
      if (firstVisible) activePlatform.value = firstVisible;
    }
  }

  return {
    platforms,
    platformVisibility,
    activePlatform,
    activeUser,
    isLoggedIn,
    login,
    logout,
    checkAuth,
    setActivePlatform,
    setPlatformVisibility,
  };
});
