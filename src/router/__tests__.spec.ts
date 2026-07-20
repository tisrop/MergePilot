import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authCheck, authHasToken } from "@/api";
import router from "@/router";
import { useAuthStore } from "@/stores/useAuthStore";

const storage = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

vi.mock("@/api", () => ({
  authCheck: vi.fn(),
  authHasToken: vi.fn(),
}));

describe("登录路由守卫", () => {
  beforeEach(async () => {
    storage.clear();
    setActivePinia(createPinia());
    vi.clearAllMocks();
    await router.push("/settings");
  });

  it("显式点击未登录平台时直接进入登录页，不被持久化 Token 恢复拦截", async () => {
    vi.mocked(authHasToken).mockResolvedValue(true);
    vi.mocked(authCheck).mockResolvedValue({ id: 1, login: "octocat", name: "", avatar_url: "" });

    await router.push({ path: "/login", query: { platform: "github" } });

    expect(router.currentRoute.value.path).toBe("/login");
    expect(useAuthStore().activePlatform).toBe("github");
    expect(authHasToken).not.toHaveBeenCalled();
    expect(authCheck).not.toHaveBeenCalled();
  });

  it("内存中已登录的平台仍从登录页返回工作台", async () => {
    const store = useAuthStore();
    store.platforms.github = {
      user: { id: 1, login: "octocat", name: "", avatar_url: "" },
      isLoggedIn: true,
    };

    await router.push({ path: "/login", query: { platform: "github" } });

    expect(router.currentRoute.value.path).toBe("/pr");
  });

  it("进入创建页时使用路由中的平台上下文", async () => {
    const store = useAuthStore();
    store.setActivePlatform("github");
    store.platforms.gitee = {
      user: { id: 2, login: "gitee-user", name: "", avatar_url: "" },
      isLoggedIn: true,
    };

    await router.push({ name: "pr-new", params: { platform: "gitee" } });

    expect(router.currentRoute.value.name).toBe("pr-new");
    expect(store.activePlatform).toBe("gitee");
  });

  it("旧创建页地址会规范化到当前平台路径", async () => {
    const store = useAuthStore();
    store.setActivePlatform("gitee");
    store.platforms.gitee.isLoggedIn = true;

    await router.push("/pr/new");

    expect(router.currentRoute.value.fullPath).toBe("/pr/new/gitee");
  });
});
