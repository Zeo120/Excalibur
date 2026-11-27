use excalibur_core::{BookmarkEntry, BrowserConfig, HistoryEntry, Storage};

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
    pub status_message: Option<String>,
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
            }
            None => {}
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, storage: &Storage) -> Option<DrawerAction> {
        let active = self.active?;
        let mut action = None;

        egui::Panel::right("excalibur_drawer")
            .exact_size(320.0)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let heading = match active {
                        ActiveDrawer::Bookmarks => "Bookmarks",
                        ActiveDrawer::History => "History",
                        ActiveDrawer::Settings => "Settings",
                    };
                    ui.strong(heading);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            self.active = None;
                        }
                    });
                });
                ui.separator();

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
            ui.label("No bookmarks yet. Press Ctrl+D on any page.");
            return None;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for bm in &self.bookmarks {
                ui.horizontal(|ui| {
                    let label = truncate_text(&bm.title, 28);
                    if ui
                        .selectable_label(false, label)
                        .on_hover_text(&bm.url)
                        .clicked()
                    {
                        action = Some(DrawerAction::NavigateActive(bm.url.clone()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("x").on_hover_text("Remove").clicked() {
                            deleted_url = Some(bm.url.clone());
                        }
                        if ui
                            .small_button("+")
                            .on_hover_text("Open in new tab")
                            .clicked()
                        {
                            action = Some(DrawerAction::OpenNewTab(bm.url.clone()));
                        }
                    });
                });
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
                    .desired_width(210.0)
                    .hint_text("Search history"),
            );
            if resp.changed() {
                self.history = storage
                    .search_history(&self.history_query, 100)
                    .unwrap_or_default();
            }
            if ui.button("Clear").clicked() {
                let _ = storage.clear_history();
                self.history.clear();
            }
        });
        ui.add_space(4.0);

        if self.history.is_empty() {
            ui.label("No history entries.");
            return None;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for item in &self.history {
                ui.horizontal(|ui| {
                    let label = truncate_text(&item.title, 30);
                    if ui
                        .selectable_label(false, label)
                        .on_hover_text(&item.url)
                        .clicked()
                    {
                        action = Some(DrawerAction::NavigateActive(item.url.clone()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("+")
                            .on_hover_text("Open in new tab")
                            .clicked()
                        {
                            action = Some(DrawerAction::OpenNewTab(item.url.clone()));
                        }
                    });
                });
            }
        });

        action
    }

    fn show_settings(&mut self, ui: &mut egui::Ui) -> Option<DrawerAction> {
        let mut action = None;

        ui.label("Home URL");
        ui.text_edit_singleline(&mut self.draft_home_url);
        ui.add_space(6.0);

        ui.label("Search template (must contain {query})");
        ui.text_edit_singleline(&mut self.draft_search_template);
        ui.add_space(6.0);

        ui.label("Max renderer processes (1-32, applies on restart)");
        ui.text_edit_singleline(&mut self.draft_renderer_limit);
        ui.add_space(6.0);

        ui.label("Idle tab sleep timeout (seconds, min 30)");
        ui.text_edit_singleline(&mut self.draft_sleep_secs);
        ui.add_space(10.0);

        if ui.button("Save Settings").clicked() {
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
                    };
                    self.status_message = Some("Settings saved.".to_owned());
                    action = Some(DrawerAction::SaveSettings(cfg));
                }
                (true, _, _, _) => {
                    self.status_message = Some("Home URL cannot be empty.".to_owned());
                }
                (_, false, _, _) => {
                    self.status_message = Some("Search template must contain {query}.".to_owned());
                }
                _ => {
                    self.status_message = Some("Invalid numeric value.".to_owned());
                }
            }
        }

        if let Some(msg) = &self.status_message {
            ui.add_space(6.0);
            ui.label(msg);
        }

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
