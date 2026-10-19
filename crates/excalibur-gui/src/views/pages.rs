//! Built-in `excalibur://` pages plus the loading and error states.

use super::browser::{BrowserAction, BrowserRenderer};
use crate::icons::{
    icon_button, paint_brand_mark, paint_icon, paint_site_badge, paint_spinner, url_host, Icon,
};
use crate::theme::{self, colors, UiPrefs, ACCENT_SWATCHES, PRESETS};
use crate::widgets::{
    badge, card, code_block, divider, empty_state, ghost_button, list_row, overline, page,
    page_title, paint_gradient, primary_button, search_field, section, segmented, setting_row,
    single_line, text_field, toggle,
};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Ui};
use excalibur_core::WorkspaceStorage;

const QUICK_SITES: [(&str, &str); 8] = [
    ("GitHub", "https://github.com"),
    ("YouTube", "https://www.youtube.com"),
    ("Hacker News", "https://news.ycombinator.com"),
    ("MDN", "https://developer.mozilla.org"),
    ("OWASP", "https://owasp.org"),
    ("PortSwigger", "https://portswigger.net/web-security"),
    ("Wikipedia", "https://wikipedia.org"),
    ("DuckDuckGo", "https://duckduckgo.com"),
];

const LABS: [(&str, &str, &str); 6] = [
    (
        "HTTPBin sandbox",
        "https://httpbin.org/html",
        "Request/response test suite",
    ),
    (
        "Cloudflare trace",
        "https://cloudflare.com/cdn-cgi/trace",
        "Edge, TLS SNI and IP diagnostics",
    ),
    (
        "CVE Details",
        "https://cvedetails.com",
        "Vulnerability database",
    ),
    ("HackerOne", "https://hackerone.com", "Bug bounty directory"),
    (
        "Example domain",
        "https://example.com",
        "RFC 2606 reference page",
    ),
    (
        "Local MITM proxy",
        "http://127.0.0.1:8080",
        "Excalibur proxy listener",
    ),
];

fn setting(storage: &WorkspaceStorage, key: &str, default: &str) -> String {
    storage
        .get_browser_setting(key)
        .ok()
        .flatten()
        .unwrap_or_else(|| default.to_string())
}

fn format_time(ts: i64, fmt: &str) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&chrono::Local).format(fmt).to_string())
        .unwrap_or_default()
}

fn day_label(ts: i64) -> String {
    let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) else {
        return String::new();
    };
    let day = dt.with_timezone(&chrono::Local).date_naive();
    let today = chrono::Local::now().date_naive();
    match (today - day).num_days() {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        _ => day.format("%A, %e %B %Y").to_string(),
    }
}

impl BrowserRenderer {
    // ─────────────────────────────── New tab ───────────────────────────────

    pub(super) fn render_new_tab(
        &mut self,
        ui: &mut Ui,
        storage: &WorkspaceStorage,
        action: &mut Option<BrowserAction>,
    ) {
        page(ui, "newtab_scroll", 720.0, |ui, width| {
            let now = chrono::Local::now();
            let greeting = match now.format("%H").to_string().parse::<u32>().unwrap_or(12) {
                5..=11 => "Good morning",
                12..=17 => "Good afternoon",
                _ => "Good evening",
            };
            ui.add_space((ui.available_height() * 0.08).min(60.0));
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(now.format("%H:%M").to_string())
                        .size(64.0)
                        .color(colors::text()),
                );
                ui.label(
                    RichText::new(format!("{} · {}", greeting, now.format("%A, %e %B")))
                        .size(14.0)
                        .color(colors::text_dim()),
                );
                ui.add_space(26.0);

                // Fake search field that opens the command bar
                let w = width.min(560.0);
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 48.0), Sense::click());
                let fill = if resp.hovered() {
                    colors::hover()
                } else {
                    colors::card()
                };
                ui.painter().rect_filled(rect, 14.0, fill);
                ui.painter().rect_stroke(
                    rect,
                    14.0,
                    Stroke::new(1.0_f32, colors::border()),
                    egui::StrokeKind::Inside,
                );
                paint_icon(
                    ui,
                    Rect::from_center_size(
                        pos2(rect.left() + 24.0, rect.center().y),
                        vec2(16.0, 16.0),
                    ),
                    Icon::Search,
                    colors::text_faint(),
                );
                ui.painter().text(
                    pos2(rect.left() + 44.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    "Search or enter address",
                    FontId::proportional(15.0),
                    colors::text_faint(),
                );
                ui.painter().text(
                    pos2(rect.right() - 16.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    "Ctrl L",
                    FontId::proportional(11.5),
                    colors::text_faint(),
                );
                if resp.on_hover_cursor(egui::CursorIcon::Text).clicked() {
                    *action = Some(BrowserAction::OpenCommandBar);
                }
            });

            ui.add_space(36.0);

            // Quick sites: bookmarks first, then defaults
            let mut sites: Vec<(String, String)> = storage
                .list_browser_bookmarks()
                .unwrap_or_default()
                .into_iter()
                .map(|b| {
                    let name = if b.title.is_empty() {
                        url_host(&b.url).to_string()
                    } else {
                        b.title
                    };
                    (name, b.url)
                })
                .collect();
            for (name, url) in QUICK_SITES {
                if sites.len() >= 8 {
                    break;
                }
                if !sites.iter().any(|(_, u)| u == url) {
                    sites.push((name.to_string(), url.to_string()));
                }
            }
            sites.truncate(8);

            let tile = 84.0_f32;
            let gap = 12.0_f32;
            let cols = (((width + gap) / (tile + gap)) as usize).clamp(2, 8);
            for row in sites.chunks(cols) {
                let row_w = row.len() as f32 * tile + (row.len() - 1) as f32 * gap;
                ui.horizontal(|ui| {
                    ui.add_space(((width - row_w) / 2.0).max(0.0));
                    ui.spacing_mut().item_spacing = vec2(gap, 0.0);
                    for (name, url) in row {
                        let (rect, resp) = ui.allocate_exact_size(vec2(tile, 88.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(rect, 14.0, colors::hover());
                        }
                        paint_site_badge(
                            ui,
                            Rect::from_center_size(
                                pos2(rect.center().x, rect.top() + 32.0),
                                vec2(40.0, 40.0),
                            ),
                            url,
                        );
                        let label = single_line(ui, name, 12.0, colors::text_dim(), tile - 8.0);
                        ui.painter().galley(
                            pos2(rect.center().x - label.size().x / 2.0, rect.bottom() - 24.0),
                            label,
                            colors::text_dim(),
                        );
                        if resp.on_hover_text(url.as_str()).clicked() {
                            *action = Some(BrowserAction::Navigate(url.clone()));
                        }
                    }
                });
                ui.add_space(gap);
            }

            ui.add_space(24.0);
            overline(ui, "Labs & diagnostics");
            ui.add_space(6.0);
            let two_cols = width >= 560.0;
            let col_w = if two_cols {
                (width - 10.0) / 2.0
            } else {
                width
            };
            for pair in LABS.chunks(if two_cols { 2 } else { 1 }) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(10.0, 0.0);
                    for (name, url, desc) in pair {
                        let (rect, resp) =
                            ui.allocate_exact_size(vec2(col_w, 52.0), Sense::click());
                        let fill = if resp.hovered() {
                            colors::hover()
                        } else {
                            colors::card()
                        };
                        ui.painter().rect_filled(rect, 12.0, fill);
                        paint_site_badge(
                            ui,
                            Rect::from_center_size(
                                pos2(rect.left() + 26.0, rect.center().y),
                                vec2(26.0, 26.0),
                            ),
                            url,
                        );
                        let tw = col_w - 70.0;
                        let t = single_line(ui, name, 13.0, colors::text(), tw);
                        ui.painter().galley(
                            pos2(rect.left() + 50.0, rect.top() + 9.0),
                            t,
                            colors::text(),
                        );
                        let d = single_line(ui, desc, 11.5, colors::text_faint(), tw);
                        ui.painter().galley(
                            pos2(rect.left() + 50.0, rect.top() + 28.0),
                            d,
                            colors::text_faint(),
                        );
                        if resp.on_hover_text(*url).clicked() {
                            *action = Some(BrowserAction::Navigate(url.to_string()));
                        }
                    }
                });
                ui.add_space(10.0);
            }
        });
    }

    // ─────────────────────────────── History ───────────────────────────────

    pub(super) fn render_history_page(
        &mut self,
        ui: &mut Ui,
        storage: &WorkspaceStorage,
        action: &mut Option<BrowserAction>,
    ) {
        page(ui, "history_scroll", 820.0, |ui, width| {
            ui.horizontal(|ui| {
                page_title(
                    ui,
                    "History",
                    "Every page you visit, searchable and stored locally.",
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.clear_history_confirm {
                        if ghost_button(ui, "Cancel", false).clicked() {
                            self.clear_history_confirm = false;
                        }
                        if ghost_button(ui, "Delete all history", true).clicked() {
                            let _ = storage.clear_browser_history();
                            self.clear_history_confirm = false;
                        }
                    } else if ghost_button(ui, "Clear history", true).clicked() {
                        self.clear_history_confirm = true;
                    }
                });
            });
            ui.add_space(18.0);
            search_field(
                ui,
                &mut self.history_search_query,
                "Search history",
                width,
                "history_search",
            );
            ui.add_space(14.0);

            let filter = Some(self.history_search_query.trim()).filter(|q| !q.is_empty());
            let entries = storage
                .list_browser_history(200, 0, filter)
                .unwrap_or_default();
            if entries.is_empty() {
                empty_state(
                    ui,
                    Icon::Clock,
                    "Nothing here yet",
                    "Pages you visit will show up here.",
                );
                return;
            }

            let mut delete_id = None;
            let mut current_day = String::new();
            for entry in &entries {
                let day = day_label(entry.visit_time);
                if day != current_day {
                    ui.add_space(10.0);
                    overline(ui, &day);
                    ui.add_space(4.0);
                    current_day = day;
                }
                let (rect, resp) = list_row(ui, 40.0, false);
                let hovered = ui.rect_contains_pointer(rect);
                let p = ui.painter();
                p.text(
                    pos2(rect.left() + 10.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    format_time(entry.visit_time, "%H:%M"),
                    FontId::monospace(11.5),
                    colors::text_faint(),
                );
                paint_site_badge(
                    ui,
                    Rect::from_center_size(
                        pos2(rect.left() + 70.0, rect.center().y),
                        vec2(20.0, 20.0),
                    ),
                    &entry.url,
                );
                let right_pad = if hovered { 120.0 } else { 90.0 };
                let avail = rect.width() - 90.0 - right_pad;
                let title = if entry.title.is_empty() {
                    &entry.url
                } else {
                    &entry.title
                };
                let t = single_line(ui, title, 13.0, colors::text(), avail * 0.6);
                let tw = t.size().x;
                ui.painter().galley(
                    pos2(rect.left() + 90.0, rect.center().y - t.size().y / 2.0),
                    t,
                    colors::text(),
                );
                let h = single_line(
                    ui,
                    url_host(&entry.url),
                    12.0,
                    colors::text_faint(),
                    avail - tw - 10.0,
                );
                ui.painter().galley(
                    pos2(rect.left() + 100.0 + tw, rect.center().y - h.size().y / 2.0),
                    h,
                    colors::text_faint(),
                );
                if entry.visit_count > 1 {
                    ui.painter().text(
                        pos2(
                            rect.right() - if hovered { 44.0 } else { 12.0 },
                            rect.center().y,
                        ),
                        Align2::RIGHT_CENTER,
                        format!("{}×", entry.visit_count),
                        FontId::proportional(11.5),
                        colors::text_faint(),
                    );
                }
                let mut deleted = false;
                if hovered {
                    let del = Rect::from_center_size(
                        pos2(rect.right() - 20.0, rect.center().y),
                        vec2(26.0, 26.0),
                    );
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(del));
                    if icon_button(&mut child, Icon::Trash, true, "Remove from history").clicked() {
                        delete_id = Some(entry.id);
                        deleted = true;
                    }
                }
                if resp.clicked() && !deleted {
                    *action = Some(BrowserAction::Navigate(entry.url.clone()));
                }
            }
            if let Some(id) = delete_id {
                let _ = storage.delete_browser_history_entry(id);
            }
        });
    }

    // ────────────────────────────── Bookmarks ──────────────────────────────

    pub(super) fn render_bookmarks_page(
        &mut self,
        ui: &mut Ui,
        storage: &WorkspaceStorage,
        action: &mut Option<BrowserAction>,
    ) {
        page(ui, "bookmarks_scroll", 820.0, |ui, width| {
            page_title(
                ui,
                "Bookmarks",
                "Bookmarks also appear as Essentials at the top of the sidebar.",
            );
            ui.add_space(18.0);

            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    let field_w = ((width - 160.0) / 3.0).max(120.0);
                    text_field(ui, &mut self.new_bookmark_title, "Name", field_w);
                    text_field(ui, &mut self.new_bookmark_url, "https://", field_w);
                    text_field(ui, &mut self.new_bookmark_folder, "Folder", field_w * 0.7);
                    if primary_button(ui, "Add").clicked() {
                        let url = self.new_bookmark_url.trim();
                        if !url.is_empty() {
                            let title = Some(self.new_bookmark_title.trim())
                                .filter(|t| !t.is_empty())
                                .unwrap_or(url);
                            let folder = Some(self.new_bookmark_folder.trim())
                                .filter(|f| !f.is_empty())
                                .unwrap_or("Favorites");
                            let _ = storage.add_browser_bookmark(url, title, folder);
                            self.new_bookmark_url.clear();
                            self.new_bookmark_title.clear();
                            *action = Some(BrowserAction::PreferencesChanged);
                        }
                    }
                });
            });
            ui.add_space(16.0);

            let mut bookmarks = storage.list_browser_bookmarks().unwrap_or_default();
            if bookmarks.is_empty() {
                empty_state(
                    ui,
                    Icon::Star,
                    "No bookmarks yet",
                    "Press Ctrl+D on any page to save it.",
                );
                return;
            }
            bookmarks.sort_by(|a, b| a.folder.cmp(&b.folder));

            let mut remove_url = None;
            let mut current_folder = String::new();
            for bm in &bookmarks {
                if bm.folder != current_folder {
                    ui.add_space(10.0);
                    overline(ui, &bm.folder);
                    ui.add_space(4.0);
                    current_folder = bm.folder.clone();
                }
                let (rect, resp) = list_row(ui, 44.0, false);
                let hovered = ui.rect_contains_pointer(rect);
                paint_site_badge(
                    ui,
                    Rect::from_center_size(
                        pos2(rect.left() + 22.0, rect.center().y),
                        vec2(24.0, 24.0),
                    ),
                    &bm.url,
                );
                let avail = rect.width() - 120.0;
                let t = single_line(ui, &bm.title, 13.0, colors::text(), avail);
                ui.painter().galley(
                    pos2(rect.left() + 44.0, rect.top() + 6.0),
                    t,
                    colors::text(),
                );
                let u = single_line(ui, &bm.url, 11.5, colors::text_faint(), avail);
                ui.painter().galley(
                    pos2(rect.left() + 44.0, rect.top() + 24.0),
                    u,
                    colors::text_faint(),
                );
                let mut consumed = false;
                if hovered {
                    let area = Rect::from_min_max(
                        pos2(rect.right() - 66.0, rect.top() + 8.0),
                        pos2(rect.right() - 6.0, rect.bottom() - 8.0),
                    );
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(area)
                            .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    );
                    if icon_button(&mut child, Icon::Trash, true, "Delete").clicked() {
                        remove_url = Some(bm.url.clone());
                        consumed = true;
                    }
                    if icon_button(&mut child, Icon::Copy, true, "Copy link").clicked() {
                        ui.ctx().copy_text(bm.url.clone());
                        consumed = true;
                    }
                }
                if resp.clicked() && !consumed {
                    *action = Some(BrowserAction::Navigate(bm.url.clone()));
                }
            }
            if let Some(url) = remove_url {
                let _ = storage.remove_browser_bookmark(&url);
                *action = Some(BrowserAction::PreferencesChanged);
            }
        });
    }

    // ────────────────────────────── Settings ──────────────────────────────

    pub(super) fn render_settings_page(
        &mut self,
        ui: &mut Ui,
        storage: &WorkspaceStorage,
        action: &mut Option<BrowserAction>,
    ) {
        page(ui, "settings_scroll", 760.0, |ui, width| {
            page_title(ui, "Settings", "Make Excalibur yours.");
            ui.add_space(20.0);

            let mut prefs = UiPrefs::load(storage);
            let before = prefs.clone();

            section(
                ui,
                "Theme",
                "Pick a base palette. Every surface, page and tool follows it.",
                |ui| {
                    let tile_w = 132.0_f32;
                    let gap = 10.0_f32;
                    let cols = (((width - 32.0 + gap) / (tile_w + gap)) as usize).max(1);
                    for row in PRESETS.chunks(cols) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(gap, 0.0);
                            for preset in row {
                                let selected = prefs.theme_id == preset.id;
                                if theme_tile(ui, preset, selected, tile_w).clicked() {
                                    prefs.theme_id = preset.id.to_string();
                                }
                            }
                        });
                        ui.add_space(gap);
                    }
                },
            );
            ui.add_space(12.0);

            section(ui, "Accent & surfaces", "", |ui| {
                setting_row(
                    ui,
                    "Accent color",
                    "Used for highlights, focus and selection",
                    |ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                        for (name, col) in ACCENT_SWATCHES.iter().rev() {
                            if swatch(ui, *col, prefs.accent == Some(*col))
                                .on_hover_text(*name)
                                .clicked()
                            {
                                prefs.accent = Some(*col);
                            }
                        }
                        let base = theme::preset_by_id(&prefs.theme_id).accent;
                        if swatch(ui, base, prefs.accent.is_none())
                            .on_hover_text("Theme default")
                            .clicked()
                        {
                            prefs.accent = None;
                        }
                    },
                );
                divider(ui);
                setting_row(
                    ui,
                    "Window gradient",
                    "Subtle vertical gradient behind the sidebar",
                    |ui| {
                        toggle(ui, &mut prefs.gradient);
                    },
                );
            });
            ui.add_space(12.0);

            section(ui, "Layout", "", |ui| {
                let mut layout = setting(storage, "tab_layout", "sidebar");
                if layout == "top" {
                    layout = "compact".into();
                }
                setting_row(
                    ui,
                    "Sidebar",
                    "Compact hides the sidebar until you hover the left edge",
                    |ui| {
                        if segmented(
                            ui,
                            &mut layout,
                            &[
                                ("sidebar".to_string(), "Always visible"),
                                ("compact".to_string(), "Compact"),
                            ],
                        ) {
                            let _ = storage.set_browser_setting("tab_layout", &layout);
                            *action = Some(BrowserAction::PreferencesChanged);
                        }
                    },
                );
                divider(ui);
                let mut essentials = setting(storage, "show_essentials", "on") == "on";
                setting_row(
                    ui,
                    "Show Essentials",
                    "Bookmark tiles at the top of the sidebar",
                    |ui| {
                        if toggle(ui, &mut essentials).changed() {
                            let _ = storage.set_browser_setting(
                                "show_essentials",
                                if essentials { "on" } else { "off" },
                            );
                            *action = Some(BrowserAction::PreferencesChanged);
                        }
                    },
                );
                divider(ui);
                let mut zoom = setting(storage, "default_zoom", "100%");
                setting_row(ui, "Default zoom", "Applied to new tabs", |ui| {
                    let opts: Vec<(String, &str)> = ["150%", "125%", "110%", "100%", "90%"]
                        .iter()
                        .rev()
                        .map(|z| (z.to_string(), *z))
                        .collect();
                    if segmented(ui, &mut zoom, &opts) {
                        let _ = storage.set_browser_setting("default_zoom", &zoom);
                    }
                });
            });
            ui.add_space(12.0);

            section(
                ui,
                "Search",
                "Used when what you type in the address bar isn't a URL.",
                |ui| {
                    let mut engine = setting(storage, "search_engine", "DuckDuckGo");
                    let opts: Vec<(String, &str)> =
                        ["DuckDuckGo", "Google", "Brave", "SearXNG", "YouTube"]
                            .iter()
                            .map(|e| (e.to_string(), *e))
                            .collect();
                    if segmented(ui, &mut engine, &opts) {
                        let _ = storage.set_browser_setting("search_engine", &engine);
                    }
                },
            );
            ui.add_space(12.0);

            section(
                ui,
                "Browser identity",
                "User-Agent sent with every request.",
                |ui| {
                    let current = setting(storage, "user_agent_preset", "Excalibur");
                    let presets = [
                        (
                            "Excalibur",
                            "Excalibur 1.0",
                            "Honest, non-Chromium identifier",
                        ),
                        ("Chrome", "Chrome 134", "Windows 11 · Blink"),
                        ("Firefox", "Firefox 136", "Linux · Gecko"),
                        ("Safari", "Safari 18", "macOS · WebKit"),
                        ("Curl", "curl 8.8", "Command-line scanner emulation"),
                    ];
                    for (key, name, hint) in presets {
                        let (rect, resp) = list_row(ui, 40.0, current == key);
                        ui.painter().text(
                            pos2(rect.left() + 12.0, rect.top() + 12.0),
                            Align2::LEFT_CENTER,
                            name,
                            FontId::proportional(13.0),
                            colors::text(),
                        );
                        ui.painter().text(
                            pos2(rect.left() + 12.0, rect.bottom() - 11.0),
                            Align2::LEFT_CENTER,
                            hint,
                            FontId::proportional(11.5),
                            colors::text_faint(),
                        );
                        if current == key {
                            paint_icon(
                                ui,
                                Rect::from_center_size(
                                    pos2(rect.right() - 20.0, rect.center().y),
                                    vec2(14.0, 14.0),
                                ),
                                Icon::Check,
                                colors::accent(),
                            );
                        }
                        if resp.clicked() {
                            let _ = storage.set_browser_setting("user_agent_preset", key);
                        }
                    }
                },
            );
            ui.add_space(12.0);

            section(ui, "Privacy", "", |ui| {
                setting_row(
                    ui,
                    "Browsing history",
                    "Stored locally in excalibur_browser.db",
                    |ui| {
                        if ghost_button(ui, "Clear history", true).clicked() {
                            let _ = storage.clear_browser_history();
                        }
                    },
                );
            });

            if prefs != before {
                prefs.save(storage);
                *action = Some(BrowserAction::PreferencesChanged);
            }
        });
    }

    // ─────────────────────────────── About ───────────────────────────────

    pub(super) fn render_about_page(&mut self, ui: &mut Ui, storage: &WorkspaceStorage) {
        page(ui, "about_scroll", 720.0, |ui, _width| {
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::hover());
                ui.painter().rect_filled(r, 20.0, colors::accent_soft());
                paint_brand_mark(ui, r.shrink(14.0), colors::accent());
                ui.add_space(12.0);
                ui.label(
                    RichText::new("Excalibur")
                        .size(30.0)
                        .strong()
                        .color(colors::text()),
                );
                ui.label(
                    RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                        .size(13.0)
                        .color(colors::text_faint()),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("An independent, non-Chromium browser and security workbench.")
                        .size(14.0)
                        .color(colors::text_dim()),
                );
            });
            ui.add_space(26.0);

            let pillars = [
                (
                    Icon::Globe,
                    "Independent engine",
                    "No Chromium, CEF, WebKit or Blink. Pure Rust HTML parsing and CSS cascade.",
                ),
                (
                    Icon::Code,
                    "Inspectable",
                    "Every DOM node, script and network frame is open to the built-in tools.",
                ),
                (
                    Icon::Shield,
                    "Security radar",
                    "Passive header audits, secret detection and QuickJS DOM-XSS sink tracing.",
                ),
                (
                    Icon::Network,
                    "MITM proxy",
                    "Local proxy on 127.0.0.1:8080 with on-the-fly certificate authority.",
                ),
            ];
            card(ui, |ui| {
                for (i, (icon, title, desc)) in pillars.iter().enumerate() {
                    if i > 0 {
                        divider(ui);
                    }
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                        ui.painter().rect_filled(r, 9.0, colors::accent_soft());
                        paint_icon(ui, r.shrink(9.0), *icon, colors::accent());
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(*title)
                                    .size(13.5)
                                    .strong()
                                    .color(colors::text()),
                            );
                            ui.label(RichText::new(*desc).size(12.5).color(colors::text_dim()));
                        });
                    });
                }
            });
            ui.add_space(12.0);

            let bookmarks = storage
                .list_browser_bookmarks()
                .map(|b| b.len())
                .unwrap_or(0);
            let history = storage
                .list_browser_history(1000, 0, None)
                .map(|h| h.len())
                .unwrap_or(0);
            card(ui, |ui| {
                setting_row(ui, "Storage", "SQLite with WAL and FTS5", |ui| {
                    badge(ui, "excalibur_browser.db", colors::text_dim());
                });
                divider(ui);
                setting_row(ui, "Bookmarks", "", |ui| {
                    badge(ui, &bookmarks.to_string(), colors::accent());
                });
                divider(ui);
                setting_row(ui, "History entries", "", |ui| {
                    badge(ui, &history.to_string(), colors::accent());
                });
                divider(ui);
                setting_row(ui, "Source", "", |ui| {
                    ui.label(
                        RichText::new("github.com/Zeo120/Excalibur")
                            .size(12.5)
                            .color(colors::link()),
                    );
                });
            });
        });
    }

    // ─────────────────────────── Loading & errors ───────────────────────────

    pub(super) fn render_loading(&self, ui: &mut Ui, url: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.32).max(40.0));
            let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
            paint_site_badge(ui, r.shrink(8.0), url);
            paint_spinner(ui, r, colors::accent());
            ui.add_space(14.0);
            ui.label(
                RichText::new(url_host(url))
                    .size(15.0)
                    .color(colors::text()),
            );
            ui.label(
                RichText::new("Loading…")
                    .size(12.5)
                    .color(colors::text_faint()),
            );
        });
    }

    pub(super) fn render_error(
        &self,
        ui: &mut Ui,
        url: &str,
        err: &str,
        action: &mut Option<BrowserAction>,
    ) {
        page(ui, "error_scroll", 560.0, |ui, _| {
            ui.add_space(60.0);
            let (r, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
            ui.painter().rect_filled(r, 14.0, colors::danger_soft());
            paint_icon(ui, r.shrink(14.0), Icon::Warning, colors::danger());
            ui.add_space(16.0);
            ui.label(
                RichText::new("Can't reach this page")
                    .size(24.0)
                    .strong()
                    .color(colors::text()),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "{} didn't respond or couldn't be reached.",
                    url_host(url)
                ))
                .size(14.0)
                .color(colors::text_dim()),
            );
            ui.add_space(16.0);
            code_block(ui, err);
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                if primary_button(ui, "Try again").clicked() {
                    *action = Some(BrowserAction::Navigate(url.to_string()));
                }
                if ghost_button(ui, "Start page", false).clicked() {
                    *action = Some(BrowserAction::Navigate("excalibur://newtab".to_string()));
                }
            });
        });
    }
}

/// Miniature window preview used to pick a theme.
fn theme_tile(
    ui: &mut Ui,
    preset: &theme::ThemePreset,
    selected: bool,
    width: f32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 104.0), Sense::click());
    let preview = Rect::from_min_size(rect.min, vec2(width, 76.0));
    let p = ui.painter();
    paint_gradient(p, preview, preset.chrome_top, preset.chrome_bottom, 10.0);
    // Sidebar rows
    let text_col = if preset.dark {
        Color32::from_white_alpha(60)
    } else {
        Color32::from_black_alpha(45)
    };
    for i in 0..3 {
        let y = preview.top() + 14.0 + i as f32 * 10.0;
        p.rect_filled(
            Rect::from_min_size(pos2(preview.left() + 8.0, y), vec2(26.0, 5.0)),
            2.5,
            if i == 0 { preset.accent } else { text_col },
        );
    }
    // Page card
    let page = Rect::from_min_max(
        pos2(preview.left() + 42.0, preview.top() + 7.0),
        pos2(preview.right() - 7.0, preview.bottom() - 7.0),
    );
    p.rect_filled(page, 6.0, preset.canvas);
    let ink = if preset.dark {
        Color32::from_white_alpha(40)
    } else {
        Color32::from_black_alpha(30)
    };
    p.rect_filled(
        Rect::from_min_size(page.min + vec2(8.0, 10.0), vec2(page.width() * 0.55, 6.0)),
        3.0,
        ink,
    );
    p.rect_filled(
        Rect::from_min_size(page.min + vec2(8.0, 22.0), vec2(page.width() * 0.75, 4.0)),
        2.0,
        ink,
    );
    p.rect_filled(
        Rect::from_min_size(page.min + vec2(8.0, 30.0), vec2(page.width() * 0.4, 4.0)),
        2.0,
        ink,
    );
    p.circle_filled(
        pos2(page.right() - 12.0, page.bottom() - 12.0),
        5.0,
        preset.accent,
    );

    let ring = if selected {
        Stroke::new(2.0_f32, colors::accent())
    } else if resp.hovered() {
        Stroke::new(1.0_f32, colors::border_strong())
    } else {
        Stroke::new(1.0_f32, colors::border())
    };
    p.rect_stroke(preview, 10.0, ring, egui::StrokeKind::Outside);
    let label_col = if selected {
        colors::text()
    } else {
        colors::text_dim()
    };
    p.text(
        pos2(rect.left() + 2.0, rect.bottom() - 10.0),
        Align2::LEFT_CENTER,
        preset.name,
        FontId::proportional(12.5),
        label_col,
    );
    p.text(
        pos2(rect.right() - 2.0, rect.bottom() - 10.0),
        Align2::RIGHT_CENTER,
        if preset.dark { "Dark" } else { "Light" },
        FontId::proportional(11.0),
        colors::text_faint(),
    );
    resp
}

fn swatch(ui: &mut Ui, color: Color32, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let c = rect.center();
    if selected {
        ui.painter()
            .circle_stroke(c, 11.0, Stroke::new(2.0_f32, color));
    } else if resp.hovered() {
        ui.painter()
            .circle_stroke(c, 11.0, Stroke::new(1.0_f32, colors::border_strong()));
    }
    ui.painter()
        .circle_filled(c, if selected { 7.5 } else { 9.0 }, color);
    resp
}
