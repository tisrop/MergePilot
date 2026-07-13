import { describe, expect, it } from "vitest";
import {
  assertConsistentVersions,
  parseCargoVersion,
  readProjectVersions,
} from "../version-consistency.mjs";

describe("版本一致性检查", () => {
  it("读取当前三个 manifest 的同一版本", async () => {
    const versions = await readProjectVersions();

    expect(assertConsistentVersions(versions)).toBe("0.3.0");
  });

  it("只读取 Cargo package 段中的版本", () => {
    expect(
      parseCargoVersion(
        '[package]\nname = "app"\nversion = "1.2.3"\n\n[dependencies]\nfoo = "9"\n',
      ),
    ).toBe("1.2.3");
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
});
