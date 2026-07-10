## Context

Tauri v2 提供 `tauri::menu` 模块（`Menu`, `Submenu`, `MenuItem` 等）用于创建原生菜单栏。macOS 上的应用菜单（App Menu）自动包含"关于"、"退出"等标准项，但"偏好设置..."需要手动添加。

当前设置页（`/settings`）已存在于 Vue Router 中，内容为 `AiSettings.vue` 组件。目标是移除侧边栏导航入口，改为通过原生菜单栏触发。

## Goals / Non-Goals

**Goals:**
- 添加 macOS 原生菜单栏，包含 "Merge Pilot → 设置... (⌘,)" 菜单项
- 菜单项点击 → Tauri event → 前端监听 → Vue Router 导航到 `/settings`
- 移除侧边栏导航中的 "⚙ 设置" 链接
- 保持 `/settings` 路由和 `SettingsPage.vue` 组件可访问

**Non-Goals:**
- 不在菜单中实现设置页内容（保持现有页面路由）

## Decisions

### 1. Tauri v2 `tauri::menu` 模块 vs `tauri-plugin-menu`

Tauri v2 内置了 `tauri::menu` 模块（`Menu`, `MenuItem`, `Submenu`），不需要额外 plugin。在 `tauri::Builder` 的 `.setup()` 闭包中创建菜单即可。

### 2. 菜单事件通信方式

选择 `app_handle.emit("open-settings", ())` 而非 IPC invoke。事件监听是单向通知，前端用 `listen` 即可响应，不需要返回值。

### 3. 跨平台行为

- macOS: "设置..." 放在 App 菜单，自动映射 `⌘,` 快捷键（通过 `MenuItemWithKey`）
- Windows/Linux: "Settings" 放在 "File" 或 "Help" 菜单下
- 平台不可用时（如 Wayland 菜单不可见），用户仍可通过 URL `/settings` 直接访问

## Risks / Trade-offs

- [macOS 菜单在 dev 模式下可能不可见] → 不影响功能，菜单事件在后台仍可触发
- [移除侧边栏导航后部分用户可能找不到设置] → URL 可访问 + ⌘, 快捷键，无需额外说明
- [Tauri v2 menu API 在非 macOS 平台上外观不同] → 可接受，添加平台条件判断即可

## Migration Plan

1. Rust 端: 在 `lib.rs` 的 `setup` 中创建菜单
2. 前端: 添加 `listen` 事件监听器
3. 前端: 移除侧边栏设置链接
4. 验证: `cargo tauri dev` 测试菜单出现 + 事件触发
