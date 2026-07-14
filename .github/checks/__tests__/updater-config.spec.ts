import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { assertUpdaterConfig, readUpdaterConfig } from "../updater-config.mjs";

function validConfig() {
  const keyBytes = Buffer.concat([
    Buffer.from("Ed"),
    Buffer.from("a8777e4954b77ff2", "hex"),
    Buffer.alloc(32, 7),
  ]);
  const publicKey = [
    "untrusted comment: minisign public key: F27FB754497E77A8",
    keyBytes.toString("base64"),
    "",
  ].join("\n");

  return {
    bundle: { createUpdaterArtifacts: true },
    plugins: {
      updater: {
        pubkey: Buffer.from(publicKey, "utf8").toString("base64"),
        endpoints: ["https://github.com/tisrop/MergeBeacon/releases/latest/download/latest.json"],
      },
    },
  };
}

describe("updater 配置安全检查", () => {
  it("接受当前 manifest 中的签名更新配置", async () => {
    const config = await readUpdaterConfig();

    expect(() => assertUpdaterConfig(config)).not.toThrow();
  });

  it("拒绝关闭 updater artifacts", () => {
    const config = validConfig();
    config.bundle.createUpdaterArtifacts = false;

    expect(() => assertUpdaterConfig(config)).toThrow("必须启用 bundle.createUpdaterArtifacts");
  });

  it("拒绝替换或追加 updater endpoint", () => {
    const replaced = validConfig();
    replaced.plugins.updater.endpoints = ["https://example.com/latest.json"];
    expect(() => assertUpdaterConfig(replaced)).toThrow("updater endpoint 必须精确配置");

    const appended = validConfig();
    appended.plugins.updater.endpoints.push(
      "https://github.com/tisrop/MergeBeacon/releases/latest/download/fallback.json",
    );
    expect(() => assertUpdaterConfig(appended)).toThrow("updater endpoint 必须精确配置");
  });

  it("拒绝损坏或伪装成公钥的 updater key", () => {
    const malformed = validConfig();
    malformed.plugins.updater.pubkey = "not-base64";
    expect(() => assertUpdaterConfig(malformed)).toThrow("updater 公钥不是规范的 Base64");

    const secretKey = validConfig();
    secretKey.plugins.updater.pubkey = Buffer.from(
      "untrusted comment: minisign secret key\nAAAA\n",
    ).toString("base64");
    expect(() => assertUpdaterConfig(secretKey)).toThrow(
      "updater 公钥必须是 Tauri 生成的 minisign 公钥文件",
    );
  });

  it("拒绝注释 Key ID 与公钥数据不一致", () => {
    const config = validConfig();
    const publicKey = Buffer.from(config.plugins.updater.pubkey, "base64")
      .toString("utf8")
      .replace("F27FB754497E77A8", "0000000000000000");
    config.plugins.updater.pubkey = Buffer.from(publicKey, "utf8").toString("base64");

    expect(() => assertUpdaterConfig(config)).toThrow(
      "minisign 公钥注释中的 Key ID 与公钥数据不一致",
    );
  });

  it("普通 CI 和 Release 都执行配置门禁，Release 额外校验真实签名密钥", async () => {
    const [ciWorkflow, releaseWorkflow] = await Promise.all([
      readFile(resolve(".github/workflows/ci.yml"), "utf8"),
      readFile(resolve(".github/workflows/release.yml"), "utf8"),
    ]);

    expect(ciWorkflow).toContain("node .github/checks/updater-config.mjs");
    expect(releaseWorkflow).toContain("node .github/checks/updater-config.mjs");
    expect(releaseWorkflow).toContain("name: Verify updater signing key");
    expect(releaseWorkflow).toContain("npm run tauri -- signer sign");
  });
});
