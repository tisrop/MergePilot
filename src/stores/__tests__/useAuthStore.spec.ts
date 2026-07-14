import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authCheck, authHasToken, authLogin, authLogout } from "@/api";
import { useAuthStore } from "@/stores/useAuthStore";

const storage = new Map<string, string>();
let storageReadError = false;
let storageWriteError = false;
vi.stubGlobal("localStorage", {
  getItem: (key: string) => {
    if (storageReadError) throw new DOMException("storage denied", "SecurityError");
    return storage.get(key) ?? null;
  },
  setItem: (key: string, value: string) => {
    if (storageWriteError) throw new DOMException("storage denied", "SecurityError");
    storage.set(key, value);
  },
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

vi.mock("@/api", () => ({
  authLogin: vi.fn(),
  authLogout: vi.fn(),
  authCheck: vi.fn(),
  authHasToken: vi.fn(),
}));

const user = { id: 1, login: "saved-user", name: "Saved User", avatar_url: "" };

describe("useAuthStore session restore", () => {
  beforeEach(() => {
    storage.clear();
    storageReadError = false;
    storageWriteError = false;
    setActivePinia(createPinia());
  });

  it("本地存储不可用时使用默认平台状态且保持会话可操作", () => {
    storageReadError = true;
    storageWriteError = true;

    const store = useAuthStore();
    store.setActivePlatform("gitlab");
    store.setPlatformVisibility("gitee", false);

    expect(store.activePlatform).toBe("gitlab");
    expect(store.platformVisibility).toEqual({ github: true, gitlab: true, gitee: false });
  });

  it("忽略损坏或非法的平台可见性持久化数据", () => {
    storage.set("mergebeacon:platformVisibility", "{invalid-json");
    expect(useAuthStore().platformVisibility).toEqual({ github: true, gitlab: true, gitee: true });

    setActivePinia(createPinia());
    storage.set(
      "mergebeacon:platformVisibility",
      JSON.stringify({ github: false, gitlab: false, gitee: false, unknown: true }),
    );
    expect(useAuthStore().platformVisibility).toEqual({ github: true, gitlab: true, gitee: true });
  });

  it("迁移旧品牌的本地平台设置", () => {
    storage.set("mergepilot:activePlatform", "gitlab");
    storage.set(
      "mergepilot:platformVisibility",
      JSON.stringify({ github: true, gitlab: false, gitee: true }),
    );

    const store = useAuthStore();

    expect(store.activePlatform).toBe("gitlab");
    expect(store.platformVisibility.gitlab).toBe(false);
    expect(storage.get("mergebeacon:activePlatform")).toBe("gitlab");
    expect(storage.has("mergepilot:activePlatform")).toBe(false);
    expect(storage.has("mergepilot:platformVisibility")).toBe(false);
  });

  it("优先恢复上次使用的平台", async () => {
    storage.set("mergebeacon:activePlatform", "gitlab");
    vi.mocked(authHasToken).mockResolvedValue(true);
    vi.mocked(authCheck).mockResolvedValue(user);
    const store = useAuthStore();

    expect(await store.restoreSession()).toBe(true);
    expect(authHasToken).toHaveBeenCalledWith("gitlab");
    expect(store.activePlatform).toBe("gitlab");
    expect(store.activeUser?.login).toBe("saved-user");
  });

  it("上次平台无 Token 时恢复其他已登录平台", async () => {
    storage.set("mergebeacon:activePlatform", "github");
    vi.mocked(authHasToken).mockImplementation(async (platform) => platform === "gitee");
    vi.mocked(authCheck).mockResolvedValue(user);
    const store = useAuthStore();

    expect(await store.restoreSession()).toBe(true);
    expect(store.activePlatform).toBe("gitee");
    expect(store.platforms.gitee.isLoggedIn).toBe(true);
    expect(storage.get("mergebeacon:activePlatform")).toBe("gitee");
  });
  it("指定未登录平台时不回退到其他已登录平台", async () => {
    storage.set("mergebeacon:activePlatform", "gitee");
    vi.mocked(authHasToken).mockImplementation(async (platform) => platform === "github");
    vi.mocked(authCheck).mockResolvedValue(user);
    const store = useAuthStore();
    store.platforms.github = { user, isLoggedIn: true };

    expect(await store.restorePlatformSession("gitee")).toBe(false);
    expect(authHasToken).toHaveBeenCalledWith("gitee");
    expect(authCheck).not.toHaveBeenCalledWith("github");
    expect(store.activePlatform).toBe("gitee");
  });

  it("登录状态按平台隔离，登出一个平台不影响其他平台", async () => {
    const githubUser = { ...user, id: 1, login: "github-user" };
    const gitlabUser = { ...user, id: 2, login: "gitlab-user" };
    vi.mocked(authLogin)
      .mockResolvedValueOnce({ user: githubUser, credential_storage: "system_keyring" })
      .mockResolvedValueOnce({ user: gitlabUser, credential_storage: "encrypted_file" });
    vi.mocked(authLogout).mockResolvedValue(undefined);
    const store = useAuthStore();

    await store.login("github", "github-token");
    await store.login("gitlab", "gitlab-token");
    await store.logout("github");

    expect(store.platforms.github).toEqual({ user: null, isLoggedIn: false });
    expect(store.platforms.gitlab).toEqual({ user: gitlabUser, isLoggedIn: true });
    expect(store.activePlatform).toBe("gitlab");
    expect(store.activeUser?.login).toBe("gitlab-user");
  });
});
