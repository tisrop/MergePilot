<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { usePrStore } from "@/stores/usePrStore";
import { useRepoStore } from "@/stores/useRepoStore";
import { useReviewInboxStore } from "@/stores/useReviewInboxStore";
import { dispatchAppCommand } from "@/types/commands";
import type { Platform, RepoSummary, ReviewInboxItem } from "@/types";

interface PaletteCommand {
  id: string;
  group: string;
  label: string;
  hint: string;
  keywords: string;
  run: () => void | Promise<void>;
}

const platformLabels: Record<Platform, string> = {
  github: "GitHub",
  gitlab: "GitLab",
  gitee: "Gitee",
};
const platforms: Platform[] = ["github", "gitlab", "gitee"];

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const pr = usePrStore();
const repo = useRepoStore();
const inbox = useReviewInboxStore();

const open = ref(false);
const query = ref("");
const selectedIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const dialogRef = ref<HTMLElement | null>(null);
let previousFocus: HTMLElement | null = null;

function repositoryTarget(repository: RepoSummary): { owner: string; repo: string } {
  const fullName =
    repository.fork && repository.parent_full_name
      ? repository.parent_full_name
      : repository.full_name;
  const [owner, ...parts] = fullName.split("/");
  return { owner, repo: parts.join("/") };
}

function selectPlatform(platform: Platform): void {
  auth.setActivePlatform(platform);
  pr.clearContext();
  if (!auth.platforms[platform].isLoggedIn) {
    void router.push({ name: "login", query: { platform } });
    return;
  }
  if (repo.reposCache[platform].length === 0) void repo.fetchRepos(platform);
  void router.push({ name: "pr-list" });
}

function selectRepository(platform: Platform, repository: RepoSummary): void {
  auth.setActivePlatform(platform);
  const target = repositoryTarget(repository);
  repo.setActiveRepo(target.owner, target.repo, platform);
  if (repository.fork) {
    const [forkOwner, ...forkRepoParts] = repository.full_name.split("/");
    repo.setForkContext(
      {
        upstreamFullName: repository.parent_full_name,
        upstreamOwner: repository.parent_owner,
        forkOwner,
        forkRepo: forkRepoParts.join("/"),
      },
      platform,
    );
  } else {
    repo.setForkContext(null, platform);
  }
  pr.clearContext();
  void router.push({ name: "pr-list", query: { _t: Date.now().toString() } });
}

function openPullRequest(
  platform: Platform,
  owner: string,
  repository: string,
  number: number,
): void {
  auth.setActivePlatform(platform);
  repo.setActiveRepo(owner, repository, platform);
  repo.setForkContext(null, platform);
  pr.clearContext();
  void router.push({
    name: "pr-detail",
    params: { platform, owner, repo: repository, number },
  });
}

function inboxCommands(): PaletteCommand[] {
  const commands = new Map<string, PaletteCommand>();
  for (const item of platforms.flatMap((platform) => inbox.itemsByPlatform[platform])) {
    const key = `${item.platform}:${item.repository_full_name}:${item.summary.number}`;
    commands.set(key, pullRequestCommand(item));
  }
  const activeRepo = repo.activeRepos[auth.activePlatform];
  if (activeRepo) {
    for (const summary of pr.list) {
      const key = `${auth.activePlatform}:${activeRepo.owner}/${activeRepo.repo}:${summary.number}`;
      if (!commands.has(key)) {
        commands.set(key, {
          id: `pr:${key}`,
          group: "Pull Request",
          label: `#${summary.number} ${summary.title}`,
          hint: `${platformLabels[auth.activePlatform]} · ${activeRepo.owner}/${activeRepo.repo}`,
          keywords: `${summary.author.login} ${summary.labels.join(" ")}`,
          run: () =>
            openPullRequest(auth.activePlatform, activeRepo.owner, activeRepo.repo, summary.number),
        });
      }
    }
  }
  return [...commands.values()];
}

function pullRequestCommand(item: ReviewInboxItem): PaletteCommand {
  return {
    id: `pr:${item.platform}:${item.repository_full_name}:${item.summary.number}`,
    group: "Pull Request",
    label: `#${item.summary.number} ${item.summary.title}`,
    hint: `${platformLabels[item.platform]} · ${item.repository_full_name}`,
    keywords: `${item.summary.author.login} ${item.relationships.join(" ")}`,
    run: () => openPullRequest(item.platform, item.owner, item.repo, item.summary.number),
  };
}

const commands = computed<PaletteCommand[]>(() => {
  const result: PaletteCommand[] = [
    {
      id: "navigate:inbox",
      group: "导航",
      label: "打开 PR 收件箱",
      hint: "查看待处理评审",
      keywords: "inbox 收件箱 review",
      run: () => void router.push({ name: "review-inbox" }),
    },
    {
      id: "navigate:pr",
      group: "导航",
      label: "打开 Pull Request",
      hint: "当前仓库的 PR 列表",
      keywords: "pr mr pull request merge request",
      run: () => void router.push({ name: "pr-list" }),
    },
    {
      id: "navigate:issue",
      group: "导航",
      label: "打开 Issue",
      hint: "当前仓库的 Issue 列表",
      keywords: "issue 问题",
      run: () => void router.push({ name: "issue-list" }),
    },
    {
      id: "navigate:settings",
      group: "导航",
      label: "打开设置",
      hint: "平台、通知、AI 与更新设置",
      keywords: "settings preference 设置",
      run: () => void router.push({ name: "settings" }),
    },
  ];

  for (const platform of platforms) {
    result.push({
      id: `platform:${platform}`,
      group: "切换平台",
      label: `切换到 ${platformLabels[platform]}`,
      hint: auth.platforms[platform].isLoggedIn ? "已登录" : "需要登录",
      keywords: `${platform} 平台`,
      run: () => selectPlatform(platform),
    });
    for (const repository of repo.reposCache[platform]) {
      result.push({
        id: `repo:${platform}:${repository.id}:${repository.full_name}`,
        group: "仓库",
        label: repository.full_name,
        hint: `${platformLabels[platform]}${repository.private ? " · 私有仓库" : ""}`,
        keywords: `${repository.name} ${repository.owner} ${repository.description}`,
        run: () => selectRepository(platform, repository),
      });
    }
  }

  result.push(...inboxCommands());

  if (route.name === "pr-detail") {
    for (const file of pr.diff?.files ?? []) {
      result.push({
        id: `diff:${file.filename}`,
        group: "Diff 文件",
        label: file.filename,
        hint: `${file.additions} 行新增 · ${file.deletions} 行删除`,
        keywords: `diff ${file.status}`,
        run: () => dispatchAppCommand({ type: "open_diff_file", path: file.filename }),
      });
    }
    result.push(
      {
        id: "review:start-ai",
        group: "评审",
        label: "开始 AI 评审",
        hint: "使用当前 PR Diff 开始评审",
        keywords: "ai review 人工智能",
        run: () => dispatchAppCommand({ type: "start_ai_review" }),
      },
      {
        id: "review:prepare-submit",
        group: "评审",
        label: "提交评审",
        hint: "打开评审意见输入框",
        keywords: "review submit approve comment",
        run: () => dispatchAppCommand({ type: "prepare_review" }),
      },
    );
  }
  return result;
});

const filteredCommands = computed(() => {
  const terms = query.value.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const matched =
    terms.length === 0
      ? commands.value
      : commands.value.filter((command) => {
          const searchable =
            `${command.label} ${command.hint} ${command.group} ${command.keywords}`.toLocaleLowerCase();
          return terms.every((term) => searchable.includes(term));
        });
  const direct = query.value.trim().match(/^(.+)\/([^/#\s]+)\s*#(\d+)$/);
  if (!direct) return matched;
  const number = Number(direct[3]);
  if (!Number.isSafeInteger(number) || number <= 0) return matched;
  return [
    {
      id: `direct-pr:${auth.activePlatform}:${direct[1]}/${direct[2]}:${number}`,
      group: "快速打开",
      label: `打开 ${direct[1]}/${direct[2]} #${number}`,
      hint: platformLabels[auth.activePlatform],
      keywords: "",
      run: () => openPullRequest(auth.activePlatform, direct[1], direct[2], number),
    },
    ...matched,
  ];
});

watch(filteredCommands, () => {
  selectedIndex.value = 0;
});

function openPalette(): void {
  if (open.value) return;
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  open.value = true;
  query.value = "";
  selectedIndex.value = 0;
  for (const platform of platforms) {
    if (auth.platforms[platform].isLoggedIn && repo.reposCache[platform].length === 0) {
      void repo.fetchRepos(platform);
    }
  }
  void nextTick(() => inputRef.value?.focus());
}

function closePalette(): void {
  if (!open.value) return;
  open.value = false;
  void nextTick(() => previousFocus?.focus());
}

async function runCommand(command: PaletteCommand | undefined): Promise<void> {
  if (!command) return;
  closePalette();
  await command.run();
}

function moveSelection(offset: number): void {
  const count = filteredCommands.value.length;
  if (count === 0) return;
  selectedIndex.value = (selectedIndex.value + offset + count) % count;
  void nextTick(() => {
    dialogRef.value
      ?.querySelector<HTMLElement>(`[data-command-index="${selectedIndex.value}"]`)
      ?.scrollIntoView({ block: "nearest" });
  });
}

function handleGlobalKeydown(event: KeyboardEvent): void {
  if (event.isComposing) return;
  if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "k") {
    event.preventDefault();
    openPalette();
    return;
  }
  if (event.key === "Escape" && open.value) {
    event.preventDefault();
    closePalette();
  }
}

function handleDialogKeydown(event: KeyboardEvent): void {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    moveSelection(1);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    moveSelection(-1);
  } else if (event.key === "Enter") {
    event.preventDefault();
    void runCommand(filteredCommands.value[selectedIndex.value]);
  } else if (event.key === "Tab") {
    const focusable = [...(dialogRef.value?.querySelectorAll<HTMLElement>("input, button") ?? [])];
    if (focusable.length === 0) return;
    const current = focusable.indexOf(document.activeElement as HTMLElement);
    const next = event.shiftKey
      ? (current - 1 + focusable.length) % focusable.length
      : (current + 1) % focusable.length;
    event.preventDefault();
    focusable[next].focus();
  }
}

onMounted(() => window.addEventListener("keydown", handleGlobalKeydown));
onUnmounted(() => window.removeEventListener("keydown", handleGlobalKeydown));

defineExpose({ open: openPalette, close: closePalette });
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="command-palette-backdrop" @mousedown.self="closePalette">
      <section
        ref="dialogRef"
        class="command-palette"
        role="dialog"
        aria-modal="true"
        aria-labelledby="command-palette-title"
        @keydown="handleDialogKeydown"
      >
        <h2 id="command-palette-title" class="sr-only">命令面板</h2>
        <div class="command-search">
          <svg
            width="17"
            height="17"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            aria-hidden="true"
          >
            <circle cx="11" cy="11" r="7" />
            <path d="m20 20-4-4" />
          </svg>
          <input
            ref="inputRef"
            v-model="query"
            type="search"
            autocomplete="off"
            aria-label="搜索命令、仓库、PR 或 Diff 文件"
            placeholder="搜索命令、仓库、PR 或 Diff 文件..."
          />
          <kbd>Esc</kbd>
        </div>
        <div class="command-results" role="listbox" aria-label="可用命令">
          <button
            v-for="(command, index) in filteredCommands"
            :key="command.id"
            type="button"
            class="command-item"
            :class="{ selected: selectedIndex === index }"
            role="option"
            :aria-selected="selectedIndex === index"
            :data-command-index="index"
            @mouseenter="selectedIndex = index"
            @click="runCommand(command)"
          >
            <span class="command-copy">
              <strong>{{ command.label }}</strong>
              <small>{{ command.hint }}</small>
            </span>
            <span class="command-group">{{ command.group }}</span>
          </button>
          <p v-if="filteredCommands.length === 0" class="command-empty">没有匹配的命令</p>
        </div>
        <footer class="command-footer">
          <span><kbd>↑</kbd><kbd>↓</kbd> 选择</span>
          <span><kbd>↵</kbd> 执行</span>
          <span><kbd>Ctrl/⌘</kbd><kbd>K</kbd> 打开</span>
        </footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.command-palette-backdrop {
  position: fixed;
  z-index: 1000;
  inset: 0;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: min(14vh, 120px) var(--space-5) var(--space-5);
  background: rgba(13, 18, 32, 0.36);
  backdrop-filter: blur(8px);
}

.command-palette {
  width: min(680px, 100%);
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  background: var(--color-surface);
  box-shadow: var(--shadow-xl);
}

.command-search {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--color-border);
  color: var(--color-text-tertiary);
}

.command-search svg {
  flex: 0 0 auto;
  stroke-width: 1.8;
  stroke-linecap: round;
}

.command-search input {
  min-width: 0;
  flex: 1;
  border: 0;
  background: transparent;
  color: var(--color-text);
  font: inherit;
  font-size: 16px;
}

.command-search:focus-within {
  background: var(--color-control-highlight);
  box-shadow: inset 0 -1px var(--color-focus);
}

.command-search input:focus-visible {
  outline: 2px solid transparent;
  outline-offset: 0;
}

kbd {
  display: inline-flex;
  min-width: 22px;
  height: 22px;
  align-items: center;
  justify-content: center;
  padding: 0 5px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: var(--color-surface-hover);
  color: var(--color-text-tertiary);
  font: 11px var(--font-mono);
  box-shadow: 0 1px 0 var(--color-border);
}

.command-results {
  max-height: min(58vh, 480px);
  overflow-y: auto;
  padding: var(--space-2);
}

.command-item {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: 10px var(--space-3);
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text);
  text-align: left;
}

.command-item:hover,
.command-item.selected {
  border-color: var(--color-primary-border);
  background: var(--color-primary-light);
}

.command-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.command-copy strong,
.command-copy small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-copy strong {
  font-size: 13px;
  font-weight: 600;
}

.command-copy small,
.command-group,
.command-empty,
.command-footer {
  color: var(--color-text-tertiary);
  font-size: 11px;
}

.command-group {
  flex: 0 0 auto;
}

.command-empty {
  margin: 0;
  padding: var(--space-8);
  text-align: center;
}

.command-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-4);
  padding: var(--space-2) var(--space-4);
  border-top: 1px solid var(--color-border);
  background: var(--color-bg);
}

.command-footer span {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  clip-path: inset(50%);
}

@media (max-width: 640px) {
  .command-palette-backdrop {
    padding: var(--space-4);
  }

  .command-group,
  .command-footer {
    display: none;
  }
}

@media (prefers-reduced-motion: no-preference) {
  .command-palette {
    animation: command-palette-enter 120ms ease-out;
  }
}

@keyframes command-palette-enter {
  from {
    opacity: 0;
    transform: translateY(-8px) scale(0.99);
  }
}
</style>
