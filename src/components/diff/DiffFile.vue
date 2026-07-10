<script setup lang="ts">
import type { PrFile } from "@/types";
import DiffLine from "./DiffLine.vue";

const props = defineProps<{
  file: PrFile;
}>();

interface ParsedLine {
  type: "add" | "remove" | "context" | "header";
  content: string;
  oldLine: number | null;
  newLine: number | null;
}

function parsePatch(patch: string): ParsedLine[] {
  if (!patch) return [];

  const lines = patch.split("\n");
  const result: ParsedLine[] = [];
  let oldLineNum = 0;
  let newLineNum = 0;

  for (const line of lines) {
    if (line.startsWith("@@")) {
      result.push({ type: "header", content: line, oldLine: null, newLine: null });
      // Parse hunk header for line numbers
      const match = line.match(/@@ -(\d+),\d+ \+(\d+),\d+ @@/);
      if (match) {
        oldLineNum = parseInt(match[1]) - 1;
        newLineNum = parseInt(match[2]) - 1;
      }
    } else if (line.startsWith("+")) {
      newLineNum++;
      result.push({ type: "add", content: line, oldLine: null, newLine: newLineNum });
    } else if (line.startsWith("-")) {
      oldLineNum++;
      result.push({ type: "remove", content: line, oldLine: oldLineNum, newLine: null });
    } else {
      oldLineNum++;
      newLineNum++;
      result.push({ type: "context", content: line, oldLine: oldLineNum, newLine: newLineNum });
    }
  }

  return result;
}

const parsedLines = parsePatch(props.file.patch);
</script>

<template>
  <div class="diff-file">
    <div class="file-header" :class="`status-${file.status}`">
      <span class="file-icon">
        {{ file.status === "added" ? "+" : file.status === "removed" ? "-" : "~" }}
      </span>
      <span class="file-name">{{ file.filename }}</span>
      <span class="file-stats">
        <span class="add">+{{ file.additions }}</span>
        <span class="del">-{{ file.deletions }}</span>
      </span>
    </div>

    <div class="file-diff">
      <DiffLine
        v-for="(line, idx) in parsedLines"
        :key="idx"
        :line="line"
      />
    </div>
  </div>
</template>

<style scoped>
.diff-file {
  border-bottom: 1px solid var(--color-border);
}

.diff-file:last-child {
  border-bottom: none;
}

.file-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  background: #f6f8fa;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.file-icon {
  width: 18px;
  text-align: center;
}

.status-added .file-icon { color: var(--color-success); }
.status-removed .file-icon { color: var(--color-danger); }
.status-modified .file-icon { color: var(--color-warning); }
.status-renamed .file-icon { color: var(--color-warning); }

.file-stats {
  margin-left: auto;
  font-size: 12px;
  font-family: monospace;
}

.file-stats .add { color: var(--color-success); }
.file-stats .del { color: var(--color-danger); }

.file-diff {
  font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
  font-size: 12px;
  line-height: 1.6;
  overflow-x: auto;
}
</style>
