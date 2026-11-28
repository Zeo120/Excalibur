---
name: Excalibur
description: Zen-inspired dark zinc native egui chrome with switchable vertical sidebar and compact top bar.
colors:
  bg-shell: "#141417"
  bg-surface: "#1c1c21"
  bg-pill: "#232329"
  bg-hover: "#2e2e36"
  bg-field: "#0f0f12"
  border-hairline: "rgba(255, 255, 255, 0.06)"
  border-strong: "rgba(255, 255, 255, 0.15)"
  text-primary: "#f2f2f5"
  text-secondary: "#a1a1ae"
  text-muted: "#71717e"
  accent: "#eb7b54"
  accent-soft: "rgba(52, 29, 22, 0.82)"
  status-success: "#4ad492"
  status-warning: "#ebb347"
  status-danger: "#ef5f5f"
rounded:
  control: "7px"
  pill: "8px"
  viewport: "6px"
spacing:
  item: "6px"
  button-padding: "5px 8px"
  sidebar-width: "240px"
  compact-bar-height: "42px"
  viewport-inset: "6px"
components:
  tab-active:
    backgroundColor: "{colors.bg-pill}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.control}"
  tab-hover:
    backgroundColor: "{colors.bg-hover}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.control}"
  url-pill:
    backgroundColor: "{colors.bg-pill}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.pill}"
    padding: "4px 8px"
  icon-button:
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.control}"
  icon-button-hover:
    backgroundColor: "{colors.bg-hover}"
    textColor: "{colors.text-primary}"
---

# Design System: Excalibur

## Overview

Excalibur uses a warm dark zinc shell modeled on Zen Browser's quiet ergonomics (`crates/excalibur-app/src/theme.rs`). Chrome lives primarily in a collapsible `240pt` vertical sidebar (switchable to a single `42pt` compact top bar with `Ctrl+\` or `Alt+S`) while the embedded Chromium viewport sits inside a `6px` inset frame. Idle controls remain borderless and reveal tonal pill fills only when hovered or selected.

Key characteristics:
- Warm zinc dark palette (`#141417` shell, `#1c1c21` surface, `#232329` pills) with a restrained warm coral accent (`#eb7b54`).
- Native Windows `Segoe UI` and `Consolas` fonts loaded when available (`install_system_fonts`).
- Unified URL pill folding origin lock status, omnibox input, tracker blocker toggle/count, and bookmark star into one control.
- Hover-only tab close buttons, compact pinned-tab tiles, and domain monograms instead of text prefixes.

## Colors

Defined in `crates/excalibur-app/src/theme.rs`:

- **Surfaces**:
  - `BG_SHELL` (`#141417`): Outer browser shell, left vertical sidebar, compact top bar, and `6px` viewport gutter.
  - `BG_SURFACE` (`#1c1c21`): Side drawers (`Bookmarks`, `History`, `Settings`), sleeping-tab restore card, inactive pinned tiles, and domain monogram chips.
  - `BG_PILL` (`#232329`): Integrated URL pill, active tab pill, and drawer item cards.
  - `BG_HOVER` (`#2e2e36`): Borderless hover surface for tabs and icon buttons.
  - `BG_FIELD` (`#0f0f12`): Recessed text inputs and sleeping-tab backdrop.
- **Borders**:
  - `BORDER_HAIRLINE` (`Color32::from_rgba_premultiplied(255, 255, 255, 16)`): Subtle dividers, unfocused URL pill stroke, and `1px` viewport frame.
  - `BORDER_STRONG` (`Color32::from_rgba_premultiplied(255, 255, 255, 38)`): Active tab stroke.
- **Text**:
  - `TEXT_PRIMARY` (`#f2f2f5`): Active tabs, headings, and URL text.
  - `TEXT_SECONDARY` (`#a1a1ae`): Inactive tabs, URL subtitles, and secondary labels.
  - `TEXT_MUTED` (`#71717e`): Disabled states and sleeping tabs.
- **Accent & Semantic Status**:
  - `ACCENT` (`#eb7b54`): Focused URL pill border, loading indicator dot, active pin/monogram highlight, and primary action borders.
  - `ACCENT_SOFT` (`Color32::from_rgba_premultiplied(52, 29, 22, 210)`): Active button fill and primary button fill.
  - `STATUS_SUCCESS` (`#4ad492`): HTTPS lock indicator, shield enabled state, low RAM status dot.
  - `STATUS_WARNING` (`#ebb347`): Bookmarked star, sleeping-tab moon indicator, elevated RAM dot.
  - `STATUS_DANGER` (`#ef5f5f`): High RAM dot and validation errors.

## Typography

- **Proportional UI**: `Segoe UI` (`C:\Windows\Fonts\segoeui.ttf` on Windows, falling back to `egui` defaults on other platforms).
- **Monospace Data**: `Consolas` (`C:\Windows\Fonts\consola.ttf` on Windows) used strictly for live RAM (`MB`) readouts and blocked tracker counts.

## Layout

- **Sidebar Mode (default, `240pt` left panel)**:
  - Top row: Sidebar toggle (`Ctrl+\` / `Alt+S`), Back, Forward, Reload/Stop, and `+` New Tab.
  - Full-width URL pill directly beneath the navigation row.
  - Optional wrapped grid of pinned-tab tiles (`68x32pt`).
  - Scrollable vertical tab list (`30pt` rows) with domain monograms, hover-only close buttons, and an inline `+ New Tab` action row.
  - Bottom utility footer: colored RAM status dot + tabular `MB` readout on the left; `Sleep Idle`, `Bookmarks`, `History`, and `Settings` buttons on the right.
- **Compact Mode (`42pt` single top bar)**:
  - Single horizontal row holding Sidebar toggle, Back/Forward/Reload, compact URL pill (`200..320pt`), horizontally scrollable tab pills, `+`, RAM readout, and utility icons.
- **Inset Viewport**:
  - `CentralPanel` applies a `6px` margin around the embedded CEF browser area with a `1px` `BORDER_HAIRLINE` perimeter.

## Elevation & Depth

Flat tonal hierarchy using `BG_SHELL` -> `BG_SURFACE` -> `BG_PILL` -> `BG_HOVER` and `1px` structural hairlines (`BORDER_HAIRLINE`). No drop shadows or decorative blur overlays are used.

## Shapes

- Controls and tabs use `CornerRadius::same(7)`; URL pills use `CornerRadius::same(8)`; viewport framing uses `CornerRadius::same(6)`; sleeping-tab card uses `CornerRadius::same(10)`.
- Vector icons in `crates/excalibur-app/src/icons.rs` use uniform `1.5px` strokes (`Sidebar`, `Back`, `Forward`, `Reload`, `Stop`, `Plus`, `Close`, `Pin`, `Lock`, `StarOutline`, `StarFilled`, `Shield`, `Sleep`, `Bookmarks`, `History`, `Settings`).

## Components

- **Icon Button (`icons::icon_button` / `icon_button_sized`)**: Transparent and borderless at rest, revealing `BG_HOVER` pill fill on hover and `ACCENT_SOFT` when selected.
- **URL Pill**: Houses HTTPS lock icon, select-all-on-focus `TextEdit`, inline `Shield` toggle + blocked counter, and `Star` bookmark button.
- **Tab Row / Pill**: Displays domain monogram (or loading dot / sleeping moon), truncated title, right-click context menu, middle-click close, and a close `×` target visible only on hover or active state.
- **Sleeping Tab Card**: Centered `BG_SURFACE` card over `BG_FIELD` backdrop with moon icon, truncated URL, and `Wake Tab` action.

## Do's and Don'ts

- **Do** keep idle buttons and tabs borderless, reserving strokes for structural dividers, focused inputs, and active tab pills.
- **Do** fold page-level actions (`Lock`, `Shield`, `Bookmark`) inside the URL pill rather than adding standalone toolbar clutter.
- **Do** support both Vertical Sidebar Mode and Compact Top Mode cleanly via shared rendering helpers.
- **Don't** render `[P]` ASCII prefixes, emojis, harsh boxed button grids, or side-accent stripes on rounded tabs.
