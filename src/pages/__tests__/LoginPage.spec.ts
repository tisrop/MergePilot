import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import LoginPage from "../LoginPage.vue";
import { authLogin } from "@/api";
import { useAuthStore } from "@/stores/useAuthStore";

const replace = vi.fn().mockResolvedValue(undefined);
const routeQuery: { platform?: string } = {};
const storage = new Map<string, string>();

vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

vi.mock("vue-router", async (importOriginal) => {
  const original = await importOriginal<typeof import("vue-router")>();
  return {
    ...original,
    useRoute: () => ({ query: routeQuery }),
    useRouter: () => ({ replace }),
  };
});

vi.mock("@tauri-apps/plugin-shell", () => ({ open: vi.fn() }));
vi.mock("@/api", () => ({ authLogin: vi.fn() }));

describe("LoginPage", () => {
  beforeEach(() => {
    storage.clear();
    delete routeQuery.platform;
    replace.mockClear();
    setActivePinia(createPinia());
    vi.mocked(authLogin).mockResolvedValue({
      user: { id: 1, login: "octocat", name: "Octocat", avatar_url: "" },
      credential_storage: "system_keyring",
    });
  });

  it("认证成功后立即更新状态并等待跳转到 PR 页面", async () => {
    const wrapper = mount(LoginPage, {
      global: {
        stubs: {
          AppSelect: true,
          RouterLink: true,
        },
      },
    });
    await wrapper.get('input[type="password"]').setValue("token");
    await wrapper.get("button.login-btn").trigger("click");
    await flushPromises();

    const auth = useAuthStore();
    expect(auth.platforms.github.isLoggedIn).toBe(true);
    expect(auth.platforms.github.user?.login).toBe("octocat");
    expect(replace).toHaveBeenCalledWith("/pr");
    expect(wrapper.get("button.login-btn").text()).toContain("登录");
  });
  it("从侧栏进入时使用查询参数指定的待登录平台", async () => {
    routeQuery.platform = "gitee";
    const wrapper = mount(LoginPage, {
      global: {
        stubs: {
          AppSelect: true,
          RouterLink: true,
        },
      },
    });
    await wrapper.get('input[type="password"]').setValue("gitee-token");
    await wrapper.get("button.login-btn").trigger("click");
    await flushPromises();

    expect(authLogin).toHaveBeenCalledWith("gitee", "gitee-token", undefined);
    expect(useAuthStore().activePlatform).toBe("gitee");
    expect(replace).toHaveBeenCalledWith("/pr");
  });
});
