## 1. Rust: Add native menu bar

- [x] 1.1 Add `.setup(|app| ...)` closure to `tauri::Builder` in `src-tauri/src/lib.rs`
- [x] 1.2 Use `tauri::menu::{Menu, Submenu, MenuItem, PredefinedMenuItem}` to create app menu
- [x] 1.3 Add "设置..." `MenuItem` with `Cmd+,` accelerator under the app `Submenu`
- [x] 1.4 Attach event handler: `app.emit("open-settings", ())` on menu item click

## 2. Frontend: Listen for menu events

- [x] 2.1 Add `listen("open-settings", ...)` in `src/App.vue` (or `src/main.ts`) to navigate to `/settings` via `router.push`

## 3. Frontend: Remove sidebar settings link

- [x] 3.1 Delete `<router-link to="/settings">` block (lines 112–114) from `src/components/layout/Sidebar.vue`

## 4. Verify

- [x] 4.1 Rust compiles (`cargo check` passes with no new errors)
- [ ] 4.2 Run `cargo tauri dev` and confirm menu appears with "设置..." item
- [ ] 4.3 Click "设置..." → navigates to settings page; `⌘,` shortcut also works
- [ ] 4.4 Direct URL `/settings` still accessible
- [x] 4.5 Sidebar no longer shows "⚙ 设置" link (verified via code review)
- [x] 4.6 Frontend type check passes (`vue-tsc --noEmit`)
