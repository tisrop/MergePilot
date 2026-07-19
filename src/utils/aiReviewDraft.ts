import type { StandardPatchFile } from "@/types";

export interface DraftPosition {
  path: string;
  startLine: number | null;
  endLine: number | null;
}

export function draftPositionIsCurrent(
  draft: DraftPosition,
  patches: StandardPatchFile[],
): boolean {
  if (!draft.path || !draft.endLine) return true;
  const patch = patches.find(
    (candidate) =>
      candidate.filename === draft.path ||
      candidate.new_path === draft.path ||
      candidate.old_path === draft.path,
  );
  if (!patch || patch.content_kind !== "text") return false;
  const startLine = draft.startLine ?? draft.endLine;
  const newLines = new Set(
    patch.hunks.flatMap((hunk) =>
      hunk.lines.flatMap((line) => (line.new_line === null ? [] : [line.new_line])),
    ),
  );
  return newLines.has(startLine) && newLines.has(draft.endLine);
}
