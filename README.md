# MergeBeacon

[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-blue.svg)](https://v2.tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.x-42b883.svg)](https://vuejs.org)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

基于 **Tauri 2 + Vue 3 + Rust** 的跨平台 PR 评审与 Issue 管理桌面客户端。
使用统一界面连接 GitHub、GitLab 和 Gitee，集中处理 PR / MR 收件箱、Diff、人工评审、
合并与 Issue，并可通过 OpenAI 兼容接口进行 AI 辅助评审。

> 当前应用版本：`0.5.2`

## 功能概览

- **跨平台 PR 收件箱**
  - 汇总已登录且已启用平台中的待处理 PR / MR 和当前账号创建的 PR / MR
  - 区分 GitHub / GitLab 的 Reviewer、Assignee，以及 Gitee 的评审人和测试人；“我创建的”由范围筛选控制
  - 卡片直接展示审批、CI / 测试、Draft、冲突和总体合并状态，悬浮时显示具体阻塞原因
  - 支持按范围、角色、合并状态、平台和 `owner/repo` 筛选，并按更新时间统一排序
  - GitHub 对当前页执行一次批量状态查询；GitLab 和 Gitee 优先使用列表字段，避免逐条请求
  - 三个平台分别维护分页和错误状态；单个平台失败时可独立重试，不影响其他平台结果
  - 点击收件箱条目可直接打开对应平台、仓库和编号的详情页
- **多平台仓库管理**
  - 使用 Personal Access Token 登录 GitHub、GitLab、Gitee
  - 各平台独立保存登录状态、仓库选择、Fork 上下文和分页进度
  - 仓库侧栏支持增量“加载更多”、去重、失败重试和独立刷新
  - GitLab 与 Gitee 支持填写私有化部署地址，并统一规范化 API 版本路径
- **Pull Request / Merge Request**
  - 按 Open、Closed、Merged、All 状态筛选
  - 支持分页以及 10 / 20 / 50 / 100 条每页
  - Open 列表卡片展示审批、CI / 测试、Draft、冲突和总体合并状态，悬浮时显示具体阻塞原因
  - GitHub 对当前页 Open PR 执行一次批量状态查询；GitLab 和 Gitee 使用列表字段，不逐条请求详情检查
  - Closed / Merged 列表只显示关闭或合并终态，不继续查询实时审批和 CI / 测试状态
  - 查看标题、作者、分支、标签、合并状态和跨平台合并就绪检查；详情页检查仍是合并前的最终依据
  - 自动读取 Fork 的上游仓库信息，并可在 Fork 与上游仓库之间切换
- **Diff 与人工评审**
  - 使用标准化 patch 和 diff2html 渲染 side-by-side Diff
  - 提供文件导航、Diff 专注侧栏、重命名路径展示和按文件上下文展开/收起
  - 在 GitHub、GitLab、Gitee 上均可选中代码后添加行级评论
  - 评论支持逻辑、安全、性能、代码风格、日志等分类
  - `⌘/Ctrl + Enter` 提交快速评论，`Esc` 取消
  - 查看整体评审与行级评论；评论附带代码快照、过期检测和 MiniDiffView
- **PR 合并 / 关闭 / 重新打开**
  - 根据平台能力显示可用合并策略，并在合并前展示检查、审批、冲突和权限状态
  - 可自定义合并提交标题和描述
  - 可关闭已打开的 PR / MR，重新打开已关闭的 PR / MR
  - 合并后可关闭关联 Issue；关闭失败会作为部分成功返回，不会把已成功的合并改为失败
- **Issue**
  - 查看仓库 Issue
  - 创建带正文和标签的 Issue
- **AI 辅助评审**
  - 支持 OpenAI 兼容的 Chat Completions 与 Models API
  - 内置 OpenAI、DeepSeek、通义千问、Moonshot、Ollama 预设
  - 支持连接测试、模型列表获取与搜索、Temperature、Max Tokens 配置
  - 支持普通响应和标准 SSE 流式响应；每次流式请求使用独立 `request_id`
  - 可聚焦全部、安全、性能、逻辑或代码风格，并输出 Critical / Major / Minor / Info 建议
  - 可将建议直接加入评审草稿、编辑后加入或忽略，并从建议跳转到对应 Diff 文件和行
  - 记录评审所基于的 `head_sha`；PR / MR 更新后标记旧结果，并禁止提交旧版本草稿
  - 支持比较上次成功评审版本与当前版本，只评审新增改动；切换页签时保留评审状态
- **桌面集成与更新**
  - 原生菜单提供设置入口和撤销、重做、剪切、复制、粘贴、全选
  - 单实例运行；再次启动时激活现有主窗口
  - 安全恢复窗口位置、尺寸和最大化状态
  - 设置页支持签名更新检查、下载安装、确认重启和每日自动检查
  - Windows 便携版通过官方版本化 ZIP 手动更新
  - 可复制脱敏诊断信息，包含版本、系统、凭证存储和配置状态，不包含 Token 或私有地址
  - 可独立显示或隐藏 GitHub、GitLab、Gitee 平台入口

## 平台能力

| 能力 | GitHub | GitLab | Gitee |
|---|:---:|:---:|:---:|
| PAT 登录与仓库列表 | ✅ | ✅ | ✅ |
| 私有化实例地址 | — | ✅ | ✅ |
| 跨仓库 PR / MR 收件箱 | ✅ | ✅ | ✅ |
| PR / MR 列表、详情与 Diff | ✅ | ✅ | ✅ |
| 合并就绪检查 | ✅ | ✅ | ✅ |
| 增量评审 Compare Diff | ✅ | ✅ | ✅ |
| 整体评审 | 评论 / 批准 / 请求修改 | 评论（MR Note） | 评论（PR Comment） |
| 查看整体评审 | ✅ | ✅ | ✅ |
| Diff 行级评论 | ✅ | ✅ | ✅ |
| 合并策略 | Merge / Squash / Rebase | Merge / Squash | Merge / Squash / Rebase |
| Issue 列表与创建 | ✅ | ✅ | ✅ |
| Fork 上游识别 | ✅ | ✅ | ✅ |

> GitLab 和 Gitee 仅支持评论型整体评审。界面不会显示“批准 / 请求修改”，后端也会拒绝
> 非 `comment` 事件；不会静默降级为普通评论。

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2、Single Instance、Window State、Updater |
| 前端 | Vue 3、Composition API、Pinia、Vue Router |
| 前端构建 | TypeScript、Vite 6 |
| 代码规范 | oxlint + oxfmt + 前端规范检查器 |
| 后端 | Rust 2021、Tokio、Reqwest |
| 平台抽象 | `GitPlatform` trait + GitHub / GitLab / Gitee Adapter |
| Diff 渲染 | 标准化 patch、diff2html、highlight.js |
| AI | OpenAI 兼容 API、SSE Streaming、增量 Compare Diff |
| 凭证存储 | 系统 Keyring 优先，AES-256-GCM 加密文件降级 |
| 本地数据 | SQLite 评论快照缓存 |
| 测试 | Vitest、Vue Test Utils、jsdom、Cargo Test、WireMock |

## 快速开始

### 环境要求

- [Node.js](https://nodejs.org/) 20（项目 CI 使用的版本）
- [Rust](https://www.rust-lang.org/tools/install) stable
- 对应操作系统的 [Tauri 2 前置依赖](https://v2.tauri.app/start/prerequisites/)

### 安装与运行

```bash
git clone https://github.com/tisrop/mergebeacon.git
cd mergebeacon

npm install
npm run tauri -- dev
```

仅启动 Web 前端：

```bash
npm run dev
```

### 构建桌面应用

```bash
npm run tauri -- build
```

构建前会自动运行 `npm run build`。Tauri 产物写入 `src-tauri/target/release/bundle/`；
macOS `.app` 包名基于 `productName`，当前为 `MergeBeacon.app`。

## 使用说明

### 1. 登录代码托管平台

在登录页选择平台并填写 Personal Access Token。GitLab 或 Gitee 私有化部署可额外填写服务器地址；
未带协议时会自动补充 `https://`。

| 平台 | Token 创建地址 |
|---|---|
| GitHub | [github.com/settings/tokens](https://github.com/settings/tokens) |
| GitLab | [gitlab.com/-/user_settings/personal_access_tokens](https://gitlab.com/-/user_settings/personal_access_tokens) |
| Gitee | [gitee.com/profile/personal_access_tokens](https://gitee.com/profile/personal_access_tokens) |

Token 至少需要读取仓库、PR / MR 和 Issue 的权限；提交评审、评论、合并或创建 Issue 时还需要
相应的写权限。

### 2. 使用 PR 收件箱

1. 在左侧打开 **PR 收件箱**。
2. 选择“待我处理”或“我创建的”，再按角色、合并状态、平台或仓库名称缩小范围。
3. 在卡片中查看具体关系，以及审批、CI / 测试、Draft、冲突和总体合并状态。
4. 将鼠标悬浮在状态摘要上，查看审批不足、检查失败、冲突等具体阻塞原因。
5. 如果某个平台加载失败，使用该平台错误提示中的 **重试**，已加载的平台结果会保留。
6. 点击条目直接进入对应 PR / MR 详情页。

### 3. 评审 PR / MR

1. 从收件箱打开条目，或在左侧选择平台和仓库后从 PR / MR 列表打开记录。
2. 在 Open 列表卡片中先查看审批、CI / 测试、Draft、冲突和总体合并状态；Closed / Merged
   卡片只显示最终状态。
3. 进入详情后查看合并就绪状态，确认最新检查、审批、冲突和权限信息。
4. 在 **Diff** 页签选择文件；需要更多代码时展开单个 hunk 或全部上下文。
5. 选中代码并打开快速评论框；三个平台均支持提交行级评论。
6. 在 **评审意见** 页签查看整体评审和行级评论。
7. 需要合并时选择当前平台支持的策略，并确认关联 Issue 的关闭选项。

### 4. 配置和使用 AI 评审

1. 打开 **设置 → AI 评审设置**。
2. 选择预设或填写 OpenAI 兼容 API 端点。
3. 输入 API Key，点击 **保存设置**。
4. 点击 **获取模型** 并选择模型。
5. 调整 Temperature、Max Tokens，并使用 **测试连接** 验证配置。
6. 打开 PR / MR 的 **AI 评审** 页签，选择聚焦模式后开始评审。
7. 将建议加入评审草稿、编辑或忽略；点击文件位置可跳转到 Diff，再返回 AI 评审继续处理。
8. PR / MR 有新提交时，可使用增量评审比较上次成功版本与当前版本。

常用端点示例：

| 服务 | API 端点 | 默认模型示例 |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | `gpt-4o` |
| DeepSeek | `https://api.deepseek.com/v1` | `deepseek-chat` |
| 通义千问 | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `qwen-plus` |
| Moonshot | `https://api.moonshot.cn/v1` | `moonshot-v1-8k` |
| Ollama | `http://localhost:11434/v1` | `llama3` |

AI 请求会携带 PR / MR 标题、描述和目标 Diff。为控制输入大小，超过约 64 KiB 的 Diff 会在
UTF-8 字符边界安全截断，避免切断中文或 emoji。

### 5. 更新应用和复制诊断信息

1. 打开 **设置 → 应用更新**，手动检查更新或启用每日自动检查。
2. 安装版下载完成后确认重启；Windows 便携版会打开官方 ZIP 下载地址，需要退出应用后手动覆盖。
3. 反馈问题时，在 **设置 → 诊断信息** 复制脱敏信息并粘贴到 Issue。

## 本地数据与安全

- 平台 Token 优先保存到系统凭证库：macOS Keychain、Windows Credential Manager 或
  Linux Secret Service；service 固定为 `com.mergebeacon`，账户为 `git-platform:{platform}`。
- 系统凭证库不可用时，Token 使用 AES-256-GCM 加密后写入
  `~/.mergebeacon/config.json`；目录权限收紧为 `0700`，文件权限为 `0600`，并采用原子写入。
- 旧 `com.mergepilot` Keyring、`~/.mergepilot/config.json` 和旧明文 `token_{platform}` 会在首次
  读取时迁移；目标写入成功后才删除旧值。
- 私有化实例地址仍保存在普通配置中；HTTP 地址仅应用于可信内网，Token 传输不会被 TLS 加密。
- AI 配置保存在操作系统应用配置目录下的 `ai_config.json`。AI API Key 使用 AES-256-GCM
  加密，当前未迁移到系统 Keyring。
- 登录 Token 只发送到所选代码托管平台；AI API Key 只发送到配置的 AI 端点。
- 更新只接受配置中的官方更新源和 Minisign 公钥验证通过的元数据与安装包。
- 诊断信息会隐藏自托管平台地址、非官方 AI 地址和凭证值。
- macOS 应用标识符为 `com.mergebeacon`。

请保护本机账号及配置文件权限，不要提交本地配置文件或在不可信设备上保存凭据。

## 项目结构

```text
mergebeacon/
├── src/
│   ├── api/index.ts             # 唯一 Tauri IPC 封装入口
│   ├── components/
│   │   ├── ai/                  # AI 设置、流式/增量评审、建议卡片
│   │   ├── diff/                # 标准化 Diff、上下文展开与快速评论
│   │   ├── inbox/               # 跨平台 PR 收件箱卡片
│   │   ├── issue/               # Issue 卡片与表单
│   │   ├── layout/              # 应用布局、平台与仓库侧边栏
│   │   ├── pr/                  # PR 卡片、筛选器与合并就绪状态
│   │   ├── review/              # 整体评审、评论列表
│   │   └── shared/              # 共享表单与状态组件
│   ├── pages/                   # 7 个页面：登录、收件箱、PR、Issue、设置
│   ├── router/index.ts          # 8 条路由与登录恢复守卫
│   ├── stores/                  # Auth / Capability / Repo / PR / Inbox / UI / Update
│   └── types/index.ts           # 前端共享类型
├── src-tauri/
│   ├── src/
│   │   ├── ai/                  # OpenAI 兼容客户端、Prompt、配置
│   │   ├── commands/            # 认证、诊断、更新、平台、收件箱、PR、评审、Issue、AI
│   │   ├── platform/            # GitPlatform trait 与三个平台 Adapter
│   │   ├── file_content.rs      # Diff 上下文文件内容处理
│   │   ├── patch.rs             # 跨平台 patch 标准化
│   │   ├── single_instance.rs   # 单实例窗口激活协调
│   │   ├── window_state.rs      # 窗口状态安全恢复
│   │   ├── local_store.rs       # SQLite 评论快照缓存
│   │   ├── state.rs             # 共享状态与可取消 AI 任务注册表
│   │   └── vault.rs             # Keyring 优先、加密文件降级的 TokenVault
│   ├── tests/                   # GitHub / GitLab / Gitee WireMock 集成测试
│   ├── Cargo.toml
│   └── tauri.conf.json
├── AGENTS.md                    # AI 编码代理的项目上下文与操作约束
├── CODE_STANDARDS.md            # 代码实现与评审规范
├── FRONTEND_STANDARDS.md        # 前端视觉、交互与可访问性规范
├── package.json
└── README.md
```

## 代码规范

开发和代码评审以 [`CODE_STANDARDS.md`](CODE_STANDARDS.md) 为基线。该规范覆盖 Vue / TypeScript、
Rust / Tauri 架构边界、跨平台行为、异步生命周期、凭据安全、测试要求和合并门禁。
前端页面、组件和样式还必须遵循 [`FRONTEND_STANDARDS.md`](FRONTEND_STANDARDS.md)；AI 编码代理
同时遵循 [`AGENTS.md`](AGENTS.md)。

涉及认证、平台切换、分页、收件箱、AI 生命周期、Diff 上下文、更新流程或合并结果的改动，
必须同步增加覆盖成功、失败和竞态路径的回归测试。

## 开发与测试

```bash
# 前端类型检查、构建与测试
npm run build
npm test

# 代码检查、格式化和项目门禁
npm run lint
npm run format
npm run check:frontend-standards
npm run check:version
npm run check:updater
npm run check:frontend
npm run lint:fix
npm run format:fix

# Rust 格式、静态检查与测试
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

当前注册 37 个 Tauri Commands：

- 认证（5）：`auth_login`、`auth_logout`、`auth_check`、`auth_has_any_token`、`auth_has_token`
- 诊断、更新与平台能力（7）：`support_info`、`copy_support_info`、`app_version`、`update_check`、
  `update_download_and_install`、`update_restart`、`platform_capabilities`
- 仓库（1）：`repo_list`
- 收件箱与 PR / MR（10）：`review_inbox_list`、`pr_list`、`pr_detail`、`pr_merge_readiness`、
  `pr_diff`、`pr_compare_diff`、`pr_file_content`、`pr_merge`、`pr_close`、`pr_reopen`
- 评审（4）：`review_submit`、`review_comment_add`、`review_list`、`review_comments_list`
- Issue（2）：`issue_list`、`issue_create`
- AI（8）：`ai_get_config`、`ai_save_config`、`ai_save_api_key`、`ai_review`、
  `ai_review_stream`、`ai_review_cancel`、`ai_list_models`、`ai_test_connection`

## 已知限制

- GitLab 和 Gitee 仅支持评论型整体评审；只有 GitHub 支持原生批准和请求修改事件。
- 自托管服务器仍允许 HTTP，以兼容本地和内网实例；请勿在不可信网络中使用 HTTP 传输 Token。
- Windows 便携版不执行应用内覆盖安装，需要下载官方 ZIP 后退出应用并手动替换。
- AI 返回内容仍需包含约定的单个完整 JSON 评审对象；不兼容该结构的模型可能导致解析失败。
- `AppError` 通过字符串返回前端，错误类型信息尚未结构化；后端日志仍以 `eprintln!` 为主。

## License

[Apache 2.0](LICENSE)
