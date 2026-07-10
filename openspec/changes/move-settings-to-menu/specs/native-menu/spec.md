## ADDED Requirements

### Requirement: Application menu with Settings item
The system SHALL create a native macOS application menu bar containing a "设置..." (Preferences) menu item.

#### Scenario: Menu item exists
- **WHEN** the application starts
- **THEN** the macOS menu bar displays "Merge Pilot" menu with a "设置..." item

#### Scenario: Keyboard shortcut
- **WHEN** user presses Command+, (⌘,)
- **THEN** the system SHALL trigger the same action as clicking "设置..." menu item

### Requirement: Menu item emits frontend event
When the "设置..." menu item is clicked, the system SHALL emit an `open-settings` event to the frontend webview.

#### Scenario: Menu click navigates to settings
- **WHEN** user clicks "设置..." menu item or presses ⌘,
- **THEN** the frontend navigates to `/settings` route

### Requirement: Settings route remains accessible
The `/settings` route and `SettingsPage.vue` component SHALL remain functional by direct URL access.

#### Scenario: Direct URL access
- **WHEN** user navigates to `/settings` directly
- **THEN** the settings page renders normally

### Requirement: Sidebar settings link removed
The sidebar navigation SHALL NOT include a link to the settings page.

#### Scenario: Sidebar hides settings
- **WHEN** the sidebar renders
- **THEN** no "⚙ 设置" link appears in the navigation list
