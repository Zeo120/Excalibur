//! Zen-style browser chrome: vertical sidebar (address pill, Essentials, pinned and
//! regular tabs), compact auto-hide mode, floating command bar, popovers and the
//! X-Mode workbench header.

use crate::app::{count_dom_nodes, BrowserTab, GuiApp, TabNavLayout, XModeLayout, XModeTab};
use crate::icons::{
    chrome_button, icon_button_ex, is_internal_url, paint_brand_mark, paint_icon, paint_site_badge,
    paint_spinner, url_host, Icon,
};
use crate::theme::{self, colors, ACCENT_SWATCHES, PRESETS};
use crate::widgets::{
    divider, menu_item, overline, paint_gradient, popup_frame, single_line, status_color,
};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Frame, Id, Key, Margin, Modifiers,
    Rect, RichText, Sense, Stroke, Ui,
};
use excalibur_core::storage::OmniboxMatchType;

pub const SIDEBAR_WIDTH: f32 = 248.0;
const COMMAND_INPUT_ID: &str = "excalibur_command_input";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Popover {
    Menu,
    Theme,
    PageInfo,
}

/// Transient chrome UI state (not persisted).
pub struct ChromeState {
    pub command_open: bool,
    pub command_input: String,
    pub command_selected: usize,
    pub command_focus_pending: bool,
    command_cache_key: Option<String>,
    pub popover: Option<Popover>,
    popover_anchor: Rect,
    popover_just_opened: bool,
    pub compact_peek: bool,
    pub card_rect: Rect,
    pub applied_prefs: Option<theme::UiPrefs>,
}

impl Default for ChromeState {
    fn default() -> Self {
        Self {
            command_open: false,
            command_input: String::new(),
            command_selected: 0,
            command_focus_pending: false,
            command_cache_key: None,
            popover: None,
            popover_anchor: Rect::NOTHING,
            popover_just_opened: false,
            compact_peek: false,
            card_rect: Rect::NOTHING,
            applied_prefs: None,
        }
    }
}

/// One row in the command bar.
enum CommandItem {
    Go {
        icon: Icon,
        label: String,
        detail: String,
        target: String,
    },
    Tab {
        id: usize,
        title: String,
        url: String,
    },
}

/// Deferred tab operations collected while iterating the tab list.
enum TabCmd {
    Select(usize),
    Close(usize),
    TogglePin(usize),
    Duplicate(usize),
    CloseOthers(usize),
    ClearUnpinned,
    NewTab,
}

impl GuiApp {
    // ───────────────────────────── Tab management ─────────────────────────────

    /// Opens a new tab on the start page and focuses the command bar for it.
    pub fn open_new_tab(&mut self) {
        let new_id = self.next_tab_id;
        self.next_tab_id += 1;
        let mut tab = BrowserTab::new(new_id, "excalibur://newtab");
        let zoom = self
            .storage
            .get_browser_setting("default_zoom")
            .ok()
            .flatten()
            .and_then(|z| z.trim_end_matches('%').parse::<f32>().ok());
        if let Some(z) = zoom {
            tab.zoom = (z / 100.0).clamp(0.6, 2.5);
        }
        self.tabs.push(tab);
        self.select_tab(new_id);
        self.open_command_bar(false);
    }

    pub fn reopen_closed_tab(&mut self) {
        if let Some(url) = self.closed_tabs_history.pop() {
            self.open_new_tab();
            self.chrome.command_open = false;
            self.navigate(self.active_tab_id, &url);
        }
    }

    pub fn select_tab(&mut self, id: usize) {
        if let Some(tab) = self.tabs.iter().find(|t| t.id == id) {
            self.active_tab_id = id;
            self.omnibox_input = tab.url.clone();
        }
    }

    /// Closes a tab, remembering it for Ctrl+Shift+T and activating its neighbour.
    pub fn close_tab(&mut self, id: usize) {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        let removed = self.tabs.remove(idx);
        if let Some(task) = &removed.nav_task {
            task.cancel();
        }
        if !is_internal_url(&removed.url) {
            self.closed_tabs_history.push(removed.url);
        }
        if self.tabs.is_empty() {
            self.open_new_tab();
            self.chrome.command_open = false;
            return;
        }
        if self.active_tab_id == id {
            let next = self.tabs[idx.min(self.tabs.len() - 1)].id;
            self.select_tab(next);
        }
    }

    /// Zen's "Clear": closes every unpinned tab.
    pub fn clear_unpinned_tabs(&mut self) {
        let ids: Vec<usize> = self
            .tabs
            .iter()
            .filter(|t| !t.pinned)
            .map(|t| t.id)
            .collect();
        for id in ids {
            self.close_tab(id);
        }
    }

    pub fn toggle_tab_layout(&mut self) {
        self.tab_nav_layout = match self.tab_nav_layout {
            TabNavLayout::Sidebar => TabNavLayout::Compact,
            TabNavLayout::Compact => TabNavLayout::Sidebar,
        };
        self.chrome.compact_peek = false;
        let _ = self
            .storage
            .set_browser_setting("tab_layout", self.tab_nav_layout.as_setting());
    }

    pub fn toggle_bookmark_for_active(&mut self) {
        let Some((url, title)) = self.active_tab().map(|t| (t.url.clone(), t.title.clone())) else {
            return;
        };
        if url.is_empty() || is_internal_url(&url) {
            return;
        }
        if self.cached_bookmarks.iter().any(|b| b.url == url) {
            let _ = self.storage.remove_browser_bookmark(&url);
        } else {
            let _ = self
                .storage
                .add_browser_bookmark(&url, &title, "Essentials");
        }
        self.cached_bookmarks = self.storage.list_browser_bookmarks().unwrap_or_default();
    }

    pub fn go_back(&mut self) {
        let id = self.active_tab_id;
        if let Some(tab) = self.active_tab_mut() {
            if tab.history_idx > 0 {
                tab.history_idx -= 1;
                let url = tab.history[tab.history_idx].clone();
                self.navigate(id, &url);
            }
        }
    }

    pub fn go_forward(&mut self) {
        let id = self.active_tab_id;
        if let Some(tab) = self.active_tab_mut() {
            if tab.history_idx + 1 < tab.history.len() {
                tab.history_idx += 1;
                let url = tab.history[tab.history_idx].clone();
                self.navigate(id, &url);
            }
        }
    }

    pub fn open_command_bar(&mut self, _new_tab: bool) {
        let url = self.active_tab().map(|t| t.url.clone()).unwrap_or_default();
        self.chrome.command_input = if is_internal_url(&url) {
            String::new()
        } else {
            url
        };
        self.chrome.command_open = true;
        self.chrome.command_selected = 0;
        self.chrome.command_focus_pending = true;
        self.chrome.command_cache_key = None;
        self.chrome.popover = None;
    }

    fn toggle_popover(&mut self, kind: Popover, anchor: Rect) {
        if self.chrome.popover == Some(kind) {
            self.chrome.popover = None;
        } else {
            self.chrome.popover = Some(kind);
            self.chrome.popover_anchor = anchor;
            self.chrome.popover_just_opened = true;
        }
    }

    fn apply_tab_cmd(&mut self, cmd: TabCmd) {
        match cmd {
            TabCmd::Select(id) => self.select_tab(id),
            TabCmd::Close(id) => self.close_tab(id),
            TabCmd::TogglePin(id) => {
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
                    t.pinned = !t.pinned;
                }
                // Keep pinned tabs grouped at the front
                self.tabs.sort_by_key(|t| !t.pinned);
            }
            TabCmd::Duplicate(id) => {
                if let Some(url) = self.tabs.iter().find(|t| t.id == id).map(|t| t.url.clone()) {
                    self.open_new_tab();
                    self.chrome.command_open = false;
                    self.navigate(self.active_tab_id, &url);
                }
            }
            TabCmd::ClearUnpinned => self.clear_unpinned_tabs(),
            TabCmd::NewTab => self.open_new_tab(),
            TabCmd::CloseOthers(id) => {
                let others: Vec<usize> = self
                    .tabs
                    .iter()
                    .filter(|t| t.id != id && !t.pinned)
                    .map(|t| t.id)
                    .collect();
                for o in others {
                    self.close_tab(o);
                }
            }
        }
    }

    // ─────────────────────────────── Sidebar ───────────────────────────────

    pub(crate) fn render_sidebar(&mut self, ui: &mut Ui) {
        let mut cmds: Vec<TabCmd> = Vec::new();
        let (can_back, can_fwd, loading) = self.active_tab().map_or((false, false, false), |t| {
            (
                t.history_idx > 0,
                t.history_idx + 1 < t.history.len(),
                t.is_loading,
            )
        });

        // Top row: layout toggle · navigation
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
            let tip = match self.tab_nav_layout {
                TabNavLayout::Sidebar => "Compact mode (Ctrl+Shift+S)",
                TabNavLayout::Compact => "Keep sidebar open (Ctrl+Shift+S)",
            };
            if chrome_button(ui, Icon::Sidebar, true, tip).clicked() {
                self.toggle_tab_layout();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                if loading {
                    if chrome_button(ui, Icon::Close, true, "Stop").clicked() {
                        self.stop_loading(self.active_tab_id);
                    }
                } else if chrome_button(ui, Icon::Reload, true, "Reload (Ctrl+R)").clicked() {
                    self.reload_active();
                }
                if chrome_button(ui, Icon::Forward, can_fwd, "Forward (Alt+→)").clicked() {
                    self.go_forward();
                }
                if chrome_button(ui, Icon::Back, can_back, "Back (Alt+←)").clicked() {
                    self.go_back();
                }
            });
        });
        ui.add_space(6.0);

        self.render_address_pill(ui);
        ui.add_space(10.0);

        if self.show_favorites_bar && !self.cached_bookmarks.is_empty() {
            self.render_essentials(ui);
            ui.add_space(10.0);
        }

        // Tabs
        let bottom_h = 40.0;
        let list_h = (ui.available_height() - bottom_h).max(80.0);
        egui::ScrollArea::vertical()
            .id_salt("sidebar_tabs")
            .max_height(list_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                let multiple = self.tabs.len() > 1;
                let pinned: Vec<&BrowserTab> = self.tabs.iter().filter(|t| t.pinned).collect();
                for tab in &pinned {
                    tab_row(ui, tab, tab.id == self.active_tab_id, multiple, &mut cmds);
                }

                // Divider with Clear (Zen)
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let has_unpinned = self.tabs.iter().any(|t| !t.pinned);
                    let w = ui.available_width();
                    let (r, _) = ui.allocate_exact_size(vec2(w - 56.0, 16.0), Sense::hover());
                    ui.painter().hline(
                        (r.left() + 6.0)..=r.right(),
                        r.center().y,
                        Stroke::new(1.0_f32, colors::chrome_hover().gamma_multiply(2.0)),
                    );
                    if has_unpinned {
                        let clear = ui.add(
                            egui::Button::new(
                                RichText::new("Clear")
                                    .size(11.5)
                                    .color(colors::text_faint()),
                            )
                            .frame(false),
                        );
                        if clear.on_hover_text("Close all unpinned tabs").clicked() {
                            self.chrome.popover = None;
                            cmds.push(TabCmd::ClearUnpinned);
                        }
                    }
                });
                ui.add_space(4.0);

                // New tab row
                let (rect, resp) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 10.0, colors::chrome_hover());
                }
                paint_icon(
                    ui,
                    Rect::from_center_size(
                        pos2(rect.left() + 19.0, rect.center().y),
                        vec2(13.0, 13.0),
                    ),
                    Icon::Plus,
                    colors::text_dim(),
                );
                ui.painter().text(
                    pos2(rect.left() + 38.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    "New Tab",
                    FontId::proportional(13.0),
                    colors::text_dim(),
                );
                if resp.clicked() {
                    cmds.push(TabCmd::NewTab);
                }

                for tab in self.tabs.iter().filter(|t| !t.pinned) {
                    tab_row(ui, tab, tab.id == self.active_tab_id, multiple, &mut cmds);
                }
            });

        // Bottom bar
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                if chrome_button(ui, Icon::Gear, true, "Settings").clicked() {
                    self.navigate(self.active_tab_id, "excalibur://settings");
                }
                let findings = self.scanner_state.findings.len();
                let tint = self.x_mode_enabled.then(colors::danger);
                let shield = icon_button_ex(
                    ui,
                    Icon::Shield,
                    true,
                    "Security workbench (Ctrl+Shift+X)",
                    30.0,
                    tint,
                    true,
                );
                if findings > 0 {
                    ui.painter().circle_filled(
                        shield.rect.right_top() + vec2(-7.0, 7.0),
                        3.5,
                        colors::danger(),
                    );
                }
                if shield.clicked() {
                    self.x_mode_enabled = !self.x_mode_enabled;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                    let menu = chrome_button(ui, Icon::Menu, true, "Menu");
                    if menu.clicked() {
                        self.toggle_popover(Popover::Menu, menu.rect);
                    }
                    let pal = chrome_button(ui, Icon::Palette, true, "Theme");
                    if pal.clicked() {
                        self.toggle_popover(Popover::Theme, pal.rect);
                    }
                });
            });
        });

        for cmd in cmds {
            self.apply_tab_cmd(cmd);
        }
    }

    fn render_address_pill(&mut self, ui: &mut Ui) {
        let (url, title, loading) = self
            .active_tab()
            .map(|t| (t.url.clone(), t.title.clone(), t.is_loading))
            .unwrap_or_default();
        let internal = is_internal_url(&url);
        let bookmarked = self.cached_bookmarks.iter().any(|b| b.url == url);

        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
        let hovered = ui.rect_contains_pointer(rect);
        let fill = if hovered {
            colors::chrome_active()
        } else {
            colors::chrome_hover()
        };
        ui.painter().rect_filled(rect, 11.0, fill);

        // Identity / security chip
        let chip =
            Rect::from_center_size(pos2(rect.left() + 19.0, rect.center().y), vec2(26.0, 26.0));
        let chip_resp = ui.interact(chip, Id::new("address_chip"), Sense::click());
        if chip_resp.hovered() {
            ui.painter().rect_filled(chip, 7.0, colors::chrome_hover());
        }
        if loading {
            paint_spinner(ui, chip.shrink(6.0), colors::accent());
        } else if internal {
            paint_brand_mark(ui, chip.shrink(6.0), colors::accent());
        } else if url.starts_with("https://") {
            paint_icon(ui, chip.shrink(7.0), Icon::Lock, colors::text_dim());
        } else {
            paint_icon(ui, chip.shrink(6.5), Icon::Warning, colors::danger());
        }
        let chip_clicked = chip_resp.clicked();
        chip_resp.on_hover_text("Page info");
        if chip_clicked {
            self.toggle_popover(Popover::PageInfo, chip);
        }

        // Label
        let label = if internal {
            title.clone()
        } else {
            url_host(&url).to_string()
        };
        let right_pad = if hovered && !internal { 36.0 } else { 10.0 };
        let galley = single_line(
            ui,
            &label,
            13.0,
            colors::text(),
            rect.width() - 40.0 - right_pad,
        );
        ui.painter().galley(
            pos2(rect.left() + 38.0, rect.center().y - galley.size().y / 2.0),
            galley,
            colors::text(),
        );

        let mut consumed = chip_clicked;
        if hovered && !internal {
            let star_rect = Rect::from_center_size(
                pos2(rect.right() - 19.0, rect.center().y),
                vec2(26.0, 26.0),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(star_rect));
            let (icon, tint) = if bookmarked {
                (Icon::StarFilled, Some(colors::accent()))
            } else {
                (Icon::Star, None)
            };
            if icon_button_ex(
                &mut child,
                icon,
                true,
                "Bookmark (Ctrl+D)",
                26.0,
                tint,
                true,
            )
            .clicked()
            {
                self.toggle_bookmark_for_active();
                consumed = true;
            }
        }
        if resp.on_hover_text(&url).clicked() && !consumed {
            self.open_command_bar(false);
        }
    }

    fn render_essentials(&mut self, ui: &mut Ui) {
        let active_url = self.active_tab().map(|t| t.url.clone()).unwrap_or_default();
        let mut go = None;
        let cols = 3;
        let gap = 6.0;
        let tile_w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
        for row in self.cached_bookmarks.chunks(cols).take(3) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(gap, 0.0);
                for bm in row {
                    let (rect, resp) = ui.allocate_exact_size(vec2(tile_w, 46.0), Sense::click());
                    let is_current = bm.url == active_url;
                    let fill = if is_current {
                        colors::chrome_active()
                    } else if resp.hovered() {
                        colors::chrome_active().gamma_multiply(0.7)
                    } else {
                        colors::chrome_hover()
                    };
                    ui.painter().rect_filled(rect, 12.0, fill);
                    if is_current {
                        ui.painter().rect_stroke(
                            rect,
                            12.0,
                            Stroke::new(1.5_f32, colors::accent()),
                            egui::StrokeKind::Inside,
                        );
                    }
                    paint_site_badge(
                        ui,
                        Rect::from_center_size(rect.center(), vec2(20.0, 20.0)),
                        &bm.url,
                    );
                    let name = if bm.title.is_empty() {
                        url_host(&bm.url)
                    } else {
                        &bm.title
                    };
                    if resp.on_hover_text(name).clicked() {
                        go = Some(bm.url.clone());
                    }
                }
            });
            ui.add_space(gap);
        }
        if let Some(url) = go {
            self.navigate(self.active_tab_id, &url);
        }
    }

    /// Floating sidebar for compact mode, revealed from the left window edge.
    pub(crate) fn render_compact_sidebar(&mut self, ctx: &egui::Context) {
        let screen = ctx.screen_rect();
        let width = SIDEBAR_WIDTH;
        let panel = Rect::from_min_size(pos2(8.0, 8.0), vec2(width, screen.height() - 16.0));
        if let Some(p) = ctx.pointer_hover_pos() {
            if p.x <= screen.left() + 6.0 {
                self.chrome.compact_peek = true;
            } else if !panel.expand(24.0).contains(p) && self.chrome.popover.is_none() {
                self.chrome.compact_peek = false;
            }
        }
        let t = ctx.animate_bool_with_time(Id::new("compact_peek"), self.chrome.compact_peek, 0.16);
        if t <= 0.0 {
            return;
        }
        let x = egui::lerp((-width - 20.0)..=8.0, t);
        egui::Area::new(Id::new("compact_sidebar"))
            .order(egui::Order::Middle)
            .fixed_pos(pos2(x, 8.0))
            .show(ctx, |ui| {
                let rect = Rect::from_min_size(pos2(x, 8.0), panel.size());
                let p = ui.painter();
                p.add(
                    egui::Shadow {
                        offset: [0, 10],
                        blur: 30,
                        spread: 0,
                        color: colors::shadow(),
                    }
                    .as_shape(rect, 14),
                );
                paint_gradient(p, rect, colors::chrome_top(), colors::chrome_bottom(), 14.0);
                p.rect_stroke(
                    rect,
                    14.0,
                    Stroke::new(1.0_f32, colors::chrome_hover().gamma_multiply(2.0)),
                    egui::StrokeKind::Inside,
                );
                let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(8.0)));
                self.render_sidebar(&mut inner);
                ui.allocate_rect(rect, Sense::hover());
            });
    }

    // ───────────────────────────── Command bar ─────────────────────────────

    fn command_items(&mut self) -> Vec<CommandItem> {
        let input = self.chrome.command_input.trim().to_string();
        let mut items = Vec::new();
        let tab_item = |t: &BrowserTab| CommandItem::Tab {
            id: t.id,
            title: t.title.clone(),
            url: t.url.clone(),
        };

        if input.is_empty() {
            for t in self
                .tabs
                .iter()
                .filter(|t| t.id != self.active_tab_id)
                .take(6)
            {
                items.push(tab_item(t));
            }
            for (label, target, icon) in [
                ("History", "excalibur://history", Icon::Clock),
                ("Bookmarks", "excalibur://bookmarks", Icon::Star),
                ("Settings", "excalibur://settings", Icon::Gear),
            ] {
                items.push(CommandItem::Go {
                    icon,
                    label: label.into(),
                    detail: target.into(),
                    target: target.into(),
                });
            }
            return items;
        }

        let target = self.normalize_target(&input);
        let is_search = !target.contains(&input) || input.contains(' ');
        let engine = self
            .storage
            .get_browser_setting("search_engine")
            .ok()
            .flatten()
            .unwrap_or_else(|| "DuckDuckGo".into());
        items.push(CommandItem::Go {
            icon: if is_search { Icon::Search } else { Icon::Globe },
            label: input.clone(),
            detail: if is_search {
                format!("Search with {}", engine)
            } else {
                "Open".into()
            },
            target: target.clone(),
        });

        if self.chrome.command_cache_key.as_deref() != Some(input.as_str()) {
            self.omnibox_suggestions = self.storage.search_omnibox(&input, 6).unwrap_or_default();
            self.chrome.command_cache_key = Some(input.clone());
        }
        for m in &self.omnibox_suggestions {
            if m.url == target {
                continue;
            }
            let icon = match m.match_type {
                OmniboxMatchType::Bookmark => Icon::StarFilled,
                OmniboxMatchType::History => Icon::Clock,
                OmniboxMatchType::SearchSuggestion => Icon::Search,
            };
            items.push(CommandItem::Go {
                icon,
                label: if m.title.is_empty() {
                    m.url.clone()
                } else {
                    m.title.clone()
                },
                detail: url_host(&m.url).to_string(),
                target: m.url.clone(),
            });
        }

        let needle = input.to_lowercase();
        for t in self.tabs.iter().filter(|t| {
            t.id != self.active_tab_id
                && (t.title.to_lowercase().contains(&needle)
                    || t.url.to_lowercase().contains(&needle))
        }) {
            items.push(tab_item(t));
        }
        items.truncate(9);
        items
    }

    pub(crate) fn render_command_bar(&mut self, ctx: &egui::Context) {
        if !self.chrome.command_open {
            return;
        }
        let items = self.command_items();
        let n = items.len();

        // Keyboard handling before the text field sees the keys
        let (down, up, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if down && n > 0 {
            self.chrome.command_selected = (self.chrome.command_selected + 1) % n;
        }
        if up && n > 0 {
            self.chrome.command_selected = (self.chrome.command_selected + n - 1) % n;
        }
        self.chrome.command_selected = self.chrome.command_selected.min(n.saturating_sub(1));
        if esc {
            self.chrome.command_open = false;
            return;
        }

        let screen = ctx.screen_rect();
        let card = if self.chrome.card_rect.is_positive() {
            self.chrome.card_rect
        } else {
            screen
        };
        let backdrop = Color32::from_black_alpha(if theme::palette().dark { 110 } else { 60 });

        let width = (card.width() - 48.0).clamp(320.0, 640.0);
        let mut picked: Option<usize> = if enter && n > 0 {
            Some(self.chrome.command_selected)
        } else {
            None
        };
        let mut changed = false;

        let area = egui::Area::new(Id::new("command_bar"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(card.center().x, card.top() + card.height() * 0.16))
            .pivot(Align2::CENTER_TOP)
            .show(ctx, |ui| {
                // Dim everything behind the bar, painted first in the bar's own layer
                ui.painter()
                    .with_clip_rect(screen)
                    .rect_filled(screen, 0.0, backdrop);
                popup_frame()
                    .corner_radius(CornerRadius::same(16))
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.horizontal(|ui| {
                            ui.set_height(44.0);
                            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                            paint_icon(ui, r.shrink(9.0), Icon::Search, colors::text_faint());
                            let te_id = Id::new(COMMAND_INPUT_ID);
                            let te = ui.add(
                                egui::TextEdit::singleline(&mut self.chrome.command_input)
                                    .id(te_id)
                                    .frame(false)
                                    .font(FontId::proportional(17.0))
                                    .hint_text("Search or enter address")
                                    .desired_width(ui.available_width() - 40.0),
                            );
                            if self.chrome.command_focus_pending {
                                te.request_focus();
                                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), te_id)
                                {
                                    let len = self.chrome.command_input.chars().count();
                                    state.cursor.set_char_range(Some(
                                        egui::text::CCursorRange::two(
                                            egui::text::CCursor::new(0),
                                            egui::text::CCursor::new(len),
                                        ),
                                    ));
                                    state.store(ui.ctx(), te_id);
                                }
                                self.chrome.command_focus_pending = false;
                            }
                            changed = te.changed();
                            ui.label(RichText::new("esc").size(11.0).color(colors::text_faint()));
                        });

                        if n > 0 {
                            divider(ui);
                            ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                            for (i, item) in items.iter().enumerate() {
                                let selected = i == self.chrome.command_selected;
                                let (rect, resp) =
                                    ui.allocate_exact_size(vec2(width, 42.0), Sense::click());
                                if selected {
                                    ui.painter().rect_filled(rect, 10.0, colors::accent_soft());
                                } else if resp.hovered() {
                                    ui.painter().rect_filled(rect, 10.0, colors::hover());
                                }
                                let badge = Rect::from_center_size(
                                    pos2(rect.left() + 22.0, rect.center().y),
                                    vec2(22.0, 22.0),
                                );
                                let (label, detail) = match item {
                                    CommandItem::Go {
                                        icon,
                                        label,
                                        detail,
                                        target,
                                    } => {
                                        if matches!(
                                            icon,
                                            Icon::Globe | Icon::StarFilled | Icon::Clock
                                        ) && !is_internal_url(target)
                                        {
                                            paint_site_badge(ui, badge, target);
                                        } else {
                                            paint_icon(
                                                ui,
                                                badge.shrink(4.0),
                                                *icon,
                                                colors::text_dim(),
                                            );
                                        }
                                        (label.as_str(), detail.as_str())
                                    }
                                    CommandItem::Tab { title, url, .. } => {
                                        paint_site_badge(ui, badge, url);
                                        (title.as_str(), "Switch to tab")
                                    }
                                };
                                let d = single_line(
                                    ui,
                                    detail,
                                    12.0,
                                    colors::text_faint(),
                                    width * 0.35,
                                );
                                let dw = d.size().x;
                                ui.painter().galley(
                                    pos2(
                                        rect.right() - 12.0 - dw,
                                        rect.center().y - d.size().y / 2.0,
                                    ),
                                    d,
                                    colors::text_faint(),
                                );
                                let l =
                                    single_line(ui, label, 14.0, colors::text(), width - 80.0 - dw);
                                ui.painter().galley(
                                    pos2(rect.left() + 44.0, rect.center().y - l.size().y / 2.0),
                                    l,
                                    colors::text(),
                                );
                                if resp.hovered()
                                    && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
                                {
                                    self.chrome.command_selected = i;
                                }
                                if resp.clicked() {
                                    picked = Some(i);
                                }
                            }
                        }
                    });
            });

        if changed {
            self.chrome.command_selected = 0;
        }
        let clicked_outside =
            area.response.clicked_elsewhere() && !self.chrome.command_focus_pending;
        if let Some(i) = picked {
            self.chrome.command_open = false;
            match &items[i] {
                CommandItem::Go { target, .. } => {
                    let t = target.clone();
                    self.navigate(self.active_tab_id, &t);
                }
                CommandItem::Tab { id, .. } => self.select_tab(*id),
            }
        } else if clicked_outside {
            self.chrome.command_open = false;
        }
    }

    // ─────────────────────────────── Popovers ───────────────────────────────

    pub(crate) fn render_popovers(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.chrome.popover else {
            return;
        };
        let anchor = self.chrome.popover_anchor;
        let (pos, pivot) = match kind {
            Popover::PageInfo => (anchor.left_bottom() + vec2(-6.0, 8.0), Align2::LEFT_TOP),
            Popover::Menu | Popover::Theme => {
                (anchor.left_top() + vec2(0.0, -8.0), Align2::LEFT_BOTTOM)
            }
        };
        let mut close = false;
        let area = egui::Area::new(Id::new("chrome_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .pivot(pivot)
            .constrain(true)
            .show(ctx, |ui| {
                popup_frame().show(ui, |ui| match kind {
                    Popover::Menu => close = self.menu_contents(ui),
                    Popover::Theme => self.theme_contents(ui),
                    Popover::PageInfo => close = self.page_info_contents(ui),
                })
            });
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        if self.chrome.popover_just_opened {
            self.chrome.popover_just_opened = false;
        } else if close || esc || area.response.clicked_elsewhere() {
            self.chrome.popover = None;
        }
    }

    /// Main menu; returns `true` when an item was chosen.
    fn menu_contents(&mut self, ui: &mut Ui) -> bool {
        ui.set_width(250.0);
        ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
        let id = self.active_tab_id;
        let mut chosen = false;
        if menu_item(ui, Some(Icon::Plus), "New tab", "Ctrl+T") {
            self.open_new_tab();
            chosen = true;
        }
        if !self.closed_tabs_history.is_empty()
            && menu_item(ui, Some(Icon::Reload), "Reopen closed tab", "Ctrl+Shift+T")
        {
            self.reopen_closed_tab();
            chosen = true;
        }
        divider(ui);

        let zoom = self.active_tab().map_or(1.0, |t| t.zoom);
        ui.horizontal(|ui| {
            ui.set_height(30.0);
            ui.add_space(10.0);
            ui.label(RichText::new("Zoom").size(13.0).color(colors::text_dim()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                if icon_button_ex(ui, Icon::Plus, zoom < 2.5, "Zoom in", 24.0, None, false)
                    .clicked()
                {
                    self.adjust_zoom(0.1);
                }
                let pct = ui.add(
                    egui::Button::new(
                        RichText::new(format!("{}%", (zoom * 100.0).round() as i32))
                            .size(12.0)
                            .color(colors::text()),
                    )
                    .frame(false)
                    .min_size(vec2(46.0, 24.0)),
                );
                if pct.on_hover_text("Reset (Ctrl+0)").clicked() {
                    if let Some(t) = self.active_tab_mut() {
                        t.zoom = 1.0;
                    }
                }
                if icon_button_ex(ui, Icon::Minus, zoom > 0.6, "Zoom out", 24.0, None, false)
                    .clicked()
                {
                    self.adjust_zoom(-0.1);
                }
            });
        });
        divider(ui);

        if menu_item(ui, Some(Icon::Search), "Find in page", "Ctrl+F") {
            if let Some(t) = self.active_tab_mut() {
                t.show_find = true;
            }
            chosen = true;
        }
        if menu_item(ui, Some(Icon::Clock), "History", "Ctrl+H") {
            self.navigate(id, "excalibur://history");
            chosen = true;
        }
        if menu_item(ui, Some(Icon::Star), "Bookmarks", "Ctrl+B") {
            self.navigate(id, "excalibur://bookmarks");
            chosen = true;
        }
        divider(ui);
        let layout_label = match self.tab_nav_layout {
            TabNavLayout::Sidebar => "Compact mode",
            TabNavLayout::Compact => "Keep sidebar open",
        };
        if menu_item(ui, Some(Icon::Sidebar), layout_label, "Ctrl+Shift+S") {
            self.toggle_tab_layout();
            chosen = true;
        }
        if menu_item(ui, Some(Icon::Code), "Inspect page", "") {
            self.x_mode_enabled = true;
            self.x_mode_tab = XModeTab::DomAndScripts;
            chosen = true;
        }
        divider(ui);
        if menu_item(ui, Some(Icon::Gear), "Settings", "") {
            self.navigate(id, "excalibur://settings");
            chosen = true;
        }
        if menu_item(ui, Some(Icon::Info), "About Excalibur", "") {
            self.navigate(id, "excalibur://about");
            chosen = true;
        }
        chosen
    }

    fn theme_contents(&mut self, ui: &mut Ui) {
        ui.set_width(236.0);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            overline(ui, "Theme");
        });
        ui.add_space(4.0);
        let mut prefs = self.prefs.clone();
        for row in PRESETS.chunks(3) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                for preset in row {
                    let (rect, resp) = ui.allocate_exact_size(vec2(72.0, 46.0), Sense::click());
                    paint_gradient(
                        ui.painter(),
                        rect,
                        preset.chrome_top,
                        preset.chrome_bottom,
                        9.0,
                    );
                    let page =
                        Rect::from_min_max(rect.min + vec2(22.0, 6.0), rect.max - vec2(6.0, 6.0));
                    ui.painter().rect_filled(page, 5.0, preset.canvas);
                    ui.painter()
                        .circle_filled(page.center(), 5.0, preset.accent);
                    let selected = prefs.theme_id == preset.id;
                    let ring = if selected {
                        Stroke::new(2.0_f32, colors::accent())
                    } else if resp.hovered() {
                        Stroke::new(1.0_f32, colors::border_strong())
                    } else {
                        Stroke::new(1.0_f32, colors::border())
                    };
                    ui.painter()
                        .rect_stroke(rect, 9.0, ring, egui::StrokeKind::Outside);
                    if resp.on_hover_text(preset.name).clicked() {
                        prefs.theme_id = preset.id.to_string();
                    }
                }
            });
            ui.add_space(6.0);
        }
        divider(ui);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            overline(ui, "Accent");
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            ui.add_space(4.0);
            let base = theme::preset_by_id(&prefs.theme_id).accent;
            let options = std::iter::once(("Theme default", base, None))
                .chain(ACCENT_SWATCHES.iter().map(|(n, c)| (*n, *c, Some(*c))));
            for (name, col, value) in options {
                let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                let selected = prefs.accent == value;
                if selected {
                    ui.painter()
                        .circle_stroke(rect.center(), 10.0, Stroke::new(2.0_f32, col));
                }
                ui.painter()
                    .circle_filled(rect.center(), if selected { 6.5 } else { 8.0 }, col);
                if resp.on_hover_text(name).clicked() {
                    prefs.accent = value;
                }
            }
        });
        ui.add_space(4.0);
        if menu_item(ui, Some(Icon::Gear), "All appearance settings", "") {
            self.navigate(self.active_tab_id, "excalibur://settings");
            self.chrome.popover = None;
        }
        if prefs != self.prefs {
            prefs.save(&self.storage);
            self.prefs = prefs;
        }
    }

    fn page_info_contents(&mut self, ui: &mut Ui) -> bool {
        ui.set_width(280.0);
        let Some(tab) = self.active_tab() else {
            return false;
        };
        let internal = is_internal_url(&tab.url);
        let (icon, col, headline, detail) = if internal {
            (
                None,
                colors::accent(),
                "Excalibur page",
                "Built into the browser.",
            )
        } else if tab.url.starts_with("https://") {
            (
                Some(Icon::Lock),
                colors::success(),
                "Connection is secure",
                "Traffic is encrypted with TLS.",
            )
        } else {
            (
                Some(Icon::Warning),
                colors::danger(),
                "Not secure",
                "Traffic is sent unencrypted.",
            )
        };
        let mut open_radar = false;
        Frame::NONE.inner_margin(Margin::same(8)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                ui.painter()
                    .rect_filled(r, 8.0, theme::mix(colors::card(), col, 0.18));
                match icon {
                    Some(i) => paint_icon(ui, r.shrink(8.0), i, col),
                    None => paint_brand_mark(ui, r.shrink(7.0), col),
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new(headline).strong().color(colors::text()));
                    ui.label(RichText::new(detail).size(12.0).color(colors::text_dim()));
                });
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(&tab.url)
                    .size(11.5)
                    .monospace()
                    .color(colors::text_faint()),
            );
            if !internal {
                divider(ui);
                let rows = [
                    (
                        "Response",
                        tab.status_code
                            .map_or("—".into(), |s| format!("HTTP {}", s)),
                        tab.status_code.map_or(colors::text_dim(), status_color),
                    ),
                    (
                        "Load time",
                        tab.elapsed_ms.map_or("—".into(), |ms| format!("{} ms", ms)),
                        colors::text_dim(),
                    ),
                    (
                        "DOM nodes",
                        tab.parsed_dom
                            .as_ref()
                            .map_or("—".into(), |d| count_dom_nodes(d).to_string()),
                        colors::text_dim(),
                    ),
                ];
                for (k, v, c) in rows {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(k).size(12.5).color(colors::text_faint()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(v).size(12.5).color(c));
                        });
                    });
                }
            }
        });
        let findings = self.scanner_state.findings.len();
        if menu_item(
            ui,
            Some(Icon::Radar),
            &format!("Security findings ({})", findings),
            "",
        ) {
            open_radar = true;
        }
        if open_radar {
            self.x_mode_enabled = true;
            self.x_mode_tab = XModeTab::Scanner;
        }
        open_radar
    }

    // ─────────────────────────── X-Mode workbench header ───────────────────────────

    pub(crate) fn render_x_mode_toolbar(&mut self, ui: &mut Ui) {
        let captured = self.interceptor_state.traffic_items.len();
        let findings = self.scanner_state.findings.len();
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            paint_icon(ui, r, Icon::Shield, colors::danger());
            ui.label(
                RichText::new("Workbench")
                    .strong()
                    .size(13.5)
                    .color(colors::text()),
            );
            ui.add_space(10.0);

            let tools = [
                (XModeTab::Interceptor, "Traffic", Some(captured)),
                (XModeTab::Repeater, "Repeater", None),
                (XModeTab::Scanner, "Radar", Some(findings)),
                (XModeTab::DomAndScripts, "Inspector", None),
                (XModeTab::ProxyManager, "Proxy", None),
            ];
            for (tab, label, count) in tools {
                let selected = self.x_mode_tab == tab;
                let col = if selected {
                    colors::text()
                } else {
                    colors::text_dim()
                };
                let galley =
                    ui.painter()
                        .layout_no_wrap(label.to_owned(), FontId::proportional(12.5), col);
                let count_text = count.filter(|c| *c > 0).map(|c| c.to_string());
                let count_w = count_text
                    .as_ref()
                    .map_or(0.0, |t| t.len() as f32 * 7.0 + 14.0);
                let (rect, resp) = ui.allocate_exact_size(
                    vec2(galley.size().x + 22.0 + count_w, 28.0),
                    Sense::click(),
                );
                if selected {
                    ui.painter().rect_filled(rect, 8.0, colors::hover());
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 8.0, colors::panel());
                }
                let tp = pos2(rect.left() + 11.0, rect.center().y - galley.size().y / 2.0);
                let tr = tp.x + galley.size().x;
                ui.painter().galley(tp, galley, col);
                if let Some(t) = count_text {
                    let pill = Rect::from_min_size(
                        pos2(tr + 6.0, rect.center().y - 8.0),
                        vec2(count_w - 8.0, 16.0),
                    );
                    let fg = if tab == XModeTab::Scanner {
                        colors::danger()
                    } else {
                        colors::text_dim()
                    };
                    ui.painter()
                        .rect_filled(pill, 8.0, theme::mix(colors::canvas(), fg, 0.16));
                    ui.painter().text(
                        pill.center(),
                        Align2::CENTER_CENTER,
                        t,
                        FontId::proportional(10.5),
                        fg,
                    );
                }
                if resp.clicked() {
                    self.x_mode_tab = tab;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                if icon_button_ex(
                    ui,
                    Icon::Close,
                    true,
                    "Close workbench (Ctrl+Shift+X)",
                    28.0,
                    None,
                    false,
                )
                .clicked()
                {
                    self.x_mode_enabled = false;
                }
                let (icon, tip, next) = match self.x_mode_layout {
                    XModeLayout::SplitDrawer => {
                        (Icon::Maximize, "Expand", XModeLayout::FullscreenTools)
                    }
                    XModeLayout::FullscreenTools => (
                        Icon::SplitBottom,
                        "Dock below page",
                        XModeLayout::SplitDrawer,
                    ),
                };
                if icon_button_ex(ui, icon, true, tip, 28.0, None, false).clicked() {
                    self.x_mode_layout = next;
                }
            });
        });
    }

    // ───────────────────────────── Card overlays ─────────────────────────────

    /// Indeterminate progress beam along the top edge of the page card.
    pub(crate) fn paint_loading_beam(&self, ui: &Ui, card: Rect) {
        if !self.active_tab().is_some_and(|t| t.is_loading) {
            return;
        }
        let track = Rect::from_min_max(
            pos2(card.left() + 14.0, card.top() + 1.0),
            pos2(card.right() - 14.0, card.top() + 3.0),
        );
        let cycle = (ui.input(|i| i.time) * 1.1).fract() as f32;
        let beam_w = (track.width() * 0.3).max(60.0);
        let x = track.left() + (track.width() + beam_w) * cycle - beam_w;
        let beam = Rect::from_min_max(
            pos2(x.max(track.left()), track.top()),
            pos2((x + beam_w).min(track.right()), track.bottom()),
        );
        if beam.width() > 0.0 {
            ui.painter().rect_filled(beam, 1.0, colors::accent());
        }
    }

    pub(crate) fn render_find_overlay(&mut self, ui: &mut Ui, raw_html: &str, find_query: &str) {
        let mut close = false;
        let mut new_query = None;
        let area_rect = ui.max_rect();
        let width = 320.0_f32.min(area_rect.width() - 24.0);
        let te_id = Id::new("find_in_page_input");
        let opened_id = Id::new("find_in_page_was_open");
        let was_open: bool = ui.data(|d| d.get_temp(opened_id).unwrap_or(false));

        egui::Area::new(Id::new("find_in_page_overlay"))
            .fixed_pos(pos2(
                area_rect.right() - width - 12.0,
                area_rect.top() + 12.0,
            ))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                popup_frame()
                    .inner_margin(Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.set_width(width - 16.0);
                        ui.horizontal(|ui| {
                            ui.set_height(30.0);
                            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            paint_icon(ui, r, Icon::Search, colors::text_faint());
                            let mut q = find_query.to_string();
                            let te = ui.add(
                                egui::TextEdit::singleline(&mut q)
                                    .id(te_id)
                                    .frame(false)
                                    .desired_width(width - 140.0)
                                    .hint_text("Find in page"),
                            );
                            if !was_open {
                                te.request_focus();
                            }
                            if te.changed() {
                                new_query = Some(q.clone());
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button_ex(
                                        ui,
                                        Icon::Close,
                                        true,
                                        "Close (Esc)",
                                        24.0,
                                        None,
                                        false,
                                    )
                                    .clicked()
                                    {
                                        close = true;
                                    }
                                    if !q.is_empty() {
                                        let hits = raw_html
                                            .to_lowercase()
                                            .matches(&q.to_lowercase())
                                            .count();
                                        let (text, col) = match hits {
                                            0 => ("No matches".to_string(), colors::danger()),
                                            1 => ("1 match".to_string(), colors::text_dim()),
                                            n => (format!("{} matches", n), colors::text_dim()),
                                        };
                                        ui.label(RichText::new(text).size(11.5).color(col));
                                    }
                                },
                            );
                        });
                    });
            });

        if ui.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }
        ui.data_mut(|d| d.insert_temp(opened_id, !close));
        if let Some(tab) = self.active_tab_mut() {
            if let Some(q) = new_query {
                tab.find_query = q;
            }
            if close {
                tab.show_find = false;
            }
        }
    }
}

/// One sidebar tab row with hover close button and context menu.
fn tab_row(ui: &mut Ui, tab: &BrowserTab, active: bool, can_close: bool, cmds: &mut Vec<TabCmd>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    let hovered = ui.rect_contains_pointer(rect);
    let p = ui.painter();
    if active {
        if !theme::palette().dark {
            p.add(
                egui::Shadow {
                    offset: [0, 1],
                    blur: 4,
                    spread: 0,
                    color: Color32::from_black_alpha(22),
                }
                .as_shape(rect, 10),
            );
        }
        p.rect_filled(rect, 10.0, colors::chrome_active());
    } else if hovered {
        p.rect_filled(rect, 10.0, colors::chrome_hover());
    }

    let badge = Rect::from_center_size(pos2(rect.left() + 19.0, rect.center().y), vec2(16.0, 16.0));
    if tab.is_loading {
        paint_spinner(ui, badge, colors::accent());
    } else {
        paint_site_badge(ui, badge, &tab.url);
    }

    let show_close = can_close && (hovered || active);
    let close_rect =
        Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(22.0, 22.0));
    let title_right = if show_close {
        close_rect.left() - 4.0
    } else {
        rect.right() - 10.0
    };
    let col = if active {
        colors::text()
    } else {
        colors::text_dim()
    };
    let galley = single_line(
        ui,
        &tab.title,
        13.0,
        col,
        title_right - badge.right() - 10.0,
    );
    ui.painter().galley(
        pos2(
            badge.right() + 10.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        col,
    );
    if tab.pinned && !show_close {
        paint_icon(
            ui,
            Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), vec2(11.0, 11.0)),
            Icon::Pin,
            colors::text_faint(),
        );
    }

    let mut close_clicked = false;
    if show_close {
        let c = ui.interact(close_rect, Id::new(("tab_close", tab.id)), Sense::click());
        if c.hovered() {
            ui.painter()
                .rect_filled(close_rect, 6.0, colors::chrome_hover());
        }
        paint_icon(
            ui,
            close_rect.shrink(6.0),
            Icon::Close,
            if c.hovered() {
                colors::text()
            } else {
                colors::text_faint()
            },
        );
        if c.clicked() {
            close_clicked = true;
            cmds.push(TabCmd::Close(tab.id));
        }
    }

    if resp.middle_clicked() {
        cmds.push(TabCmd::Close(tab.id));
    } else if resp.clicked() && !close_clicked {
        cmds.push(TabCmd::Select(tab.id));
    }
    resp.context_menu(|ui| {
        ui.set_min_width(180.0);
        if ui
            .button(if tab.pinned { "Unpin tab" } else { "Pin tab" })
            .clicked()
        {
            cmds.push(TabCmd::TogglePin(tab.id));
            ui.close_menu();
        }
        if ui.button("Duplicate tab").clicked() {
            cmds.push(TabCmd::Duplicate(tab.id));
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Close other tabs").clicked() {
            cmds.push(TabCmd::CloseOthers(tab.id));
            ui.close_menu();
        }
        if ui.button("Close tab").clicked() {
            cmds.push(TabCmd::Close(tab.id));
            ui.close_menu();
        }
    });
}
