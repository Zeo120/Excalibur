use excalibur_core::{BookmarkEntry, BrowserConfig, HistoryEntry, Storage};

use crate::{
    icons::{icon_button, IconKind},
    theme,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveDrawer {
    Bookmarks,
    History,
    Settings,
}

pub enum DrawerAction {
    NavigateActive(String),
    OpenNewTab(String),
    SaveSettings(BrowserConfig),
}

pub struct DrawerState {
    pub active: Option<ActiveDrawer>,
    pub history_query: String,
    pub bookmarks: Vec<BookmarkEntry>,
    pub history: Vec<HistoryEntry>,
    pub draft_home_url: String,
    pub draft_search_template: String,
    pub draft_renderer_limit: String,
    pub draft_sleep_secs: String,
    pub draft_ad_block: bool,
    pub status_message: Option<(bool, String)>,
}

impl DrawerState {
    pub fn new(config: &BrowserConfig) -> Self {
        Self {
            active: None,
            history_query: String::new(),
            bookmarks: Vec::new(),
            history: Vec::new(),
            draft_home_url: config.home_url.clone(),
            draft_search_template: config.search_template.clone(),
            draft_renderer_limit: config.renderer_process_limit.to_string(),
            draft_sleep_secs: config.tab_sleep_timeout_secs.to_string(),
            draft_ad_block: config.ad_block_enabled,
            status_message: None,
        }
    }

    pub fn toggle(&mut self, target: ActiveDrawer, storage: &Storage, config: &BrowserConfig) {
        if self.active == Some(target) {
            self.active = None;
            return;
        }
        self.active = Some(target);
        self.status_message = None;
        self.refresh(storage, config);
    }

    pub fn refresh(&mut self, storage: &Storage, config: &BrowserConfig) {
        match self.active {
            Some(ActiveDrawer::Bookmarks) => {
                self.bookmarks = storage.list_bookmarks().unwrap_or_default();
            }
            Some(ActiveDrawer::History) => {
                self.history = storage
                    .search_history(&self.history_query, 100)
                    .unwrap_or_default();
            }
            Some(ActiveDrawer::Settings) => {
                self.draft_home_url = config.home_url.clone();
                self.draft_search_template = config.search_template.clone();
                self.draft_renderer_limit = config.renderer_process_limit.to_string();
                self.draft_sleep_secs = config.tab_sleep_timeout_secs.to_string();
                self.draft_ad_block = config.ad_block_enabled;
            }
            None => {}
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, storage: &Storage) -> Option<DrawerAction> {
        let active = self.active?;
        let mut action = None;

        let panel_frame = egui::Frame::NONE
            .fill(theme::BG_SURFACE)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .stroke(egui::Stroke::new(1.0, theme::BORDER_HAIRLINE));

        egui::Panel::right("excalibur_drawer")
            .exact_size(320.0)
            .frame(panel_frame)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let heading = match active {
                        ActiveDrawer::Bookmarks => "Bookmarks",
                        ActiveDrawer::History => "History",
                        ActiveDrawer::Settings => "Settings",
                    };
                    ui.label(
                        egui::RichText::new(heading)
                            .size(15.0)
                            .strong()
                            .color(theme::TEXT_PRIMARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, IconKind::Close, true, false, None)
                            .on_hover_text("Close panel")
                            .clicked()
                        {
                            self.active = None;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(8.0);

                match active {
                    ActiveDrawer::Bookmarks => {
                        action = self.show_bookmarks(ui, storage);
                    }
                    ActiveDrawer::History => {
                        action = self.show_history(ui, storage);
                    }
                    ActiveDrawer::Settings => {
                        action = self.show_settings(ui);
                    }
                }
            });

        action
    }

    fn show_bookmarks(&mut self, ui: &mut egui::Ui, storage: &Storage) -> Option<DrawerAction> {
        let mut action = None;
        let mut deleted_url = None;

        if self.bookmarks.is_empty() {
            ui.label(
                egui::RichText::new(
                    "No bookmarks saved yet. Click the star icon or press Ctrl+D on any page.",
                )
                .color(theme::TEXT_SECONDARY),
            );
            return None;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for bm in &self.bookmarks {
                let row_frame = egui::Frame::NONE
                    .fill(theme::BG_PILL)
                    .corner_radius(egui::CornerRadius::same(7))
                    .inner_margin(egui::Margin::symmetric(8, 6));
                row_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_max_width(206.0);
                            if ui
                                .selectable_label(false, truncate_text(&bm.title, 26))
                                .on_hover_text(&bm.url)
                                .clicked()
                            {
                                action = Some(DrawerAction::NavigateActive(bm.url.clone()));
                            }
                            ui.label(
                                egui::RichText::new(truncate_text(&bm.url, 32))
                                    .size(11.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, IconKind::Close, true, false, None)
                                .on_hover_text("Remove bookmark")
                                .clicked()
                            {
                                deleted_url = Some(bm.url.clone());
                            }
                            if icon_button(ui, IconKind::Plus, true, false, None)
                                .on_hover_text("Open in new tab")
                                .clicked()
                            {
                                action = Some(DrawerAction::OpenNewTab(bm.url.clone()));
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
        });

        if let Some(url) = deleted_url {
            let _ = storage.remove_bookmark_by_url(&url);
            self.bookmarks = storage.list_bookmarks().unwrap_or_default();
        }
        action
    }

    fn show_history(&mut self, ui: &mut egui::Ui, storage: &Storage) -> Option<DrawerAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.history_query)
                    .desired_width(220.0)
                    .hint_text("Filter history by URL or title"),
            );
            if resp.changed() {
                self.history = storage
                    .search_history(&self.history_query, 100)
                    .unwrap_or_default();
            }
            if ui
                .button("Clear")
                .on_hover_text("Clear all browsing history")
                .clicked()
            {
                let _ = storage.clear_history();
                self.history.clear();
            }
        });
        ui.add_space(8.0);

        if self.history.is_empty() {
            let msg = if self.history_query.trim().is_empty() {
                "History is empty. Pages appear here once finished loading."
            } else {
                "No history entries match your filter."
            };
            ui.label(egui::RichText::new(msg).color(theme::TEXT_SECONDARY));
            return None;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for item in &self.history {
                let row_frame = egui::Frame::NONE
                    .fill(theme::BG_PILL)
                    .corner_radius(egui::CornerRadius::same(7))
                    .inner_margin(egui::Margin::symmetric(8, 6));
                row_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_max_width(232.0);
                            if ui
                                .selectable_label(false, truncate_text(&item.title, 28))
                                .on_hover_text(&item.url)
                                .clicked()
                            {
                                action = Some(DrawerAction::NavigateActive(item.url.clone()));
                            }
                            ui.label(
                                egui::RichText::new(truncate_text(&item.url, 36))
                                    .size(11.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, IconKind::Plus, true, false, None)
                                .on_hover_text("Open in new tab")
                                .clicked()
                            {
                                action = Some(DrawerAction::OpenNewTab(item.url.clone()));
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
        });

        action
    }

    fn show_settings(&mut self, ui: &mut egui::Ui) -> Option<DrawerAction> {
        let mut action = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.checkbox(
                &mut self.draft_ad_block,
                "Enable built-in ad and tracker blocker",
            );
            ui.add_space(8.0);

            ui.label(egui::RichText::new("Home URL").color(theme::TEXT_SECONDARY));
            ui.add(
                egui::TextEdit::singleline(&mut self.draft_home_url).desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);

            ui.label(
                egui::RichText::new("Search template (requires {query})")
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.draft_search_template)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);

            ui.label(
                egui::RichText::new("Max renderer processes (1-32, restart required)")
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.draft_renderer_limit)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);

            ui.label(
                egui::RichText::new("Idle tab sleep timeout in seconds (min 30)")
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.draft_sleep_secs).desired_width(f32::INFINITY),
            );
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                let save_btn = egui::Button::new(
                    egui::RichText::new("Save Settings")
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                )
                .fill(theme::ACCENT_SOFT)
                .stroke(egui::Stroke::new(1.0, theme::ACCENT));

                if ui.add(save_btn).clicked() {
                    let home = self.draft_home_url.trim();
                    let search = self.draft_search_template.trim();
                    let limit = self.draft_renderer_limit.trim().parse::<u32>();
                    let sleep = self.draft_sleep_secs.trim().parse::<u64>();

                    match (home.is_empty(), search.contains("{query}"), limit, sleep) {
                        (false, true, Ok(limit), Ok(sleep)) => {
                            let cfg = BrowserConfig {
                                home_url: home.to_owned(),
                                search_template: search.to_owned(),
                                renderer_process_limit: limit.clamp(1, 32),
                                tab_sleep_timeout_secs: sleep.max(30),
                                ad_block_enabled: self.draft_ad_block,
                            };
                            self.status_message = Some((true, "Settings saved.".to_owned()));
                            action = Some(DrawerAction::SaveSettings(cfg));
                        }
                        (true, _, _, _) => {
                            self.status_message =
                                Some((false, "Home URL cannot be empty.".to_owned()));
                        }
                        (_, false, _, _) => {
                            self.status_message =
                                Some((false, "Search template must include {query}.".to_owned()));
                        }
                        _ => {
                            self.status_message = Some((
                                false,
                                "Enter valid numbers for limits and timeout.".to_owned(),
                            ));
                        }
                    }
                }

                if ui
                    .button("Reset Defaults")
                    .on_hover_text("Restore default settings in form")
                    .clicked()
                {
                    let defaults = BrowserConfig::default();
                    self.draft_home_url = defaults.home_url;
                    self.draft_search_template = defaults.search_template;
                    self.draft_renderer_limit = defaults.renderer_process_limit.to_string();
                    self.draft_sleep_secs = defaults.tab_sleep_timeout_secs.to_string();
                    self.draft_ad_block = defaults.ad_block_enabled;
                    self.status_message = Some((
                        true,
                        "Defaults loaded. Click Save Settings to apply.".to_owned(),
                    ));
                }
            });

            if let Some((ok, msg)) = &self.status_message {
                ui.add_space(8.0);
                let color = if *ok {
                    theme::STATUS_SUCCESS
                } else {
                    theme::STATUS_DANGER
                };
                ui.label(egui::RichText::new(msg).color(color));
            }
        });

        action
    }
}

pub fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    if out.is_empty() {
        "New Tab".to_owned()
    } else {
        out
    }
}
