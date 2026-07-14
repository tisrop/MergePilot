# MergeBeacon — AGENTS.md

Tauri 2 + Vue 3 + Rust 跨平台 PR 评审与 Issue 管理桌面客户端（GitHub/GitLab/Gitee + AI 评审）。

## 代码规范

所有实现与代码评审必须遵循 [`CODE_STANDARDS.md`](CODE_STANDARDS.md)。该文件定义前端、
Rust/Tauri、跨平台语义、异步生命周期、安全、测试和合并门禁；本文件中的项目约束仍同时生效。

所有前端页面、组件和样式实现还必须遵循
[`FRONTEND_STANDARDS.md`](FRONTEND_STANDARDS.md)。该文件定义视觉语言、设计 Token、布局、
组件状态、交互、可访问性和视觉验收门禁。

## 开发者命令

```bash
npm run dev              # 仅前端 Vite dev server（port 1420）
npm run tauri -- dev     # 完整 Tauri 桌面应用（dev server + 原生窗口）
npm run build            # vue-tsc --noEmit 类型检查 → vite build
npm run tauri -- build   # 生产构建，产物在 src-tauri/target/release/bundle/
npm run lint             # oxlint 静态检查
npm run lint:fix         # oxlint 安全自动修复
npm run format           # oxfmt 格式检查
npm run format:fix       # oxfmt 格式化
npm run check:frontend    # oxlint + oxfmt + 前端规范检查器
npm test                 # Vitest（Vue Test Utils + jsdom）
cd src-tauri && cargo fmt --all -- --check
cd src-tauri && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo test
cd src-tauri && cargo test github_adapter -- --nocapture
```

- `TAURI_DEV_HOST` 环境变量启用网络 HMR（默认仅 `localhost:1420`）。
- 前端 IPC 封装在 `src/api/index.ts`（`invoke("command_name", {...})`）。**不要直接调用 Tauri API**。
- Vite 配置了 `@/` → `./src/*` 别名。

## 项目结构

```text
src/                 # Vue 3 前端（Composition API, <script setup lang="ts">）
  api/index.ts       # 唯一 IPC 入口
  types/index.ts     # 前端 TS 类型
  stores/            # Pinia（Auth / PR / Repo）
  router/index.ts    # 7 条路由与登录恢复守卫
  pages/             # 6 个页面组件
  components/        # ai/ diff/ issue/ layout/ pr/ review/ shared/
  main.ts            # 挂载 Pinia + Router；HMR 时阻止恢复到 /settings
src-tauri/
  src/lib.rs         # Tauri Builder：注册 26 个 IPC 命令，设置原生菜单
  src/state.rs       # AppState + 可取消的 AI 任务注册表
  src/vault.rs       # TokenVault：Keyring 优先、AES-256-GCM 文件降级
  src/local_store.rs # SQLite 评论快照缓存
  src/platform/      # GitPlatform trait + GitHub/GitLab/Gitee adapter
  src/ai/            # OpenAI 兼容客户端、标准 SSE、Prompt、配置
  src/commands/      # auth / pr / review / issue / ai
  tests/             # GitHub/GitLab/Gitee WireMock 集成测试
```

## Rust 后端要点

- `error.rs`：`AppError` 通过 `impl From<AppError> for String` 兼容 Tauri 命令；新增错误变体需同步转换。
- `vault.rs`：平台 Token 优先保存到系统 Keyring（service `com.mergebeacon`，账户
  `git-platform:{platform}`）；不可用时 AES-256-GCM 加密写入 `~/.mergebeacon/config.json`。
  旧 `com.mergepilot` Keyring、`~/.mergepilot/config.json` 和旧明文 Token 均采用“先成功写目标、再删旧值”迁移。不要重新引入明文 Token。
- Keyring 依赖必须保留 `apple-native`、`windows-native`、`sync-secret-service` features；无 feature
  时 keyring crate 会退化为不持久化的内存 Mock。
- `crypto.rs`：AES-256-GCM；密钥 = SHA256（应用固定值 + OS 用户名）。AI API Key 仍使用此方案。
- `platform/mod.rs`：统一 `GitPlatform` trait 和 API base 规范化；GitLab/Gitee 自托管地址要保留代理子路径。
- GitLab 项目路径必须编码完整 `owner/repo`，包括 nested subgroup 的每个 `/`。
- GitHub 支持 comment/approve/request_changes；GitLab/Gitee 仅支持 comment。前后端都必须拒绝不支持事件，禁止静默降级。
- `pr_merge` 返回 `PrMergeOutcome`；合并成功后关闭关联 Issue 的失败属于部分成功，不得把合并改成失败。
- AI SSE 使用 `eventsource-stream`；事件携带 `request_id`，`AppState.ai_tasks` 管理取消句柄。
- AI Prompt 按 UTF-8 字符边界截断到约 64 KiB，禁止直接按字节切片。
- AppError → String 会丢失类型信息；当前无结构化日志（主要为 `eprintln!`）。

## 前端要点

- **无 UI 框架**：手写 CSS + CSS 变量；未经沟通不要引入 Tailwind 或组件库。
- Pinia store 只能调用 `src/api/index.ts`，组件和 store 不直接 `invoke`。
- 认证、仓库选择、Fork 上下文、仓库分页均按平台隔离。切换到未登录平台时，登录链接必须携带目标平台，不能回退到其他已登录平台。
- PR/Issue 异步请求需捕获平台、仓库和序列号；上下文改变或旧请求迟到时不得覆盖新状态。
- 仓库首次加载替换第一页；“加载更多”追加并按 `id + full_name` 去重；失败保留已有数据并可重试。
- AI 流式事件只消费当前 `request_id`；新评审和组件卸载必须取消旧请求并解除监听；取消不是错误提示。
- 模型 ID 等远端字符串只能作为文本渲染，不使用 `v-html`。
- oxlint 使用 `.oxlintrc.json`；禁止内联 `oxlint-disable`。oxfmt：双引号、分号、尾逗号、
  100 列宽。
- `src/main.ts` 有 `/settings` HMR 保护；菜单“设置...”通过 `window.__goToSettings()` 跳转。
- 界面和 AI Prompt 使用中文。

## 测试

- 前端：Vitest + Vue Test Utils + jsdom，测试位于各模块 `__tests__/*.spec.ts`，运行 `npm test`。
- Rust：单元测试覆盖 TokenVault、UTF-8 截断、SSE、API base；WireMock 覆盖三个平台 Adapter。
- 命名集成测试目标：`github_adapter`、`gitlab_adapter`、`gitee_adapter`。
- 涉及认证、平台切换、分页、AI 生命周期或合并 outcome 时，必须同步增加回归测试。
- 提交前完整执行：

```bash
npm run lint
npm run format
npm run build
npm test
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## 构建与部署

- macOS bundle：`MergeBeacon.app`，identifier：`com.mergebeacon`。
- 构建前自动执行 `npm run build`；前端产物在 `dist/`，Rust/bundle 在 `src-tauri/target/`。
- 生产 CSP 在 `src-tauri/tauri.conf.json`；修改网络或资源来源时同步审查 CSP，不得放宽远程脚本。
- macOS entitlement：`src-tauri/mergebeacon.entitlements`。

## 注意

- `.github/workflows/` 有 4 个工作流：`ci.yml`、`release.yml`、`issue-claim.yml`、`spam-comment-guard.yml`。
- 仓库使用 OpenSpec 工作流（`openspec/`）。规范或提案不得与当前能力声明冲突。
- 不要修改或提交无关文件；尤其不要把文档、生成物或全仓格式化混入功能提交。
- `.gitignore` 排除 `.opencode/`、`.reasonix/`、`reasonix.toml`。
