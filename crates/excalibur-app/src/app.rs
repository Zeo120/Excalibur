use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use excalibur_cef::{BrowserShortcut, CefEvent, CefRuntime};
use excalibur_core::{
    normalize_url_or_search, sample_process_tree_memory, BrowserConfig, MemorySnapshot, Storage,
    TabInfo, TabManager,
};
use raw_window_handle::HasWindowHandle;

use crate::{
    drawer::{truncate_text, ActiveDrawer, DrawerAction, DrawerState},
    icons::{icon_button, icon_button_sized, paint_icon, IconKind},
    theme,
};

#[derive(Default)]
struct TabActions {
    switch: Option<u64>,
    close: Option<u64>,
    pin: Option<u64>,
    sleep: Option<u64>,
    wake: Option<u64>,
    duplicate_url: Option<String>,
    open_new: bool,
}

#[derive(Default)]
struct ChromeActions {
    omnibox_focused: bool,
    submit_navigation: bool,
    toggle_bookmark: bool,
    toggle_blocker: bool,
    trigger_sleep_idle: bool,
}

struct ActiveUiSnapshot {
    active_id: Option<u64>,
    can_back: bool,
    can_forward: bool,
    is_loading: bool,
    is_sleeping: bool,
    current_url: String,
    is_bookmarked: bool,
    tab_blocked: u64,
    idle_sleepable_count: usize,
    sleeping_count: usize,
}

pub struct ExcaliburBrowserApp {
    config: BrowserConfig,
    storage: Storage,
    runtime: Rc<CefRuntime>,
    tabs: TabManager,
    drawer: DrawerState,
    omnibox_id: egui::Id,
    omnibox_text: String,
    focus_omnibox_next_frame: bool,
    compact_mode: bool,
    last_bounds: Option<(i32, i32, i32, i32)>,
    last_window_title: String,
    memory: MemorySnapshot,
    last_housekeeping: Instant,
}

impl ExcaliburBrowserApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: BrowserConfig,
        storage: Storage,
        runtime: Rc<CefRuntime>,
        initial_url: String,
    ) -> Self {
        let ctx = cc.egui_ctx.clone();
        runtime.set_wake_callback(Arc::new(move |delay_ms| {
            if delay_ms <= 0 {
                ctx.request_repaint();
            } else {
                let capped = u64::try_from(delay_ms).unwrap_or(16).min(33);
                ctx.request_repaint_after(Duration::from_millis(capped));
            }
        }));

        theme::apply_theme(&cc.egui_ctx);

        let mut tabs = TabManager::new();
        tabs.open_tab(initial_url.clone());
        let drawer = DrawerState::new(&config);
        let memory = sample_process_tree_memory();

        Self {
            config,
            storage,
            runtime,
            tabs,
            drawer,
            omnibox_id: egui::Id::unique("excalibur_omnibox"),
            omnibox_text: initial_url,
            focus_omnibox_next_frame: false,
            compact_mode: false,
            last_bounds: None,
            last_window_title: "Excalibur".to_owned(),
            memory,
            last_housekeeping: Instant::now(),
        }
    }

    fn active_tab_id(&self) -> Option<u64> {
        self.tabs.active_tab_id()
    }

    fn open_new_tab(&mut self, url: String, focus_omnibox: bool) {
        let prev_id = self.active_tab_id();
        let new_id = self.tabs.open_tab(url.clone());
        if let Some(prev) = prev_id {
            if prev != new_id {
                self.runtime.set_browser_visible(prev, false);
            }
        }
        self.omnibox_text = url;
        self.last_bounds = None;
        self.focus_omnibox_next_frame = focus_omnibox;
    }

    fn switch_to_tab(&mut self, ctx: &egui::Context, target_id: u64) {
        let prev_id = self.active_tab_id();
        if prev_id == Some(target_id) {
            return;
        }
        if !self.tabs.select_tab(target_id) {
            return;
        }
        if let Some(prev) = prev_id {
            self.runtime.set_browser_visible(prev, false);
        }
        if self.runtime.has_browser(target_id) {
            self.runtime.set_browser_visible(target_id, true);
        }
        self.last_bounds = None;
        self.sync_active_tab_ui(ctx, target_id);
    }

    fn sleep_tab(&mut self, tab_id: u64) {
        self.runtime.close_browser(tab_id);
        self.tabs.mark_sleeping(tab_id);
        self.last_bounds = None;
    }

    fn sleep_all_idle(&mut self) {
        for id in self.tabs.idle_sleepable_ids() {
            self.sleep_tab(id);
        }
        self.memory = sample_process_tree_memory();
    }

    fn wake_tab(&mut self, tab_id: u64) {
        if self.tabs.wake_tab(tab_id) {
            self.last_bounds = None;
        }
    }

    fn close_tab(&mut self, ctx: &egui::Context, target_id: u64) {
        let was_active = self.active_tab_id() == Some(target_id);
        if !self.tabs.close_tab(target_id) {
            return;
        }
        self.runtime.close_browser(target_id);

        if let Some(new_active) = self.active_tab_id() {
            if was_active {
                if self.runtime.has_browser(new_active) {
                    self.runtime.set_browser_visible(new_active, true);
                }
                self.last_bounds = None;
                self.sync_active_tab_ui(ctx, new_active);
            }
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn sync_active_tab_ui(&mut self, ctx: &egui::Context, tab_id: u64) {
        if let Some((url, title)) = self
            .tabs
            .tab(tab_id)
            .map(|t| (t.url.clone(), t.title.clone()))
        {
            self.omnibox_text = url;
            self.update_window_title(ctx, &title);
        }
    }

    fn update_window_title(&mut self, ctx: &egui::Context, title: &str) {
        let full = if title.trim().is_empty() {
            "Excalibur".to_owned()
        } else {
            format!("{title} - Excalibur")
        };
        if full != self.last_window_title {
            self.last_window_title = full.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(full));
        }
    }

    fn run_housekeeping(&mut self) {
        if self.last_housekeeping.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.last_housekeeping = Instant::now();
        self.memory = sample_process_tree_memory();

        let timeout = Duration::from_secs(self.config.tab_sleep_timeout_secs);
        for id in self.tabs.tabs_eligible_to_sleep(timeout) {
            self.sleep_tab(id);
        }
    }

    fn process_cef_events(&mut self, ctx: &egui::Context, omnibox_focused: bool) {
        let active_id = self.active_tab_id();
        for event in self.runtime.poll_events() {
            match event {
                CefEvent::ContextInitialized => {
                    ctx.request_repaint();
                }
                CefEvent::BrowserCreated { tab_id } => {
                    if let Some(tab) = self.tabs.tab_mut(tab_id) {
                        tab.browser_created = true;
                    }
                    if Some(tab_id) == active_id {
                        self.last_bounds = None;
                    } else {
                        self.runtime.set_browser_visible(tab_id, false);
                    }
                    ctx.request_repaint();
                }
                CefEvent::TitleChanged { tab_id, title } => {
                    if let Some(tab) = self.tabs.tab_mut(tab_id) {
                        tab.title = title.clone();
                    }
                    if Some(tab_id) == active_id {
                        self.update_window_title(ctx, &title);
                    }
                }
                CefEvent::AddressChanged { tab_id, url } => {
                    if let Some(tab) = self.tabs.tab_mut(tab_id) {
                        tab.url = url.clone();
                    }
                    if Some(tab_id) == active_id && !omnibox_focused {
                        self.omnibox_text = url;
                    }
                }
                CefEvent::LoadingStateChanged {
                    tab_id,
                    is_loading,
                    can_go_back,
                    can_go_forward,
                } => {
                    let finished_loading = self
                        .tabs
                        .tab(tab_id)
                        .is_some_and(|t| t.is_loading && !is_loading);

                    if let Some(tab) = self.tabs.tab_mut(tab_id) {
                        tab.is_loading = is_loading;
                        tab.can_go_back = can_go_back;
                        tab.can_go_forward = can_go_forward;
                    }

                    if finished_loading {
                        if let Some(tab) = self.tabs.tab(tab_id) {
                            let _ = self.storage.record_history(&tab.url, &tab.title);
                            if self.drawer.active == Some(ActiveDrawer::History) {
                                self.drawer.refresh(&self.storage, &self.config);
                            }
                        }
                    }
                }
                CefEvent::RequestBlocked { tab_id, .. } => {
                    self.tabs.increment_blocked(tab_id);
                }
                CefEvent::OpenPopup { url } => {
                    self.open_new_tab(url, false);
                }
                CefEvent::Shortcut(shortcut) => {
                    self.execute_shortcut(ctx, shortcut);
                }
                CefEvent::BrowserClosed { tab_id } => {
                    self.runtime.prune_closed(tab_id);
                }
            }
        }
    }

    fn navigate_omnibox(&mut self) {
        let Some(active_id) = self.active_tab_id() else {
            return;
        };
        let target = normalize_url_or_search(&self.omnibox_text, &self.config.search_template);
        self.omnibox_text = target.clone();
        if let Some(tab) = self.tabs.tab_mut(active_id) {
            tab.url = target.clone();
            tab.is_sleeping = false;
        }
        self.runtime.navigate(active_id, &target);
    }

    fn toggle_bookmark_active(&mut self) {
        let Some(tab) = self.tabs.active_tab() else {
            return;
        };
        let url = tab.url.clone();
        let title = tab.title.clone();
        if self.storage.is_bookmarked(&url).unwrap_or(false) {
            let _ = self.storage.remove_bookmark_by_url(&url);
        } else {
            let _ = self.storage.add_bookmark(&url, &title);
        }
        if self.drawer.active == Some(ActiveDrawer::Bookmarks) {
            self.drawer.refresh(&self.storage, &self.config);
        }
    }

    fn toggle_ad_block(&mut self) {
        let next = !self.config.ad_block_enabled;
        self.config.ad_block_enabled = next;
        self.runtime.blocker().set_enabled(next);
        let _ = self.storage.save_settings(&self.config);
        if self.drawer.active == Some(ActiveDrawer::Settings) {
            self.drawer.refresh(&self.storage, &self.config);
        }
    }

    fn execute_shortcut(&mut self, ctx: &egui::Context, shortcut: BrowserShortcut) {
        match shortcut {
            BrowserShortcut::NewTab => {
                self.open_new_tab(self.config.home_url.clone(), true);
            }
            BrowserShortcut::CloseTab => {
                if let Some(active) = self.tabs.active_tab() {
                    if !active.pinned {
                        let id = active.id;
                        self.close_tab(ctx, id);
                    }
                }
            }
            BrowserShortcut::NextTab => {
                if let Some(next_id) = self.tabs.cycle_tab(true) {
                    self.sync_after_cycle(ctx, next_id);
                }
            }
            BrowserShortcut::PrevTab => {
                if let Some(prev_id) = self.tabs.cycle_tab(false) {
                    self.sync_after_cycle(ctx, prev_id);
                }
            }
            BrowserShortcut::FocusOmnibox => {
                self.focus_omnibox_next_frame = true;
            }
            BrowserShortcut::ToggleSidebar => {
                self.compact_mode = !self.compact_mode;
                self.last_bounds = None;
            }
            BrowserShortcut::ToggleBookmark => {
                self.toggle_bookmark_active();
            }
            BrowserShortcut::ToggleBookmarksDrawer => {
                self.drawer
                    .toggle(ActiveDrawer::Bookmarks, &self.storage, &self.config);
            }
            BrowserShortcut::ToggleHistoryDrawer => {
                self.drawer
                    .toggle(ActiveDrawer::History, &self.storage, &self.config);
            }
            BrowserShortcut::Reload => {
                if let Some(active_id) = self.active_tab_id() {
                    if self.tabs.tab(active_id).is_some_and(|t| t.is_sleeping) {
                        self.wake_tab(active_id);
                    } else {
                        self.runtime.reload(active_id);
                    }
                }
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (
            ctrl_t,
            ctrl_w,
            ctrl_l,
            ctrl_backslash,
            alt_s,
            ctrl_d,
            ctrl_b,
            ctrl_h,
            reload_key,
            tab_fwd,
            tab_bwd,
        ) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;
            let alt_only = i.modifiers.alt && !cmd;
            (
                cmd && i.key_pressed(egui::Key::T),
                cmd && i.key_pressed(egui::Key::W),
                cmd && i.key_pressed(egui::Key::L),
                cmd && i.key_pressed(egui::Key::Backslash),
                alt_only && i.key_pressed(egui::Key::S),
                cmd && i.key_pressed(egui::Key::D),
                cmd && i.key_pressed(egui::Key::B),
                cmd && i.key_pressed(egui::Key::H),
                i.key_pressed(egui::Key::F5) || (cmd && i.key_pressed(egui::Key::R)),
                cmd && !shift && i.key_pressed(egui::Key::Tab),
                cmd && shift && i.key_pressed(egui::Key::Tab),
            )
        });

        if ctrl_t {
            self.execute_shortcut(ctx, BrowserShortcut::NewTab);
        }
        if ctrl_w {
            self.execute_shortcut(ctx, BrowserShortcut::CloseTab);
        }
        if tab_fwd {
            self.execute_shortcut(ctx, BrowserShortcut::NextTab);
        } else if tab_bwd {
            self.execute_shortcut(ctx, BrowserShortcut::PrevTab);
        }
        if ctrl_l {
            self.execute_shortcut(ctx, BrowserShortcut::FocusOmnibox);
        }
        if ctrl_backslash || alt_s {
            self.execute_shortcut(ctx, BrowserShortcut::ToggleSidebar);
        }
        if ctrl_d {
            self.execute_shortcut(ctx, BrowserShortcut::ToggleBookmark);
        }
        if ctrl_b {
            self.execute_shortcut(ctx, BrowserShortcut::ToggleBookmarksDrawer);
        }
        if ctrl_h {
            self.execute_shortcut(ctx, BrowserShortcut::ToggleHistoryDrawer);
        }
        if reload_key {
            self.execute_shortcut(ctx, BrowserShortcut::Reload);
        }
    }

    fn sync_after_cycle(&mut self, ctx: &egui::Context, new_active_id: u64) {
        let all_ids: Vec<u64> = self.tabs.tabs().iter().map(|t| t.id).collect();
        for id in all_ids {
            if id != new_active_id {
                self.runtime.set_browser_visible(id, false);
            }
        }
        if self.runtime.has_browser(new_active_id) {
            self.runtime.set_browser_visible(new_active_id, true);
        }
        self.last_bounds = None;
        self.sync_active_tab_ui(ctx, new_active_id);
    }

    fn apply_tab_actions(&mut self, ctx: &egui::Context, actions: TabActions) {
        if let Some(id) = actions.switch {
            self.switch_to_tab(ctx, id);
        }
        if let Some(id) = actions.pin {
            self.tabs.toggle_pin(id);
        }
        if let Some(id) = actions.sleep {
            self.sleep_tab(id);
        }
        if let Some(id) = actions.wake {
            self.wake_tab(id);
        }
        if let Some(url) = actions.duplicate_url {
            self.open_new_tab(url, false);
        }
        if let Some(id) = actions.close {
            self.close_tab(ctx, id);
        }
        if actions.open_new {
            self.open_new_tab(self.config.home_url.clone(), true);
        }
    }

    fn render_nav_controls(&mut self, ui: &mut egui::Ui, snap: &ActiveUiSnapshot) {
        let sidebar_tip = if self.compact_mode {
            "Show sidebar (Ctrl+\\ or Alt+S)"
        } else {
            "Switch to compact top bar (Ctrl+\\ or Alt+S)"
        };
        if icon_button(ui, IconKind::Sidebar, true, !self.compact_mode, None)
            .on_hover_text(sidebar_tip)
            .clicked()
        {
            self.compact_mode = !self.compact_mode;
            self.last_bounds = None;
        }

        if icon_button(ui, IconKind::Back, snap.can_back, false, None)
            .on_hover_text("Back")
            .clicked()
        {
            if let Some(id) = snap.active_id {
                self.runtime.go_back(id);
            }
        }
        if icon_button(ui, IconKind::Forward, snap.can_forward, false, None)
            .on_hover_text("Forward")
            .clicked()
        {
            if let Some(id) = snap.active_id {
                self.runtime.go_forward(id);
            }
        }
        let (reload_icon, reload_tip) = if snap.is_loading {
            (IconKind::Stop, "Stop loading")
        } else {
            (IconKind::Reload, "Reload (F5)")
        };
        if icon_button(ui, reload_icon, snap.active_id.is_some(), false, None)
            .on_hover_text(reload_tip)
            .clicked()
        {
            if let Some(id) = snap.active_id {
                if snap.is_sleeping {
                    self.wake_tab(id);
                } else if snap.is_loading {
                    self.runtime.stop_load(id);
                } else {
                    self.runtime.reload(id);
                }
            }
        }
    }

    fn render_url_pill(
        &mut self,
        ui: &mut egui::Ui,
        pill_width: f32,
        snap: &ActiveUiSnapshot,
        actions: &mut ChromeActions,
    ) {
        let has_focus = ui.ctx().memory(|m| m.has_focus(self.omnibox_id));
        let stroke = if has_focus {
            egui::Stroke::new(1.0, theme::ACCENT)
        } else {
            egui::Stroke::new(1.0, theme::BORDER_HAIRLINE)
        };
        let pill_frame = egui::Frame::NONE
            .fill(theme::BG_PILL)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(8, 4))
            .stroke(stroke);

        let trailing_width = if snap.tab_blocked > 0 { 66.0 } else { 46.0 };
        let text_width = (pill_width - 16.0 - 18.0 - trailing_width).max(60.0);

        pill_frame.show(ui, |ui| {
            ui.set_width((pill_width - 16.0).max(110.0));
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                let (lock_rect, _) =
                    ui.allocate_exact_size(egui::vec2(13.0, 13.0), egui::Sense::hover());
                let lock_color = if snap.current_url.starts_with("https://") {
                    theme::STATUS_SUCCESS
                } else {
                    theme::TEXT_MUTED
                };
                paint_icon(ui.painter(), lock_rect, IconKind::Lock, lock_color);

                let mut output = egui::TextEdit::singleline(&mut self.omnibox_text)
                    .id(self.omnibox_id)
                    .desired_width(text_width)
                    .frame(egui::Frame::NONE)
                    .hint_text("Search or enter URL (Ctrl+L)")
                    .show(ui);

                if self.focus_omnibox_next_frame || output.response.gained_focus() {
                    output.response.request_focus();
                    let char_len = self.omnibox_text.chars().count();
                    output
                        .state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(char_len),
                        )));
                    output.state.store(ui.ctx(), output.response.id);
                    self.focus_omnibox_next_frame = false;
                }

                actions.omnibox_focused = output.response.has_focus();
                if output.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    actions.submit_navigation = true;
                }

                let shield_tint = if self.config.ad_block_enabled {
                    theme::STATUS_SUCCESS
                } else {
                    theme::TEXT_MUTED
                };
                if icon_button_sized(
                    ui,
                    IconKind::Shield,
                    20.0,
                    true,
                    self.config.ad_block_enabled,
                    Some(shield_tint),
                )
                .on_hover_text(format!(
                    "Tracker blocker: {} ({} blocked on tab, {} total)",
                    if self.config.ad_block_enabled {
                        "ON"
                    } else {
                        "OFF"
                    },
                    snap.tab_blocked,
                    self.tabs.total_blocked_count()
                ))
                .clicked()
                {
                    actions.toggle_blocker = true;
                }

                if snap.tab_blocked > 0 {
                    ui.label(
                        egui::RichText::new(format!("{}", snap.tab_blocked))
                            .monospace()
                            .size(10.5)
                            .color(theme::STATUS_SUCCESS),
                    );
                }

                let (star_icon, star_tint) = if snap.is_bookmarked {
                    (IconKind::StarFilled, Some(theme::STATUS_WARNING))
                } else {
                    (IconKind::StarOutline, None)
                };
                if icon_button_sized(
                    ui,
                    star_icon,
                    20.0,
                    snap.active_id.is_some(),
                    false,
                    star_tint,
                )
                .on_hover_text("Bookmark (Ctrl+D)")
                .clicked()
                {
                    actions.toggle_bookmark = true;
                }
            });
        });
    }

    fn render_utility_controls(
        &mut self,
        ui: &mut egui::Ui,
        snap: &ActiveUiSnapshot,
        actions: &mut ChromeActions,
    ) {
        let ram_color = if self.memory.total_mb >= 700.0 {
            theme::STATUS_DANGER
        } else if self.memory.total_mb >= 350.0 {
            theme::STATUS_WARNING
        } else {
            theme::STATUS_SUCCESS
        };

        let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 14.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(dot_rect.center(), 3.0, ram_color);

        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("{:.0} MB", self.memory.total_mb))
                    .monospace()
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY),
            )
            .selectable(false),
        )
        .on_hover_text(format!(
            "Host: {:.1} MB\nChromium ({} procs): {:.1} MB\nSleeping tabs: {}",
            self.memory.host_mb,
            self.memory.child_process_count,
            self.memory.chromium_mb,
            snap.sleeping_count
        ));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;

            if icon_button(
                ui,
                IconKind::Settings,
                true,
                self.drawer.active == Some(ActiveDrawer::Settings),
                None,
            )
            .on_hover_text("Settings")
            .clicked()
            {
                self.drawer
                    .toggle(ActiveDrawer::Settings, &self.storage, &self.config);
            }
            if icon_button(
                ui,
                IconKind::History,
                true,
                self.drawer.active == Some(ActiveDrawer::History),
                None,
            )
            .on_hover_text("History (Ctrl+H)")
            .clicked()
            {
                self.drawer
                    .toggle(ActiveDrawer::History, &self.storage, &self.config);
            }
            if icon_button(
                ui,
                IconKind::Bookmarks,
                true,
                self.drawer.active == Some(ActiveDrawer::Bookmarks),
                None,
            )
            .on_hover_text("Bookmarks (Ctrl+B)")
            .clicked()
            {
                self.drawer
                    .toggle(ActiveDrawer::Bookmarks, &self.storage, &self.config);
            }
            if icon_button(
                ui,
                IconKind::Sleep,
                snap.idle_sleepable_count > 0,
                false,
                (snap.idle_sleepable_count > 0).then_some(theme::STATUS_WARNING),
            )
            .on_hover_text(format!("Sleep {} idle tabs", snap.idle_sleepable_count))
            .clicked()
            {
                actions.trigger_sleep_idle = true;
            }
        });
    }

    fn show_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        snap: &ActiveUiSnapshot,
        chrome: &mut ChromeActions,
    ) {
        let mut tab_actions = TabActions::default();
        let sidebar_frame = egui::Frame::NONE
            .fill(theme::BG_SHELL)
            .inner_margin(egui::Margin::symmetric(10, 10));

        egui::Panel::left("excalibur_sidebar")
            .exact_size(240.0)
            .frame(sidebar_frame)
            .show(ui, |ui| {
                let inner_w = ui.available_width();

                // Navigation controls row
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    self.render_nav_controls(ui, snap);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, IconKind::Plus, true, false, None)
                            .on_hover_text("New tab (Ctrl+T)")
                            .clicked()
                        {
                            tab_actions.open_new = true;
                        }
                    });
                });

                ui.add_space(8.0);

                // Integrated Zen URL pill
                self.render_url_pill(ui, inner_w, snap, chrome);

                ui.add_space(8.0);
                paint_hairline_rule(ui, inner_w);
                ui.add_space(6.0);

                // Pinned tabs compact tiles (if any)
                let pinned_tabs: Vec<TabInfo> = self
                    .tabs
                    .tabs()
                    .iter()
                    .filter(|t| t.pinned)
                    .cloned()
                    .collect();
                if !pinned_tabs.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
                        for tab in &pinned_tabs {
                            let is_active = Some(tab.id) == snap.active_id;
                            render_pinned_tile(ui, tab, is_active, &mut tab_actions);
                        }
                    });
                    ui.add_space(6.0);
                    paint_hairline_rule(ui, inner_w);
                    ui.add_space(6.0);
                }

                // Vertical tab list
                let footer_reserve = 36.0;
                let list_height = (ui.available_height() - footer_reserve).max(80.0);
                let unpinned_tabs: Vec<TabInfo> = self
                    .tabs
                    .tabs()
                    .iter()
                    .filter(|t| !t.pinned)
                    .cloned()
                    .collect();

                ui.allocate_ui(egui::vec2(inner_w, list_height), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("zen_vertical_tabs")
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 3.0;
                            for tab in &unpinned_tabs {
                                let is_active = Some(tab.id) == snap.active_id;
                                render_vertical_tab_row(ui, tab, is_active, &mut tab_actions);
                            }

                            ui.add_space(4.0);
                            if render_new_tab_row(ui) {
                                tab_actions.open_new = true;
                            }
                        });
                });

                // Quiet bottom utility footer
                paint_hairline_rule(ui, inner_w);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.set_height(26.0);
                    self.render_utility_controls(ui, snap, chrome);
                });
            });

        self.apply_tab_actions(ctx, tab_actions);
    }

    fn show_compact_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        snap: &ActiveUiSnapshot,
        chrome: &mut ChromeActions,
    ) {
        let mut tab_actions = TabActions::default();
        let top_frame = egui::Frame::NONE
            .fill(theme::BG_SHELL)
            .inner_margin(egui::Margin::symmetric(8, 6));

        egui::Panel::top("excalibur_compact_bar")
            .exact_size(42.0)
            .frame(top_frame)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    self.render_nav_controls(ui, snap);

                    ui.add_space(2.0);

                    let total_w = ui.available_width();
                    let pill_w = (total_w * 0.28).clamp(200.0, 320.0);
                    self.render_url_pill(ui, pill_w, snap, chrome);

                    ui.add_space(4.0);

                    let right_reserve = 200.0;
                    let tabs_width = (ui.available_width() - right_reserve).max(100.0);
                    let all_tabs = self.tabs.tabs().to_vec();

                    ui.allocate_ui(egui::vec2(tabs_width, 28.0), |ui| {
                        egui::ScrollArea::horizontal()
                            .id_salt("compact_tab_scroll")
                            .show(ui, |ui| {
                                ui.horizontal_centered(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    for tab in &all_tabs {
                                        let is_active = Some(tab.id) == snap.active_id;
                                        render_compact_tab_pill(
                                            ui,
                                            tab,
                                            is_active,
                                            &mut tab_actions,
                                        );
                                    }
                                    if icon_button(ui, IconKind::Plus, true, false, None)
                                        .on_hover_text("New tab (Ctrl+T)")
                                        .clicked()
                                    {
                                        tab_actions.open_new = true;
                                    }
                                });
                            });
                    });

                    self.render_utility_controls(ui, snap, chrome);
                });
            });

        self.apply_tab_actions(ctx, tab_actions);
    }
}

impl eframe::App for ExcaliburBrowserApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.runtime.pump_work();
        self.run_housekeeping();
        ctx.request_repaint_after(Duration::from_millis(33));
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        if ui.input(|i| i.pointer.any_pressed()) || self.focus_omnibox_next_frame {
            if let Ok(window_handle) = frame.window_handle() {
                self.runtime.focus_host_window(window_handle.as_raw());
            }
        }

        let (active_id, can_back, can_forward, is_loading, is_sleeping, current_url, tab_blocked) =
            self.tabs
                .active_tab()
                .map(|t| {
                    (
                        Some(t.id),
                        t.can_go_back,
                        t.can_go_forward,
                        t.is_loading,
                        t.is_sleeping,
                        t.url.clone(),
                        t.blocked_count,
                    )
                })
                .unwrap_or((None, false, false, false, false, String::new(), 0));

        let is_bookmarked =
            !current_url.is_empty() && self.storage.is_bookmarked(&current_url).unwrap_or(false);
        let snap = ActiveUiSnapshot {
            active_id,
            can_back,
            can_forward,
            is_loading,
            is_sleeping,
            current_url: current_url.clone(),
            is_bookmarked,
            tab_blocked,
            idle_sleepable_count: self.tabs.idle_sleepable_ids().len(),
            sleeping_count: self.tabs.tabs().iter().filter(|t| t.is_sleeping).count(),
        };

        let mut chrome = ChromeActions::default();

        if self.compact_mode {
            self.show_compact_top_bar(ui, &ctx, &snap, &mut chrome);
        } else {
            self.show_sidebar(ui, &ctx, &snap, &mut chrome);
        }

        if chrome.toggle_bookmark {
            self.toggle_bookmark_active();
        }
        if chrome.toggle_blocker {
            self.toggle_ad_block();
        }
        if chrome.trigger_sleep_idle {
            self.sleep_all_idle();
        }
        if chrome.submit_navigation {
            self.navigate_omnibox();
        }

        if let Some(action) = self.drawer.show(ui, &self.storage) {
            match action {
                DrawerAction::NavigateActive(url) => {
                    self.omnibox_text = url;
                    self.navigate_omnibox();
                }
                DrawerAction::OpenNewTab(url) => {
                    self.open_new_tab(url, false);
                }
                DrawerAction::SaveSettings(new_cfg) => {
                    self.runtime.blocker().set_enabled(new_cfg.ad_block_enabled);
                    if self.storage.save_settings(&new_cfg).is_ok() {
                        self.config = new_cfg;
                    }
                }
            }
        }

        self.process_cef_events(&ctx, chrome.omnibox_focused);

        let inset_margin = if self.compact_mode {
            egui::Margin {
                left: 6,
                right: 6,
                top: 2,
                bottom: 6,
            }
        } else {
            egui::Margin {
                left: 0,
                right: 6,
                top: 6,
                bottom: 6,
            }
        };

        egui::CentralPanel::no_frame()
            .frame(
                egui::Frame::NONE
                    .fill(theme::BG_SHELL)
                    .inner_margin(inset_margin),
            )
            .show(ui, |ui| {
                let outer_rect = ui.available_rect_before_wrap();
                ui.painter().rect_stroke(
                    outer_rect,
                    egui::CornerRadius::same(6),
                    egui::Stroke::new(1.0, theme::BORDER_HAIRLINE),
                    egui::StrokeKind::Inside,
                );

                let Some(tab_id) = self.active_tab_id() else {
                    return;
                };
                if is_sleeping {
                    let inner = outer_rect.shrink(1.0);
                    ui.painter()
                        .rect_filled(inner, egui::CornerRadius::same(6), theme::BG_FIELD);
                    ui.centered_and_justified(|ui| {
                        let card = egui::Frame::NONE
                            .fill(theme::BG_SURFACE)
                            .corner_radius(egui::CornerRadius::same(10))
                            .inner_margin(egui::Margin::same(24))
                            .stroke(egui::Stroke::new(1.0, theme::BORDER_HAIRLINE));
                        card.show(ui, |ui| {
                            ui.set_max_width(420.0);
                            ui.vertical_centered(|ui| {
                                let (moon_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(26.0, 26.0),
                                    egui::Sense::hover(),
                                );
                                paint_icon(
                                    ui.painter(),
                                    moon_rect,
                                    IconKind::Sleep,
                                    theme::STATUS_WARNING,
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Tab is asleep to save memory")
                                        .size(16.0)
                                        .strong()
                                        .color(theme::TEXT_PRIMARY),
                                );
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(truncate_text(&current_url, 64))
                                        .size(12.0)
                                        .color(theme::TEXT_SECONDARY),
                                );
                                ui.add_space(16.0);
                                let wake_btn = egui::Button::new(
                                    egui::RichText::new("Wake Tab")
                                        .color(theme::TEXT_PRIMARY)
                                        .strong(),
                                )
                                .fill(theme::ACCENT_SOFT)
                                .stroke(egui::Stroke::new(1.0, theme::ACCENT))
                                .min_size(egui::vec2(116.0, 32.0));
                                if ui.add(wake_btn).clicked() {
                                    self.wake_tab(tab_id);
                                }
                            });
                        });
                    });
                    return;
                }

                let rect = outer_rect.shrink(1.0);
                let ppp = ctx.pixels_per_point();
                let bounds = (
                    (rect.min.x * ppp).round() as i32,
                    (rect.min.y * ppp).round() as i32,
                    (rect.width() * ppp).round() as i32,
                    (rect.height() * ppp).round() as i32,
                );

                if !self.runtime.has_browser(tab_id) {
                    if self.runtime.is_context_ready() && bounds.2 > 0 && bounds.3 > 0 {
                        if let Ok(window_handle) = frame.window_handle() {
                            match self.runtime.create_child_browser(
                                tab_id,
                                window_handle.as_raw(),
                                bounds,
                                &current_url,
                            ) {
                                Ok(_) => {
                                    self.last_bounds = Some(bounds);
                                }
                                Err(err) => {
                                    tracing::error!("Failed to spawn CEF child browser: {err}");
                                }
                            }
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                egui::RichText::new("Initializing Chromium...")
                                    .color(theme::TEXT_SECONDARY),
                            );
                        });
                    }
                } else if self.last_bounds != Some(bounds) && bounds.2 > 0 && bounds.3 > 0 {
                    self.runtime
                        .resize_browser(tab_id, bounds.0, bounds.1, bounds.2, bounds.3);
                    self.last_bounds = Some(bounds);
                }
            });
    }
}

fn paint_hairline_rule(ui: &mut egui::Ui, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.min.x, rect.center().y),
            egui::pos2(rect.max.x, rect.center().y),
        ],
        egui::Stroke::new(1.0, theme::BORDER_HAIRLINE),
    );
}

fn render_pinned_tile(ui: &mut egui::Ui, tab: &TabInfo, is_active: bool, actions: &mut TabActions) {
    let size = egui::vec2(68.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let bg = if is_active {
        theme::BG_PILL
    } else if resp.hovered() {
        theme::BG_HOVER
    } else {
        theme::BG_SURFACE
    };
    let border = if is_active {
        egui::Stroke::new(1.0, theme::BORDER_STRONG)
    } else {
        egui::Stroke::NONE
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(7),
        bg,
        border,
        egui::StrokeKind::Inside,
    );

    let pin_rect = egui::Rect::from_center_size(
        egui::pos2(rect.min.x + 12.0, rect.center().y),
        egui::vec2(10.0, 10.0),
    );
    let badge_color = if is_active {
        theme::ACCENT
    } else if tab.is_sleeping {
        theme::STATUS_WARNING
    } else {
        theme::TEXT_SECONDARY
    };
    paint_icon(ui.painter(), pin_rect, IconKind::Pin, badge_color);

    let short = truncate_text(&domain_label(&tab.url, &tab.title), 6);
    let text_color = if is_active {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_SECONDARY
    };
    ui.painter().text(
        egui::pos2(rect.min.x + 22.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        short,
        egui::FontId::proportional(11.5),
        text_color,
    );

    if resp.clicked() {
        actions.switch = Some(tab.id);
    }
    resp.context_menu(|ui| render_tab_context_menu(ui, tab, actions));
    let _ = resp.on_hover_text(&tab.url);
}

fn render_vertical_tab_row(
    ui: &mut egui::Ui,
    tab: &TabInfo,
    is_active: bool,
    actions: &mut TabActions,
) {
    let width = ui.available_width();
    let (row_rect, _) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::hover());
    let row_hovered = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| row_rect.contains(p));
    let show_close = (row_hovered || is_active) && !tab.pinned;

    let close_size = 20.0;
    let close_rect = egui::Rect::from_center_size(
        egui::pos2(row_rect.max.x - 14.0, row_rect.center().y),
        egui::vec2(close_size, close_size),
    );
    let main_rect = if show_close {
        egui::Rect::from_min_max(
            row_rect.min,
            egui::pos2(close_rect.min.x - 2.0, row_rect.max.y),
        )
    } else {
        row_rect
    };

    let main_resp = ui.interact(
        main_rect,
        ui.make_persistent_id(("vtab_main", tab.id)),
        egui::Sense::click(),
    );

    let bg = if is_active {
        theme::BG_PILL
    } else if row_hovered {
        theme::BG_HOVER
    } else {
        egui::Color32::TRANSPARENT
    };
    let border = if is_active {
        egui::Stroke::new(1.0, theme::BORDER_STRONG)
    } else {
        egui::Stroke::NONE
    };
    ui.painter().rect(
        row_rect,
        egui::CornerRadius::same(7),
        bg,
        border,
        egui::StrokeKind::Inside,
    );

    // Leading status badge or domain monogram
    let icon_center = egui::pos2(row_rect.min.x + 15.0, row_rect.center().y);
    if tab.is_loading {
        ui.painter().circle_filled(icon_center, 3.5, theme::ACCENT);
    } else if tab.is_sleeping {
        paint_icon(
            ui.painter(),
            egui::Rect::from_center_size(icon_center, egui::vec2(12.0, 12.0)),
            IconKind::Sleep,
            theme::STATUS_WARNING,
        );
    } else {
        let chip_rect = egui::Rect::from_center_size(icon_center, egui::vec2(18.0, 18.0));
        let chip_fill = if is_active {
            theme::ACCENT_SOFT
        } else {
            theme::BG_SURFACE
        };
        let chip_text = if is_active {
            theme::ACCENT
        } else {
            theme::TEXT_SECONDARY
        };
        ui.painter()
            .rect_filled(chip_rect, egui::CornerRadius::same(5), chip_fill);
        let ch = domain_monogram(&tab.url, &tab.title);
        ui.painter().text(
            chip_rect.center(),
            egui::Align2::CENTER_CENTER,
            ch,
            egui::FontId::proportional(10.5),
            chip_text,
        );
    }

    // Tab title
    let title_color = if tab.is_sleeping {
        theme::TEXT_MUTED
    } else if is_active {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_SECONDARY
    };
    let title = truncate_text(&tab.title, if show_close { 20 } else { 23 });
    ui.painter().text(
        egui::pos2(row_rect.min.x + 30.0, row_rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.5),
        title_color,
    );

    if show_close {
        let close_resp = ui.interact(
            close_rect,
            ui.make_persistent_id(("vtab_close", tab.id)),
            egui::Sense::click(),
        );
        if close_resp.hovered() {
            ui.painter()
                .rect_filled(close_rect, egui::CornerRadius::same(5), theme::BG_SURFACE);
        }
        paint_icon(
            ui.painter(),
            close_rect.shrink(5.0),
            IconKind::Close,
            if close_resp.hovered() {
                theme::TEXT_PRIMARY
            } else {
                theme::TEXT_MUTED
            },
        );
        if close_resp.clicked() {
            actions.close = Some(tab.id);
        }
        let _ = close_resp.on_hover_text("Close tab (Ctrl+W)");
    }

    if main_resp.clicked() {
        actions.switch = Some(tab.id);
    }
    if main_resp.middle_clicked() && !tab.pinned {
        actions.close = Some(tab.id);
    }
    main_resp.context_menu(|ui| render_tab_context_menu(ui, tab, actions));
    let _ = main_resp.on_hover_text(format!(
        "{}{}",
        tab.url,
        if tab.is_sleeping { " (Sleeping)" } else { "" }
    ));
}

fn render_new_tab_row(ui: &mut egui::Ui) -> bool {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 28.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(7), theme::BG_HOVER);
    }
    let plus_rect = egui::Rect::from_center_size(
        egui::pos2(rect.min.x + 15.0, rect.center().y),
        egui::vec2(11.0, 11.0),
    );
    let color = if resp.hovered() {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_MUTED
    };
    paint_icon(ui.painter(), plus_rect, IconKind::Plus, color);
    ui.painter().text(
        egui::pos2(rect.min.x + 30.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "New Tab",
        egui::FontId::proportional(12.5),
        color,
    );
    let clicked = resp.clicked();
    let _ = resp.on_hover_text("New tab (Ctrl+T)");
    clicked
}

fn render_compact_tab_pill(
    ui: &mut egui::Ui,
    tab: &TabInfo,
    is_active: bool,
    actions: &mut TabActions,
) {
    let label = truncate_text(&tab.title, if tab.pinned { 10 } else { 18 });
    let pill_w = if tab.pinned { 96.0 } else { 150.0 };
    let (pill_rect, _) = ui.allocate_exact_size(egui::vec2(pill_w, 26.0), egui::Sense::hover());
    let hovered = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| pill_rect.contains(p));
    let show_close = (hovered || is_active) && !tab.pinned;

    let close_rect = egui::Rect::from_center_size(
        egui::pos2(pill_rect.max.x - 13.0, pill_rect.center().y),
        egui::vec2(18.0, 18.0),
    );
    let main_rect = if show_close {
        egui::Rect::from_min_max(
            pill_rect.min,
            egui::pos2(close_rect.min.x - 2.0, pill_rect.max.y),
        )
    } else {
        pill_rect
    };

    let main_resp = ui.interact(
        main_rect,
        ui.make_persistent_id(("ctab_main", tab.id)),
        egui::Sense::click(),
    );

    let bg = if is_active {
        theme::BG_PILL
    } else if hovered {
        theme::BG_HOVER
    } else {
        egui::Color32::TRANSPARENT
    };
    let border = if is_active {
        egui::Stroke::new(1.0, theme::BORDER_STRONG)
    } else {
        egui::Stroke::NONE
    };
    ui.painter().rect(
        pill_rect,
        egui::CornerRadius::same(7),
        bg,
        border,
        egui::StrokeKind::Inside,
    );

    let icon_center = egui::pos2(pill_rect.min.x + 12.0, pill_rect.center().y);
    if tab.pinned {
        paint_icon(
            ui.painter(),
            egui::Rect::from_center_size(icon_center, egui::vec2(9.0, 9.0)),
            IconKind::Pin,
            if is_active {
                theme::ACCENT
            } else {
                theme::TEXT_SECONDARY
            },
        );
    } else if tab.is_loading {
        ui.painter().circle_filled(icon_center, 3.0, theme::ACCENT);
    } else if tab.is_sleeping {
        paint_icon(
            ui.painter(),
            egui::Rect::from_center_size(icon_center, egui::vec2(10.0, 10.0)),
            IconKind::Sleep,
            theme::STATUS_WARNING,
        );
    } else {
        ui.painter().text(
            icon_center,
            egui::Align2::CENTER_CENTER,
            domain_monogram(&tab.url, &tab.title),
            egui::FontId::proportional(10.5),
            if is_active {
                theme::ACCENT
            } else {
                theme::TEXT_MUTED
            },
        );
    }

    let text_color = if tab.is_sleeping {
        theme::TEXT_MUTED
    } else if is_active {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_SECONDARY
    };
    ui.painter().text(
        egui::pos2(pill_rect.min.x + 22.0, pill_rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        text_color,
    );

    if show_close {
        let close_resp = ui.interact(
            close_rect,
            ui.make_persistent_id(("ctab_close", tab.id)),
            egui::Sense::click(),
        );
        if close_resp.hovered() {
            ui.painter()
                .rect_filled(close_rect, egui::CornerRadius::same(4), theme::BG_SURFACE);
        }
        paint_icon(
            ui.painter(),
            close_rect.shrink(4.5),
            IconKind::Close,
            if close_resp.hovered() {
                theme::TEXT_PRIMARY
            } else {
                theme::TEXT_MUTED
            },
        );
        if close_resp.clicked() {
            actions.close = Some(tab.id);
        }
    }

    if main_resp.clicked() {
        actions.switch = Some(tab.id);
    }
    if main_resp.middle_clicked() && !tab.pinned {
        actions.close = Some(tab.id);
    }
    main_resp.context_menu(|ui| render_tab_context_menu(ui, tab, actions));
    let _ = main_resp.on_hover_text(&tab.url);
}

fn render_tab_context_menu(ui: &mut egui::Ui, tab: &TabInfo, actions: &mut TabActions) {
    let pin_label = if tab.pinned { "Unpin Tab" } else { "Pin Tab" };
    if ui.button(pin_label).clicked() {
        actions.pin = Some(tab.id);
        ui.close();
    }
    if tab.is_sleeping {
        if ui.button("Wake Tab").clicked() {
            actions.wake = Some(tab.id);
            ui.close();
        }
    } else if !tab.pinned && ui.button("Sleep Tab").clicked() {
        actions.sleep = Some(tab.id);
        ui.close();
    }
    if ui.button("Duplicate Tab").clicked() {
        actions.duplicate_url = Some(tab.url.clone());
        ui.close();
    }
    if !tab.pinned && ui.button("Close Tab").clicked() {
        actions.close = Some(tab.id);
        ui.close();
    }
}

fn domain_label(url: &str, fallback: &str) -> String {
    let host = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.");
    if host.is_empty() {
        fallback.to_owned()
    } else {
        host.to_owned()
    }
}

fn domain_monogram(url: &str, title: &str) -> String {
    let src = domain_label(url, title);
    src.chars()
        .find(char::is_ascii_alphanumeric)
        .unwrap_or('E')
        .to_ascii_uppercase()
        .to_string()
}
