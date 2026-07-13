import { defineStore } from "pinia";
import { ref, computed, watch } from "vue";
import type { Platform, User } from "@/types";
import { authLogin, authLogout, authCheck, authHasToken } from "@/api";

const PLATFORM_VISIBILITY_KEY = "mergepilot:platformVisibility";
const ACTIVE_PLATFORM_KEY = "mergepilot:activePlatform";
const PLATFORMS: Platform[] = ["github", "gitlab", "gitee"];
const DEFAULT_VISIBILITY: Record<Platform, boolean> = {
  github: true,
  gitlab: true,
  gitee: true,
};

function readStorage(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Keep the in-memory session usable when persistence is unavailable.
  }
}

function loadPlatformVisibility(): Record<Platform, boolean> {
  const visibility = { ...DEFAULT_VISIBILITY };
  const stored = readStorage(PLATFORM_VISIBILITY_KEY);
  if (!stored) return visibility;

  try {
    const parsed: unknown = JSON.parse(stored);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return visibility;
    for (const platform of PLATFORMS) {
      const value = (parsed as Record<string, unknown>)[platform];
      if (typeof value === "boolean") visibility[platform] = value;
    }
    return Object.values(visibility).some(Boolean) ? visibility : { ...DEFAULT_VISIBILITY };
  } catch {
    return visibility;
  }
}

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

  const platformVisibility = ref<Record<Platform, boolean>>(loadPlatformVisibility());
  watch(
    platformVisibility,
    (val) => {
      writeStorage(PLATFORM_VISIBILITY_KEY, JSON.stringify(val));
    },
    { deep: true },
  );

  const storedActivePlatform = readStorage(ACTIVE_PLATFORM_KEY);
  const activePlatform = ref<Platform>(
    storedActivePlatform === "github" ||
      storedActivePlatform === "gitlab" ||
      storedActivePlatform === "gitee"
      ? storedActivePlatform
      : "github",
  );
  watch(activePlatform, (value) => {
    writeStorage(ACTIVE_PLATFORM_KEY, value);
  });

  const activeUser = computed(() => platforms.value[activePlatform.value].user);
  const isLoggedIn = computed(() => platforms.value[activePlatform.value].isLoggedIn);

  async function login(platform: Platform, token: string) {
    const result = await authLogin(platform, token);
    platforms.value[platform] = { user: result.user, isLoggedIn: true };
    activePlatform.value = platform;
  }

  async function logout(platform: Platform) {
    await authLogout(platform);
    platforms.value[platform] = { user: null, isLoggedIn: false };
  }

  async function checkAuth(platform: Platform) {
    try {
      const user = await authCheck(platform);
      platforms.value[platform] = user
        ? { user, isLoggedIn: true }
        : { user: null, isLoggedIn: false };
    } catch {
      platforms.value[platform] = { user: null, isLoggedIn: false };
    }
  }

  async function restorePlatformSession(platform: Platform): Promise<boolean> {
    try {
      if (!(await authHasToken(platform))) {
        platforms.value[platform] = { user: null, isLoggedIn: false };
        return false;
      }
      await checkAuth(platform);
      if (platforms.value[platform].isLoggedIn) {
        activePlatform.value = platform;
        return true;
      }
    } catch {
      platforms.value[platform] = { user: null, isLoggedIn: false };
    }
    return false;
  }

  async function restoreSession(preferredPlatform?: Platform): Promise<boolean> {
    const candidates: Platform[] = [];
    const restoreOrder: Array<Platform | undefined> = [
      preferredPlatform,
      activePlatform.value,
      "github",
      "gitlab",
      "gitee",
    ];

    for (const candidate of restoreOrder) {
      if (candidate && !candidates.includes(candidate)) {
        candidates.push(candidate);
      }
    }

    for (const candidate of candidates) {
      if (await restorePlatformSession(candidate)) {
        return true;
      }
    }
    return false;
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
      const firstVisible = (Object.entries(platformVisibility.value) as [Platform, boolean][]).find(
        ([, v]) => v,
      )?.[0];
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
    restorePlatformSession,
    restoreSession,
    setActivePlatform,
    setPlatformVisibility,
  };
});
