import { defineStore } from "pinia";
import { ref, computed } from "vue";
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

  return {
    platforms,
    activePlatform,
    activeUser,
    isLoggedIn,
    login,
    logout,
    checkAuth,
    setActivePlatform,
  };
});
