import { describe, expect, it } from "vitest";
import {
  assertConsistentVersions,
  assertReleaseTag,
  parseCargoVersion,
  readProjectVersions,
} from "../version-consistency.mjs";

const FIXTURE_VERSION = "1.2.3";
const FIXTURE_PRERELEASE_VERSION = "1.2.3-rc.1";

describe("版本一致性检查", () => {
  it("读取当前三个 manifest 的同一版本", async () => {
    const versions = await readProjectVersions();

    expect(assertConsistentVersions(versions)).toBe(versions["package.json"]);
  });

  it("只读取 Cargo package 段中的版本", () => {
    expect(
      parseCargoVersion(
        '[package]\nname = "app"\nversion = "1.2.3"\n\n[dependencies]\nfoo = "9"\n',
      ),
    ).toBe(FIXTURE_VERSION);
  });

  it("报告每个不一致的文件和值", () => {
    expect(() =>
      assertConsistentVersions({
        "package.json": "1.0.0",
        "src-tauri/Cargo.toml": "1.0.0",
        "src-tauri/tauri.conf.json": "0.9.0",
      }),
    ).toThrow(/src-tauri\/tauri\.conf\.json=0\.9\.0/);
  });

  it("缺少版本字段时明确失败", () => {
    expect(() =>
      assertConsistentVersions({
        "package.json": "1.0.0",
        "src-tauri/Cargo.toml": null,
        "src-tauri/tauri.conf.json": "1.0.0",
      }),
    ).toThrow("无法读取版本：src-tauri/Cargo.toml");
  });

  it("接受与 manifest 一致的正式和预发布标签", () => {
    expect(assertReleaseTag("v1.2.3", FIXTURE_VERSION)).toBe(FIXTURE_VERSION);
    expect(assertReleaseTag("v1.2.3-rc.1", FIXTURE_PRERELEASE_VERSION)).toBe(
      FIXTURE_PRERELEASE_VERSION,
    );
  });

  it("发布标签与 manifest 不一致时明确失败", () => {
    expect(() => assertReleaseTag("v1.2.4", "1.2.3")).toThrow(
      "发布标签与应用版本不一致：tag=v1.2.4，manifest=1.2.3",
    );
  });

  it("拒绝缺少 v 前缀或不规范的发布标签", () => {
    expect(() => assertReleaseTag("1.2.3", "1.2.3")).toThrow("发布标签格式无效");
    expect(() => assertReleaseTag("v01.2.3", "01.2.3")).toThrow("发布标签格式无效");
    expect(() => assertReleaseTag("v1.2", "1.2")).toThrow("发布标签格式无效");
  });
});
