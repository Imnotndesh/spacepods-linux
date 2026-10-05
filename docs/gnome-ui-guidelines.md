# GNOME Platform Guidelines for SpacePods Linux

Research notes and a concrete audit for `spacepods-ui`, the GTK4 + libadwaita
companion app. Written before the UI cleanup so every change can be traced back
to an authoritative source instead of personal taste.

> Sources are linked inline. The primary references are the
> [GNOME Human Interface Guidelines](https://developer.gnome.org/hig/),
> the [libadwaita documentation](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/),
> and the [gtk4-rs book](https://gtk-rs.org/gtk4-rs/stable/latest/book/).

---

## 1. Design principles (why this matters)

The HIG's [Design Principles](https://developer.gnome.org/hig/principles.html)
boil down to four rules that apply directly to this app:

1. **People first / inclusive.** Accommodate different abilities and form
   factors. This is why every icon-only control needs an accessible name and
   why the layout must survive 360 px widths.
2. **Simplicity.** Do one thing well, use progressive disclosure, keep
   frequently used actions close. The sidebar already hides pages a device
   cannot use — keep that behaviour and avoid showing stale controls.
3. **Reduce effort.** Automate, minimise steps, prevent mistakes, and prefer
   **undo over confirmation dialogs**.
4. **Don't waste attention.** Never pop up a dialog unexpectedly; use toasts
   for non-critical feedback.

A useful consequence: **the device is the subject, the app is the tool.** The
window should answer "what can I do with *my* earbuds right now", which is why
the sidebar is feature-oriented and driven by the connected `DeviceProfile`.

---

## 2. Navigation and adaptive layout

Reference: [HIG Navigation](https://developer.gnome.org/hig/guidelines/navigation.html),
[HIG Scaling & Adaptiveness](https://developer.gnome.org/hig/guidelines/adaptive.html),
and libadwaita's [Adaptive Layouts](https://github.com/GNOME/libadwaita/blob/main/doc/adaptive-layouts.md).

- View switchers are for a **small, flat** set of equivalent pages; sidebars are
  for more pages. SpacePods has 8–10 feature pages, so a **sidebar split view is
  the correct pattern** (not tabs, not a stack of back-browsing pages).
- `AdwNavigationSplitView` is the canonical widget for a navigation sidebar. It
  collapses into an `AdwNavigationView` automatically. `AdwOverlaySplitView` is
  intended for the **utility pane** pattern, where the sidebar is supplemental.
- Either split view must be paired with an `AdwBreakpoint` that sets
  `collapsed=True` at a narrow width, and the sidebar widget should switch to
  `AdwSidebar`/`AdwViewSwitcherSidebar` "page" mode at the same time.
- **Minimum supported width:** GNOME desktop apps must work down to
  `1024×600`; phone-shaped layouts down to `360×294`. SpacePods currently sets a
  360 px width request, which is the right floor.
- Wrap long-form content in an `AdwClamp` so lines never get uncomfortably wide
  (the pages already do this with a 500–600 px clamp).
- **Do not manually re-implement back navigation.** Let the split view /
  navigation view own the back button and page titles.

### Audit

| Finding | Severity | Action |
|---|---|---|
| Uses `AdwOverlaySplitView` for primary navigation | medium | Keep for now (works, and `NavigationSplitView` needs libadwaita ≥ 1.7). Documented as a follow-up. |
| Manual back button + manual `content_stack` visibility | medium | Works, but duplicate of what `NavigationSplitView` does. Simplify when bumping to libadwaita 1.7. |
| Content header title is always empty | low | Populate the header with `AdwWindowTitle` per selected page. |
| Sidebar toggle icon is `go-previous-symbolic` | low | Prefer the dedicated `adw-sidebar-symbolic` toggle bound to `show-sidebar`. |

---

## 3. Preferences and rows

Reference: [libadwaita Preferences](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/class.PreferencesPage.html).

- Use `AdwPreferencesPage` > `AdwPreferencesGroup` > rows. Rows should be the
  standard `AdwActionRow`, `AdwSwitchRow`, `AdwComboRow`, `AdwSpinRow`,
  `AdwEntryRow`, `AdwExpanderRow`, or `AdwButtonRow`.
- Prefer **switch rows** over a "toggle button in a group". Avoid re-labeling a
  switch as two mutually exclusive buttons.
- `AdwButtonRow` (libadwaita ≥ 1.6) is the idiomatic destructive/action row, with
  `add_css_class("destructive-action")` for destructive actions.
- Group titles should be short nouns; use `PreferencesGroup::set_description` for
  a sentence of explanation.

### Audit

| Finding | Severity | Action |
|---|---|---|
| Settings page is a hand-rolled `GtkBox` page with a `title-1` label | medium | Already uses `PreferencesGroup`; keep the page but drop the duplicated big title once the header bar shows it. |
| Generic feature pages show controls a device may not support | medium | Already gated by `DeviceProfile` features. The new device list keeps that behaviour. |
| Icon-only suffix buttons (`go-next-symbolic`) have no accessible name | medium | Add `set_tooltip_text` / accessible labels to repeated `ActionRow` chevrons is not needed (the row has a title); for real icon buttons, set `accessible-name`. |

---

## 4. Dialogs

Reference: [HIG Dialogs](https://developer.gnome.org/hig/patterns/feedback/dialogs.html).

- GNOME has **alert dialogs** (`AdwAlertDialog`) and **action dialogs**
  (`AdwDialog`). `GtkDialog` is deprecated in GTK 4 and should not be used in new
  code.
- Alert dialogs: heading describes the action, body explains the consequence,
  **cancel comes first**, affirmative button uses a specific imperative verb
  ("Rename", not "OK").
- Destructive actions should offer **undo** (toast) or confirm; prefer undo.
- Dialogs are always modal and always have a parent window.
- Give the first useful control initial focus; bind Return to the affirmative
  action and Esc to cancel.

### Audit

| Finding | Severity | Action |
|---|---|---|
| `settings_page.rs` uses the deprecated `gtk4::Dialog` for "Change Bluetooth Name" | high | Replace with `AdwAlertDialog` + `AdwEntryRow`, return-key bound to Rename, Esc to Cancel. |
| "Clear Pairing Record" fires immediately with no confirmation | high | Confirm with an `AdwAlertDialog` whose affirmative button is marked `destructive`. |
| `connect_response` dialogs do not set initial focus | low | Call `entry.grab_focus()` before presenting. |

---

## 5. Feedback: toasts, banners, status pages

Reference: [HIG Notifications](https://developer.gnome.org/hig/patterns/feedback/notifications.html)
and libadwaita `AdwStatusPage`, `AdwToast`, `AdwBanner`.

- **Toasts** (`AdwToastOverlay`) for transient confirmation and non-critical
  errors, with an optional undo action. SpacePods already has `ctx.toast()`,
  `ctx.success()` and `ctx.error()`.
- **Banners** (`AdwBanner`, libadwaita ≥ 1.7) for persistent, page-level
  conditions such as "the background service is not running". They are dismissible
  but stay until acted on / dismissed.
- **Status pages** (`AdwStatusPage`) for full-page empty/error states; SpacePods
  already uses them for "no device" and "daemon unreachable".
- Errors must never be swallowed. The app already routes failures through
  `ctx.error()`/`ctx.daemon_unreachable()`; keep that invariant.
- Use a `GtkSpinner` for short indeterminate work, and only disable the controls
  that are actually busy.

### Audit

| Finding | Severity | Action |
|---|---|---|
| "Daemon not running" only appears inside the Settings page | medium | Surface it persistently (banner/status page) on any page when the socket is missing. |
| `set_busy` helper exists but is applied inconsistently | low | Keep and use it around slow BLE round-trips. |

---

## 6. Controls, lists and window chrome

Reference: [HIG Patterns](https://developer.gnome.org/hig/patterns.html).

- Use `AdwToolbarView` + `AdwHeaderBar` for window chrome. Add a **primary menu**
  (`GtkMenuButton`, icon `open-menu-symbolic`) exposing app-wide actions: About,
  Keyboard Shortcuts, Quit.
- Window controls belong in the header bar of the sidebar pane (libadwaita hides
  the middle buttons automatically in a split view).
- Destructive buttons use the `destructive-action` style class; the single most
  likely action on a page uses `suggested-action`.
- Use `.pill` sparingly; the HIG treats it as an emphasis style, not decoration.

### Audit

| Finding | Severity | Action |
|---|---|---|
| No primary menu anywhere | high | Add `open-menu-symbolic` menu button to the sidebar and content header bars. |
| Content and sidebar have separate `HeaderBar`s with no shared menu | medium | Wire both to the same `gio::Menu` model. |
| No About window | medium | Add `AdwAboutWindow` (available since libadwaita 1.0) opened from the menu and Settings. |

---

## 7. Keyboard shortcuts

Reference: [HIG Standard Keyboard Shortcuts](https://developer.gnome.org/hig/reference/keyboard.html)
and [HIG Keyboard](https://developer.gnome.org/hig/guidelines/keyboard.html).

Standard shortcuts the app should support:

| Action | Shortcut |
|---|---|
| Close window | `Ctrl+W` |
| Quit app | `Ctrl+Q` |
| Preferences / Settings | `Ctrl+,` |
| Toggle side pane (sidebar) | `F9` |
| Open primary menu | `F10` |
| Keyboard shortcuts | `Ctrl+?` |
| Help | `F1` |

Rules:

- Prefer `Ctrl` + letter; do **not** use `Alt` (reserved for access keys) or
  `Super` (reserved by the shell).
- Every action must be reachable by keyboard; `Tab`/`Shift+Tab` must traverse
  the whole window.
- Register shortcuts as `gio::SimpleAction`s on the application and add them
  with `Application::set_accels_for_action`, so they work in every window.

### Audit

| Finding | Severity | Action |
|---|---|---|
| No accelerators registered | high | Add `quit` (`Ctrl+Q`), `close` (`Ctrl+W`), `preferences` (`Ctrl+,`), `toggle-sidebar` (`F9`), `about`. |
| `Esc` does not close the quick-settings popup reliably | low | Rely on `GtkWindow` + `hide_on_close`; ensure it is a transient/focus-out popup. |

---

## 8. Accessibility

Reference: [HIG Accessibility](https://developer.gnome.org/hig/guidelines/accessibility.html)
and [HIG UI Styling](https://developer.gnome.org/hig/guidelines/ui-styling.html).

- Every interface element needs a descriptive accessible name. Icon-only buttons
  MUST set `accessible-name` (e.g. `button.update_property(&[AccessibleProperty::Label("...")])`
  or `set_tooltip_text` plus `accessible-name`).
- Never use colour as the only signal — pair it with an icon or text.
- Test with high-contrast mode, large-text mode, screen reader, and keyboard-only.
- Use standard style classes and CSS variables rather than hardcoded colours; the
  app's EQ accent colour already follows `AdwStyleManager` (good).

### Audit

| Finding | Severity | Action |
|---|---|---|
| Several icon-only buttons (`list-remove-symbolic`, `list-add-symbolic`, trash, chevrons) lack accessible names | high | Add accessible names/tooltips. |
| Battery/status indicators rely partly on colour | low | Ensure a text fallback (percentages/labels) is shown. |

---

## 9. Visual style, dark mode and accent colour

Reference: [libadwaita Styles & Appearance](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/styles-and-appearance.html)
and [CSS variables](https://github.com/GNOME/libadwaita/blob/main/doc/css-variables.md).

- Follow the system colour scheme by default. If a per-app switcher is added, use
  `Adw.ColorScheme.prefer-light` for "System", `force-light`, `force-dark`.
- Accent colours are automatic. Use `.accent`, `--accent-bg-color`,
  `--accent-fg-color`; do not hardcode `#3584E4`.
- High contrast is automatic for stock widgets; custom drawing must handle it.
- Keep custom CSS to a minimum. If custom styles are needed, ship them as
  `style.css` in the app resource base path and use the
  `prefers-color-scheme: dark` / `prefers-contrast: more` media queries.

### Audit

| Finding | Severity | Action |
|---|---|---|
| No custom colour issues found; EQ follows accent | — | No change. |
| `metainfo.xml` claims MIT while the project is GPL-3.0-only | high | Fix the license fields (AppStream will reject mismatches on Flathub). |

---

## 10. GNOME desktop integration APIs worth using

| Need | API | Notes |
|---|---|---|
| Colour scheme / accent / contrast | `AdwStyleManager` | Already respected by stock widgets. |
| Persistent per-user preferences | `GSettings` via `gio::Settings` | Prefer over the current JSON file for Window state/settings; schema must be installed. |
| App-wide actions & shortcuts | `Gio.Action` + `GtkApplication::set_accels_for_action` | Works in every window. |
| Notifications | `Gio.Notification` (`Application::send_notification`) | For background events when the window is not visible. |
| Autostart | `Gio.AppInfo` / `X-GNOME-AutostartEnabled` desktop file | The app already writes an autostart entry. |
| Background/status icon | StatusNotifierItem (AppIndicator extension) | GNOME has no built-in tray; the app already targets SNI via `ksni`. |
| Appearance without GTK | `org.freedesktop.portal.Settings` (`org.freedesktop.appearance`) | Needed only for non-GTK consumers; `AdwStyleManager` is preferred here. |
| App metadata | `metainfo.xml` + `.desktop` | Required for Flathub; see below. |
| Sandboxing | Flatpak manifest | Already present; add `--talk-name=org.freedesktop.Flatpak`? only if needed. |

### App metadata checklist (Flathub / AppStream)

- `<id>`, `<name>`, `<summary>`, `<description>`, `<launchable>`,
  `<project_license>` must match the source tree.
- Prefer `<developer><name>…</name></developer>` over the deprecated
  `<developer_name>`.
- Add `<content_rating type="oars-1.1"/>`, `<releases>` with at least the current
  version, `<url type="bugtracker">`, and `<kudos>` where applicable.
- Desktop file needs `StartupWMClass` and useful `Keywords`.

---

## 11. Concrete change set applied in this branch

The items marked **applied** below were implemented in
`feat/oraimo-support-and-gnome-ui`. The rest are recorded as follow-ups.

**Applied**

1. `AdwAlertDialog` replaces the deprecated `GtkDialog` rename flow; the
   destructive "Clear Pairing Record" now asks for confirmation first.
2. Primary app menu (`open-menu-symbolic`) shared by the sidebar and content
   header bars, with About / Keyboard Shortcuts / Quit.
3. `AdwAboutWindow` with application name, version, license, website and credits.
4. Standard accelerators: `Ctrl+Q` quit, `Ctrl+W` close, `Ctrl+,` settings,
   `F9` toggle sidebar, `Ctrl+?` shortcuts.
5. Per-page header titles via `AdwWindowTitle`.
6. Sidebar toggle uses `adw-sidebar-symbolic` and is bound to
   `OverlaySplitView:show-sidebar`.
7. Accessible names/tooltips for icon-only controls in the quick-settings popup
   and the scanner.
8. Corrected `metainfo.xml` license/metadata and desktop-file fields.

**Follow-ups (require libadwaita ≥ 1.7)**

- Migrate `OverlaySplitView` → `NavigationSplitView` and drop the hand-rolled
  back button/stack wiring.
- Add an `AdwBanner` for "daemon not running".
- Migrate the quick-settings segmented control to `AdwToggleGroup`.
- Replace the JSON settings store with `GSettings`.
