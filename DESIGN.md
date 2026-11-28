---
name: Excalibur
description: High-contrast dark native egui chrome for a low-memory Rust + Chromium browser.
colors:
  bg-canvas: "#09090d"
  bg-tab-bar: "#0c0c11"
  bg-toolbar: "#111118"
  bg-elevated: "#1a1b24"
  bg-hover: "#242632"
  bg-field: "#07070a"
  border-subtle: "rgba(255, 255, 255, 0.08)"
  border-strong: "rgba(255, 255, 255, 0.16)"
  text-primary: "#ececf1"
  text-secondary: "#9494a3"
  text-muted: "#5e5e6e"
  accent: "#38bdf8"
  accent-soft: "rgba(14, 48, 66, 0.71)"
  status-success: "#3dd68c"
  status-warning: "#e5a93c"
  status-danger: "#f05252"
rounded:
  control: "4px"
spacing:
  item: "6px"
  button-padding: "4px 8px"
  tab-strip-height: "34px"
  toolbar-height: "40px"
components:
  tab-active:
    backgroundColor: "{colors.bg-toolbar}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.control}"
  tab-inactive:
    backgroundColor: "{colors.bg-tab-bar}"
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.control}"
  omnibox:
    backgroundColor: "{colors.bg-field}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.control}"
    padding: "4px 8px"
  toolbar-button:
    backgroundColor: "{colors.bg-elevated}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.control}"
    padding: "4px 8px"
  toolbar-button-hover:
    backgroundColor: "{colors.bg-hover}"
    textColor: "{colors.text-primary}"
  toolbar-button-active:
    backgroundColor: "{colors.accent-soft}"
    textColor: "{colors.text-primary}"
---

# Design System: Excalibur

## Overview

Excalibur uses a compact, high-contrast dark interface built in `egui` (`crates/excalibur-app/src/theme.rs`). Every surface is functional browser chrome sitting directly above the native CEF child viewport: dark neutral layering separates the tab strip, toolbar, inputs, and side drawer without drop shadows, decorative gradients, or filler copy.

Key characteristics:
- Operate-mode precision: small vertical footprint (`34pt` tab bar + `40pt` toolbar = `74pt` total top chrome).
- Flat tonal separation with hairline `1px` borders instead of box shadows.
- Custom `1.5px`-stroke vector icons (`crates/excalibur-app/src/icons.rs`) rather than ASCII placeholders or emojis.
- Tabular numeric telemetry for live RAM usage and blocked-request counters.

## Colors

Defined in `crates/excalibur-app/src/theme.rs`:

- **Surfaces (dark to elevated)**:
  - `BG_CANVAS` (`#09090d`): Base viewport / sleeping-tab backdrop.
  - `BG_FIELD` (`#07070a`): Recessed text inputs (omnibox, drawer search/settings inputs).
  - `BG_TAB_BAR` (`#0c0c11`): Top tab strip background.
  - `BG_TOOLBAR` (`#111118`): Main navigation toolbar and active tab fill.
  - `BG_ELEVATED` (`#1a1b24`): Buttons, cards, and side drawer (`Bookmarks`, `History`, `Settings`).
  - `BG_HOVER` (`#242632`): Hovered controls and list rows.
- **Borders**:
  - `BORDER_SUBTLE` (`Color32::from_rgba_premultiplied(255, 255, 255, 20)`): Default hairline dividers and inactive control strokes.
  - `BORDER_STRONG` (`Color32::from_rgba_premultiplied(255, 255, 255, 42)`): Hovered control borders.
- **Text**:
  - `TEXT_PRIMARY` (`#ececf1`): Active tab titles, primary labels, omnibox URL text.
  - `TEXT_SECONDARY` (`#9494a3`): Inactive tabs, secondary URL lines, helper text.
  - `TEXT_MUTED` (`#5e5e6e`): Disabled icons, sleeping-tab hints, empty-state metadata.
- **Accent & Semantic Status**:
  - `ACCENT` (`#38bdf8`): Active tab top indicator bar, focused omnibox border, selection stroke.
  - `ACCENT_SOFT` (`Color32::from_rgba_premultiplied(14, 48, 66, 180)`): Active/toggled button and selection fill.
  - `STATUS_SUCCESS` (`#3dd68c`): Shield enabled state, low memory readout, valid save confirmation.
  - `STATUS_WARNING` (`#e5a93c`): Bookmarked star icon, sleeping-tab indicator, elevated memory warning.
  - `STATUS_DANGER` (`#f05252`): Validation errors, high memory alert, destructive clear action.

## Typography

- Uses `egui` default proportional UI font for labels, tab titles, and drawer entries.
- Uses monospace / tabular alignment for live RAM telemetry (`MB` working set) and blocked-request counters so numbers do not jitter during 2-second updates.
- Hierarchy relies on size and color contrast (`TEXT_PRIMARY` for titles/actions, `TEXT_SECONDARY` for URLs/subtitles) rather than decorative weights or all-caps eyebrows.

## Layout

- **Top Tab Strip** (`34pt` height): Horizontal scrollable tab list with active top accent bar, pin/sleep/loading indicators, close button, and `+` new-tab button.
- **Navigation Toolbar** (`40pt` height): Back, Forward, Reload/Stop, expandable Omnibox (`Ctrl+L`), Bookmark star (`Ctrl+D`), Shield toggle + blocked counter, RAM telemetry pill (with hover process breakdown), `Sleep Idle` action, and Drawer toggles (`Bookmarks`, `History`, `Settings`).
- **Central Viewport**: `egui::CentralPanel::no_frame()` hosts the native OS child CEF browser window when the active tab is awake, or an inline centered restore card when the active tab is sleeping.
- **Right Side Drawer**: Optional `egui::Panel::right` for Bookmarks, History, and Settings without blocking the main toolbar.
- **Spacing**: `item_spacing = (6.0, 6.0)`, `button_padding = (8.0, 4.0)`.

## Elevation & Depth

Flat tonal layering with `1px` hairline borders (`BORDER_SUBTLE` / `BORDER_STRONG`). No drop shadows or blur effects are used on chrome panels, tabs, or drawers.

## Shapes

- Uniform `CornerRadius::same(4)` (`4px`) across buttons, tabs, text fields, and cards.
- Custom vector glyphs in `crates/excalibur-app/src/icons.rs` are drawn with `1.5px` strokes on a pixel-snapped square canvas (`ArrowLeft`, `ArrowRight`, `Reload`, `Close`, `Plus`, `StarOutline`, `StarFilled`, `Shield`, `Bookmarks`, `History`, `Settings`, `Moon`, `Pin`).

## Components

- **Icon Button (`icons::icon_button`)**: Compact square hit target using `BG_ELEVATED` (default), `BG_HOVER` (hover), `ACCENT_SOFT` (toggled), and muted stroke when disabled.
- **Omnibox**: Recessed `BG_FIELD` frame with `1px` `BORDER_SUBTLE` stroke switching to `ACCENT` when focused (`self.omnibox_id`), containing a frameless `TextEdit::singleline`.
- **Tab Item**: Shows optional pin/sleep/loading state, truncated title, close icon, and right-click context menu (`Pin`/`Unpin`, `Sleep Tab`/`Wake Tab`, `Close Tab`).
- **Sleeping Tab Restore Card**: Centered `BG_ELEVATED` card inside `BG_CANVAS` displaying the tab title, URL, and a `Wake Tab` action so sleeping tabs stay lightweight until explicitly needed.
- **RAM Telemetry Readout**: Compact status pill showing total OS working set (`MB`) colored by threshold (`STATUS_SUCCESS` / `STATUS_WARNING` / `STATUS_DANGER`), with per-process-type details on hover.

## Do's and Don'ts

- **Do** keep all top chrome compact (`74pt` combined height) so the CEF web viewport maximizes screen space.
- **Do** use tokens from `crates/excalibur-app/src/theme.rs` and vector icons from `crates/excalibur-app/src/icons.rs` for any new UI controls.
- **Do** provide explicit empty states and non-blocking inline feedback in drawers and sleeping-tab views.
- **Don't** use Unicode emojis, ASCII symbol stand-ins (`<`, `>`, `R`, `X`), drop shadows, or decorative gradients in browser chrome.
- **Don't** introduce a second webview or heavy modal overlay for settings, history, or telemetry.
