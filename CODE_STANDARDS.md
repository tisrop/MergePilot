# MergeBeacon 代码规范

本文是 MergeBeacon 的代码实现与评审基线，适用于 `src/`、`src-tauri/`、测试、构建配置和
OpenSpec 变更。`AGENTS.md` 记录项目上下文和操作约束；本文定义可执行的代码质量要求。若二者
冲突，以更具体、更严格且不违背产品能力声明的规则为准。

规则关键词：

- **必须**：合并前必须满足；违反时应阻止合并。
- **应该**：默认遵循；偏离时需在 PR 中说明原因和替代保障。
- **禁止**：不得通过静默降级、忽略错误或关闭检查绕过。

## 1. 通用原则

1. **保持变更聚焦**：只修改完成当前需求所需的文件；禁止混入全仓格式化、生成物、IDE 配置或
   无关重构。
2. **先保持行为，再改善结构**：重构必须有测试保护；功能修改必须覆盖成功、失败和边界路径。
3. **边界处校验，内部使用明确类型**：外部 API、Tauri IPC、持久化数据和用户输入进入系统时完成
   校验与规范化，内部不得依赖隐含的字符串约定。
4. **错误必须可见且语义稳定**：不得吞掉错误或把失败伪装成成功；部分成功必须用明确 outcome
   表达，不能覆盖已经完成的主操作。
5. **默认安全**：密钥、Token、远端文本和自托管地址按不可信输入处理；禁止为了便利降低 CSP、
   明文存储凭据或使用 `v-html`。
6. **不静默降级能力**：平台不支持的动作必须由前端和后端共同拒绝，并向用户展示清晰原因。
7. **中文产品体验**：界面文案、错误提示和 AI Prompt 使用中文；代码标识符、协议字段沿用英文。

## 2. 文件、命名与格式

### 2.1 TypeScript / Vue

- 使用 Vue 3 Composition API 和 `<script setup lang="ts">`。
- 组件文件使用 PascalCase；组合式函数和 Pinia store 使用 `useXxx`；布尔变量使用
  `is`、`has`、`can` 或 `should` 前缀。
- 优先使用 `@/` 别名导入 `src/` 内模块；同目录短路径可使用相对导入。
- 新增共享数据结构时更新 `src/types/index.ts`；不得在多个调用点复制形状相近但语义相同的类型。
- 禁止新增无理由的 `any`、非空断言和类型强转；确需使用时必须先完成运行时校验并缩小作用域。
- 禁止内联 `oxlint-disable`。应修复根因或在 `.oxlintrc.json` 中以最小文件范围配置并说明
  原因。
- 使用 oxfmt：双引号、分号、尾逗号、2 空格缩进、100 列宽、LF 换行。

### 2.2 Rust

- 遵循 `rustfmt`；`cargo clippy --all-targets -- -D warnings` 必须通过。
- 模块、函数和变量使用 `snake_case`；类型和 trait 使用 `PascalCase`；常量使用
  `SCREAMING_SNAKE_CASE`。
- 生产路径禁止新增可避免的 `unwrap`、`expect`、`panic!` 和未说明的 `unreachable!`；使用
  `Result`、`Option` 组合子或带上下文的 `AppError`。
- 函数应保持单一职责。解析、校验、请求构造和响应映射复杂时拆成可单测的纯函数。
- 公共类型和跨模块约束应靠类型、枚举和返回值表达，禁止以魔法字符串或注释代替。

## 3. 架构边界

### 3.1 前端边界

- `src/api/index.ts` 是唯一 Tauri IPC 入口。组件和 Pinia store **禁止**直接调用 `invoke` 或其他
  Tauri command API。
- 组件负责展示和局部交互，跨页面业务状态放入 Pinia；不得在多个组件复制请求编排逻辑。
- Pinia store 只能通过 `src/api/index.ts` 访问后端，并保留 loading、error、空状态和重试语义。
- 禁止未经讨论引入 UI 框架、Tailwind 或第二套状态管理方案。样式复用 CSS 变量和现有手写 CSS。
- 前端视觉与交互实现必须同时遵循 [`FRONTEND_STANDARDS.md`](FRONTEND_STANDARDS.md)；
  修改全局 Token、共享组件、布局基线或品牌语言时，必须同步更新该规范。

### 3.2 Rust / Tauri 边界

- `src-tauri/src/commands/` 负责 IPC 参数校验、状态协调和结果映射；平台 HTTP 细节放在
  `src-tauri/src/platform/`，AI 协议细节放在 `src-tauri/src/ai/`。
- 新增 Tauri command 时必须同步：命令实现、`src-tauri/src/lib.rs` 注册、`src/api/index.ts`
  封装、TypeScript 类型及测试。
- 平台差异通过 `GitPlatform` trait、能力判断或明确枚举表达；禁止在上层散落平台名称字符串分支。
- 新增 `AppError` 变体时必须同步 `AppError -> String` 转换，并验证用户可理解的中文错误语义。

## 4. 异步状态与生命周期

- PR、Issue、仓库和认证请求发起时必须捕获平台、仓库、分页游标与请求序列号；响应写入前重新
  核对当前上下文。旧请求或其他平台的迟到响应不得覆盖新状态。
- 仓库首次加载替换第一页；“加载更多”只追加，并按 `id + full_name` 去重。追加失败必须保留
  已有数据并允许重试。
- 平台隔离的数据不得共享隐式单例状态。切换到未登录平台时，登录链接必须携带目标平台。
- AI 流式事件只处理当前 `request_id`。开始新评审和组件卸载时必须取消旧任务并解除监听；用户
  主动取消属于正常生命周期，不显示为错误。
- Vue 组件注册的事件监听器、定时器和异步订阅必须在卸载时清理；Rust 可取消任务必须从
  `AppState.ai_tasks` 中正确注册和移除。

## 5. 平台与网络语义

- API base 的规范化必须保留 GitLab/Gitee 自托管地址中的代理子路径，禁止按“域名根路径”重建。
- GitLab 项目标识必须编码完整 `owner/repo`，包括任意层级 nested subgroup。
- GitHub 支持 `comment`、`approve`、`request_changes`；GitLab/Gitee 仅支持 `comment`。前后端
  必须一致校验，不得把不支持的事件自动改成 comment。
- 平台 adapter 必须把远端状态码、空响应和错误响应映射为稳定的应用语义；不得只检查 JSON
  形状而忽略 HTTP 状态。
- `pr_merge` 使用 `PrMergeOutcome` 表达结果。合并成功后关闭关联 Issue 失败属于部分成功，必须
  保留“已合并”事实并单独报告后续失败。
- HTTP 客户端、超时、认证头和用户代理应复用现有基础设施；新增依赖或例外需说明必要性。

## 6. 安全与隐私

- 平台 Token 优先保存到系统 Keyring；降级文件必须使用 AES-256-GCM。禁止将 Token、AI API
  Key 或 Authorization header 写入明文配置、日志、错误提示、测试快照和提交内容。
- Keyring 依赖必须保留 `apple-native`、`windows-native`、`sync-secret-service` features。
- 凭据迁移必须遵循“成功写入新目标后再删除旧值”，迁移失败时不得丢失现有凭据。
- 所有远端字符串按纯文本渲染。禁止使用 `v-html` 展示模型 ID、评论、标题或错误正文。
- AI Prompt 截断必须位于 UTF-8 字符边界，并保持约 64 KiB 上限；禁止直接按字节索引字符串。
- CSP 只增加功能所需的最小来源；禁止允许远程脚本或使用宽泛通配符规避资源限制。
- 日志不得包含密钥、完整认证响应或敏感仓库内容；临时诊断日志必须在提交前移除。

## 7. 错误处理与用户反馈

- 前端异步操作必须结束 loading 状态，并区分空数据、可重试失败、取消和不支持能力。
- 后端不得使用空字符串或通用“请求失败”掩盖可操作信息；同时避免把完整敏感响应直接透传。
- 错误恢复不得清空先前成功加载的数据，除非用户明确刷新且新上下文已经生效。
- 对不可逆或高影响动作，UI 必须展示目标仓库、PR/Issue 和动作语义，避免平台切换后误操作。

## 8. 测试规范

- 修复缺陷必须先复现或添加能防止回归的测试；新增行为至少覆盖主路径和一个关键失败/边界路径。
- 前端使用 Vitest + Vue Test Utils + jsdom，测试放在相邻 `__tests__/*.spec.ts` 中。
- Rust 纯逻辑使用单元测试；平台 HTTP 行为使用 WireMock 集成测试。
- 以下改动必须同步增加回归测试：
  - 认证恢复、平台切换和登录目标；
  - 仓库分页、去重、重试和迟到响应；
  - PR/Issue 异步上下文；
  - AI `request_id`、取消和监听器生命周期；
  - 平台能力拒绝、API base、路径编码；
  - `PrMergeOutcome` 与部分成功；
  - TokenVault、凭据迁移、UTF-8 截断和 SSE 解析。
- 测试必须验证可观察行为，不应依赖实现细节、真实网络、真实系统密钥或执行顺序。

## 9. 依赖、配置与规范变更

- 新增依赖前必须确认现有依赖或标准库无法满足，并评估维护状态、许可证、包体积和安全风险。
- 修改网络或资源来源时同步审查 `src-tauri/tauri.conf.json` 的 CSP，不得顺带放宽无关策略。
- 修改平台能力、用户流程或持久化语义时，必须同步更新相关 OpenSpec proposal/design/tasks；规范不得
  声明尚未实现的能力。
- 禁止提交 `dist/`、`src-tauri/target/`、IDE 文件、密钥、临时日志或本地工具状态。

## 10. 评审与合并门禁

代码评审按以下优先级检查：

1. 数据丢失、凭据泄露、权限绕过和不可逆误操作；
2. 跨平台语义错误、状态竞态、取消/监听器泄漏和部分成功处理；
3. 架构边界、错误处理、兼容性和测试缺口；
4. 可维护性、性能和由工具无法自动发现的规范问题。

只报告可复现、可定位且会影响正确性、安全性或维护性的发现。纯格式问题优先交给自动化工具；不得
用主观偏好阻止合并。

按变更范围运行检查；提交前完整门禁为：

```bash
npm run lint
npm run check:frontend-standards
npm run format
npm run build
npm test
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

合并前还必须确认：工作区无无关改动、测试与实现同步、远端文本安全渲染、凭据未进入 diff、
OpenSpec 与当前能力一致。
