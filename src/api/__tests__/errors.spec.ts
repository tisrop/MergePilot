import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, normalizeApiError, prDetail } from "@/api";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  isTauri: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-shell", () => ({
  open: vi.fn(),
}));

describe("IPC errors", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("preserves structured command error metadata", async () => {
    const payload = {
      code: "rate_limited",
      message: "代码平台请求过于频繁，请稍后重试",
      retryable: true,
      request_id: "err-0000018f12345678-0000000000000001",
      http_status: 429,
    };
    invokeMock.mockImplementation((command: string) =>
      command === "error_log_record" ? Promise.resolve() : Promise.reject(payload),
    );

    const promise = prDetail("github", "owner", "repo", 42);

    await expect(promise).rejects.toMatchObject({
      name: "ApiError",
      code: "rate_limited",
      message: "代码平台请求过于频繁，请稍后重试",
      retryable: true,
      requestId: "err-0000018f12345678-0000000000000001",
      httpStatus: 429,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(
      2,
      "error_log_record",
      expect.objectContaining({
        record: expect.objectContaining({
          command: "pr_detail",
          requestId: "err-0000018f12345678-0000000000000001",
          code: "rate_limited",
          retryable: true,
          httpStatus: 429,
        }),
      }),
    );
    expect(invokeMock.mock.calls[1]?.[1]?.record).not.toHaveProperty("message");
  });

  it("keeps the original command error when local error logging fails", async () => {
    invokeMock.mockRejectedValue({
      code: "network",
      message: "无法连接到远端服务，请检查网络",
      retryable: true,
    });

    await expect(prDetail("github", "owner", "repo", 42)).rejects.toMatchObject({
      code: "network",
      message: "无法连接到远端服务，请检查网络",
    });
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("does not wait for local error logging before rejecting", async () => {
    let finishLogging: (() => void) | undefined;
    invokeMock.mockImplementation((command: string) => {
      if (command === "error_log_record") {
        return new Promise<void>((resolve) => {
          finishLogging = resolve;
        });
      }
      return Promise.reject({
        code: "platform",
        message: "代码平台请求失败",
        retryable: false,
      });
    });

    await expect(prDetail("github", "owner", "repo", 42)).rejects.toMatchObject({
      code: "platform",
      message: "代码平台请求失败",
    });
    expect(finishLogging).toBeTypeOf("function");
    finishLogging?.();
  });

  it("keeps legacy string errors compatible with existing UI", async () => {
    invokeMock.mockRejectedValue("仓库不存在");

    try {
      await prDetail("github", "owner", "repo", 42);
      expect.unreachable("expected command to reject");
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      expect(String(error)).toBe("仓库不存在");
      expect((error as ApiError).code).toBe("unknown");
    }
  });

  it("parses structured JSON strings during mixed-version upgrades", () => {
    const error = normalizeApiError(
      JSON.stringify({
        code: "authentication",
        message: "登录凭据已失效，请重新登录",
        retryable: false,
      }),
    );

    expect(error).toMatchObject({
      code: "authentication",
      message: "登录凭据已失效，请重新登录",
      retryable: false,
    });
  });

  it("rejects malformed payloads without exposing object serialization", () => {
    const error = normalizeApiError({ code: "rate_limited", message: "raw" });

    expect(error.code).toBe("unknown");
    expect(error.message).toBe("操作失败");
  });
});
