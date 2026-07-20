import { describe, expect, it } from "vitest";
import { draftPositionIsCurrent } from "@/utils/aiReviewDraft";
import type { PatchHunk, StandardPatchFile } from "@/types";

function hunk(start: number, end: number): PatchHunk {
  const count = end - start + 1;
  return {
    header: `@@ -${start},${count} +${start},${count} @@`,
    old_start: start,
    old_count: count,
    new_start: start,
    new_count: count,
    section_header: null,
    lines: Array.from({ length: count }, (_, index) => ({
      kind: "context" as const,
      content: "line",
      old_line: start + index,
      new_line: start + index,
    })),
  };
}

function patch(overrides: Partial<StandardPatchFile> = {}): StandardPatchFile {
  return {
    filename: "src/current.ts",
    old_path: "src/current.ts",
    new_path: "src/current.ts",
    status: "modified",
    additions: 0,
    deletions: 0,
    content_kind: "text",
    patch: "",
    hunks: [hunk(1, 20)],
    message: null,
    ...overrides,
  };
}

describe("draftPositionIsCurrent", () => {
  it("允许起止行分别位于相邻 hunk", () => {
    expect(
      draftPositionIsCurrent({ path: "src/current.ts", startLine: 8, endLine: 15 }, [
        patch({ hunks: [hunk(1, 10), hunk(11, 20)] }),
      ]),
    ).toBe(true);
  });

  it("拒绝二进制文件上的行级草稿", () => {
    expect(
      draftPositionIsCurrent({ path: "assets/logo.png", startLine: 1, endLine: 1 }, [
        patch({
          filename: "assets/logo.png",
          old_path: "assets/logo.png",
          new_path: "assets/logo.png",
          content_kind: "binary",
          hunks: [],
        }),
      ]),
    ).toBe(false);
  });

  it("重命名文件使用新旧路径均能匹配当前新文件行号", () => {
    const renamed = patch({
      filename: "src/new-name.ts",
      old_path: "src/old-name.ts",
      new_path: "src/new-name.ts",
      status: "renamed",
      hunks: [hunk(20, 30)],
    });

    expect(
      draftPositionIsCurrent({ path: "src/new-name.ts", startLine: 22, endLine: 24 }, [renamed]),
    ).toBe(true);
    expect(
      draftPositionIsCurrent({ path: "src/old-name.ts", startLine: 22, endLine: 24 }, [renamed]),
    ).toBe(true);
  });

  it("文件或端点行不在当前文本补丁中时拒绝提交", () => {
    expect(
      draftPositionIsCurrent({ path: "src/missing.ts", startLine: 1, endLine: 1 }, [patch()]),
    ).toBe(false);
    expect(
      draftPositionIsCurrent({ path: "src/current.ts", startLine: 2, endLine: 99 }, [patch()]),
    ).toBe(false);
  });
});
