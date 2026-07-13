import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  checkForUpdates,
  downloadAndInstallUpdate,
  listenToUpdateProgress,
  restartAfterUpdate,
} from "@/api";
import { useUpdateStore } from "../useUpdateStore";

const storage = new Map<string, string>();

vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

vi.mock("@/api", () => ({
  checkForUpdates: vi.fn(),
  downloadAndInstallUpdate: vi.fn(),
  listenToUpdateProgress: vi.fn(),
  restartAfterUpdate: vi.fn(),
}));

const noUpdate = {
  current_version: "0.3.5",
  available: false,
  version: null,
  notes: null,
  published_at: null,
};

describe("useUpdateStore", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    storage.clear();
    setActivePinia(createPinia());
    vi.mocked(checkForUpdates).mockReset();
    vi.mocked(downloadAndInstallUpdate).mockReset();
    vi.mocked(listenToUpdateProgress).mockReset();
    vi.mocked(listenToUpdateProgress).mockResolvedValue(() => undefined);
    vi.mocked(restartAfterUpdate).mockReset();
  });

  it("启动后台检查后记录时间并在一天内避免重复请求", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue(noUpdate);
    const store = useUpdateStore();

    await store.maybeCheckForUpdatesInBackground();
    await store.maybeCheckForUpdatesInBackground();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(storage.get("mergepilot:last-update-check")).toBeTruthy();
    expect(store.updateResult).toEqual(noUpdate);
  });

  it("用户关闭自动检查后启动不发起请求", async () => {
    storage.set("mergepilot:auto-update-check", "false");
    const store = useUpdateStore();

    await store.maybeCheckForUpdatesInBackground();

    expect(checkForUpdates).not.toHaveBeenCalled();
  });

  it("后台检查失败不产生阻断错误并恢复检查状态", async () => {
    vi.mocked(checkForUpdates).mockRejectedValue("network unavailable");
    const store = useUpdateStore();

    await store.maybeCheckForUpdatesInBackground();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(store.updateError).toBe("");
    expect(store.isCheckingUpdate).toBe(false);
  });

  it("本地存储不可用时仍可执行后台检查", async () => {
    vi.spyOn(localStorage, "getItem").mockImplementation(() => {
      throw new DOMException("storage denied", "SecurityError");
    });
    vi.spyOn(localStorage, "setItem").mockImplementation(() => {
      throw new DOMException("storage denied", "SecurityError");
    });
    vi.mocked(checkForUpdates).mockResolvedValue(noUpdate);
    const store = useUpdateStore();

    await store.maybeCheckForUpdatesInBackground();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(store.updateResult).toEqual(noUpdate);
  });

  it("下载安装状态由 Store 持有并阻止同一版本重复安装", async () => {
    vi.stubGlobal("crypto", { randomUUID: vi.fn(() => "update-store") });
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.5",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
    });
    let resolveDownload!: () => void;
    const pendingDownload = new Promise<void>((resolve) => {
      resolveDownload = resolve;
    });
    vi.mocked(downloadAndInstallUpdate).mockReturnValue(pendingDownload);
    const unlisten = vi.fn();
    vi.mocked(listenToUpdateProgress).mockResolvedValue(unlisten);
    const store = useUpdateStore();
    await store.checkUpdate();

    const installing = store.installUpdate();
    await Promise.resolve();
    await store.installUpdate();

    expect(store.isInstallingUpdate).toBe(true);
    expect(downloadAndInstallUpdate).toHaveBeenCalledOnce();
    expect(downloadAndInstallUpdate).toHaveBeenCalledWith("update-store", "0.4.0");

    resolveDownload();
    await installing;
    await store.installUpdate();

    expect(store.isInstallingUpdate).toBe(false);
    expect(store.isUpdateInstalled).toBe(true);
    expect(downloadAndInstallUpdate).toHaveBeenCalledOnce();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
