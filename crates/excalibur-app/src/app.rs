use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use excalibur_cef::{CefEvent, CefRuntime};
use excalibur_core::{
    normalize_url_or_search, sample_process_tree_memory, BrowserConfig, MemorySnapshot, Storage,
    TabManager,
};
use raw_window_handle::HasWindowHandle;

use crate::{
    drawer::{truncate_text, ActiveDrawer, DrawerAction, DrawerState},
    icons::{icon_button, paint_icon, IconKind},
    theme,
};

pub struct ExcaliburBrowserApp {
    config: BrowserConfig,
    storage: Storage,
    runtime: Rc<CefRuntime>,
    tabs: TabManager,
    drawer: DrawerState,
    omnibox_id: egui::Id,
    omnibox_text: String,
    focus_omnibox_next_frame: bool,
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

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (ctrl_t, ctrl_w, ctrl_l, ctrl_d, ctrl_b, ctrl_h, reload_key, tab_fwd, tab_bwd) = ctx
            .input(|i| {
                let cmd = i.modifiers.command;
                let shift = i.modifiers.shift;
                (
                    cmd && i.key_pressed(egui::Key::T),
                    cmd && i.key_pressed(egui::Key::W),
                    cmd && i.key_pressed(egui::Key::L),
                    cmd && i.key_pressed(egui::Key::D),
                    cmd && i.key_pressed(egui::Key::B),
                    cmd && i.key_pressed(egui::Key::H),
                    i.key_pressed(egui::Key::F5) || (cmd && i.key_pressed(egui::Key::R)),
                    cmd && !shift && i.key_pressed(egui::Key::Tab),
                    cmd && shift && i.key_pressed(egui::Key::Tab),
                )
            });

        if ctrl_t {
            self.open_new_tab(self.config.home_url.clone(), true);
        }
        if ctrl_w {
            if let Some(active) = self.tabs.active_tab() {
                if !active.pinned {
                    let id = active.id;
                    self.close_tab(ctx, id);
                }
            }
        }
        if tab_fwd {
            if let Some(next_id) = self.tabs.cycle_tab(true) {
                self.sync_after_cycle(ctx, next_id);
            }
        } else if tab_bwd {
            if let Some(prev_id) = self.tabs.cycle_tab(false) {
                self.sync_after_cycle(ctx, prev_id);
            }
        }
        if ctrl_l {
            self.focus_omnibox_next_frame = true;
        }
        if ctrl_d {
            self.toggle_bookmark_active();
        }
        if ctrl_b {
            self.drawer
                .toggle(ActiveDrawer::Bookmarks, &self.storage, &self.config);
        }
        if ctrl_h {
            self.drawer
                .toggle(ActiveDrawer::History, &self.storage, &self.config);
        }
        if reload_key {
            if let Some(active_id) = self.active_tab_id() {
                if self.tabs.tab(active_id).is_some_and(|t| t.is_sleeping) {
                    self.wake_tab(active_id);
                } else {
                    self.runtime.reload(active_id);
                }
            }
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

    fn show_tab_strip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut switch_target = None;
        let mut close_target = None;
        let mut pin_target = None;
        let mut sleep_target = None;
        let mut wake_target = None;
        let mut duplicate_url = None;
        let mut open_new = false;

        let tab_frame = egui::Frame::NONE
            .fill(theme::BG_TAB_BAR)
            .inner_margin(egui::Margin::symmetric(8, 3))
            .stroke(egui::Stroke::new(1.0, theme::BORDER_SUBTLE));

        egui::Panel::top("excalibur_tabs")
            .exact_size(34.0)
            .frame(tab_frame)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let active_id = self.active_tab_id();

                    egui::ScrollArea::horizontal()
                        .id_salt("tab_scroll")
                        .show(ui, |ui| {
                            ui.horizontal_centered(|ui| {
                                for tab in self.tabs.tabs() {
                                    let is_active = Some(tab.id) == active_id;
                                    let max_len = if tab.pinned { 10 } else { 20 };
                                    let label_text = truncate_text(&tab.title, max_len);

                                    let fill = if is_active {
                                        theme::BG_ELEVATED
                                    } else {
                                        theme::BG_TAB_BAR
                                    };
                                    let text_color = if tab.is_sleeping {
                                        theme::TEXT_MUTED
                                    } else if is_active {
                                        theme::TEXT_PRIMARY
                                    } else {
                                        theme::TEXT_SECONDARY
                                    };

                                    let rich = if tab.pinned {
                                        egui::RichText::new(format!("[P] {label_text}"))
                                            .color(text_color)
                                    } else {
                                        egui::RichText::new(label_text).color(text_color)
                                    };

                                    let stroke = if is_active {
                                        egui::Stroke::new(1.0, theme::BORDER_STRONG)
                                    } else {
                                        egui::Stroke::new(1.0, theme::BORDER_SUBTLE)
                                    };

                                    let resp = ui
                                        .add(
                                            egui::Button::new(rich)
                                                .fill(fill)
                                                .stroke(stroke)
                                                .min_size(egui::vec2(0.0, 24.0)),
                                        )
                                        .on_hover_text(format!(
                                            "{}{}",
                                            tab.url,
                                            if tab.is_sleeping { " (Sleeping)" } else { "" }
                                        ));

                                    if is_active {
                                        let r = resp.rect;
                                        ui.painter().line_segment(
                                            [
                                                egui::pos2(r.min.x + 2.0, r.min.y + 1.0),
                                                egui::pos2(r.max.x - 2.0, r.min.y + 1.0),
                                            ],
                                            egui::Stroke::new(2.0, theme::ACCENT),
                                        );
                                    } else if tab.is_sleeping {
                                        let icon_rect = egui::Rect::from_center_size(
                                            egui::pos2(resp.rect.min.x + 7.0, resp.rect.center().y),
                                            egui::vec2(8.0, 8.0),
                                        );
                                        paint_icon(
                                            ui.painter(),
                                            icon_rect,
                                            IconKind::Sleep,
                                            theme::STATUS_WARNING,
                                        );
                                    } else if tab.is_loading {
                                        ui.painter().circle_filled(
                                            egui::pos2(resp.rect.min.x + 6.0, resp.rect.center().y),
                                            2.5,
                                            theme::ACCENT,
                                        );
                                    }

                                    if resp.clicked() {
                                        switch_target = Some(tab.id);
                                    }
                                    if resp.middle_clicked() && !tab.pinned {
                                        close_target = Some(tab.id);
                                    }
                                    resp.context_menu(|ui| {
                                        let pin_label =
                                            if tab.pinned { "Unpin Tab" } else { "Pin Tab" };
                                        if ui.button(pin_label).clicked() {
                                            pin_target = Some(tab.id);
                                            ui.close();
                                        }
                                        if tab.is_sleeping {
                                            if ui.button("Wake Tab").clicked() {
                                                wake_target = Some(tab.id);
                                                ui.close();
                                            }
                                        } else if !tab.pinned && ui.button("Sleep Tab").clicked() {
                                            sleep_target = Some(tab.id);
                                            ui.close();
                                        }
                                        if ui.button("Duplicate Tab").clicked() {
                                            duplicate_url = Some(tab.url.clone());
                                            ui.close();
                                        }
                                        if !tab.pinned && ui.button("Close Tab").clicked() {
                                            close_target = Some(tab.id);
                                            ui.close();
                                        }
                                    });

                                    if !tab.pinned
                                        && icon_button(ui, IconKind::Close, true, false, None)
                                            .on_hover_text("Close tab (Ctrl+W)")
                                            .clicked()
                                    {
                                        close_target = Some(tab.id);
                                    }
                                }

                                if icon_button(ui, IconKind::Plus, true, false, None)
                                    .on_hover_text("New tab (Ctrl+T)")
                                    .clicked()
                                {
                                    open_new = true;
                                }
                            });
                        });
                });
            });

        if let Some(id) = switch_target {
            self.switch_to_tab(ctx, id);
        }
        if let Some(id) = pin_target {
            self.tabs.toggle_pin(id);
        }
        if let Some(id) = sleep_target {
            self.sleep_tab(id);
        }
        if let Some(id) = wake_target {
            self.wake_tab(id);
        }
        if let Some(url) = duplicate_url {
            self.open_new_tab(url, false);
        }
        if let Some(id) = close_target {
            self.close_tab(ctx, id);
        }
        if open_new {
            self.open_new_tab(self.config.home_url.clone(), true);
        }
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
        self.show_tab_strip(ui, &ctx);

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
        let idle_sleepable_count = self.tabs.idle_sleepable_ids().len();
        let sleeping_count = self.tabs.tabs().iter().filter(|t| t.is_sleeping).count();

        let mut omnibox_focused = false;
        let mut submit_navigation = false;
        let mut toggle_bookmark = false;
        let mut toggle_blocker = false;
        let mut trigger_sleep_idle = false;

        let toolbar_frame = egui::Frame::NONE
            .fill(theme::BG_TOOLBAR)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .stroke(egui::Stroke::new(1.0, theme::BORDER_SUBTLE));

        egui::Panel::top("excalibur_toolbar")
            .exact_size(40.0)
            .frame(toolbar_frame)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    if icon_button(ui, IconKind::Back, can_back, false, None)
                        .on_hover_text("Back")
                        .clicked()
                    {
                        if let Some(id) = active_id {
                            self.runtime.go_back(id);
                        }
                    }
                    if icon_button(ui, IconKind::Forward, can_forward, false, None)
                        .on_hover_text("Forward")
                        .clicked()
                    {
                        if let Some(id) = active_id {
                            self.runtime.go_forward(id);
                        }
                    }
                    let (reload_icon, reload_tip) = if is_loading {
                        (IconKind::Stop, "Stop loading")
                    } else {
                        (IconKind::Reload, "Reload (F5)")
                    };
                    if icon_button(ui, reload_icon, active_id.is_some(), false, None)
                        .on_hover_text(reload_tip)
                        .clicked()
                    {
                        if let Some(id) = active_id {
                            if is_sleeping {
                                self.wake_tab(id);
                            } else if is_loading {
                                self.runtime.stop_load(id);
                            } else {
                                self.runtime.reload(id);
                            }
                        }
                    }

                    ui.add_space(4.0);

                    let right_cluster_width = 310.0;
                    let avail = (ui.available_width() - right_cluster_width).max(120.0);
                    let has_omnibox_focus = ui.ctx().memory(|m| m.has_focus(self.omnibox_id));
                    let field_stroke = if has_omnibox_focus {
                        theme::ACCENT
                    } else {
                        theme::BORDER_SUBTLE
                    };
                    let field_frame = egui::Frame::NONE
                        .fill(theme::BG_FIELD)
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .stroke(egui::Stroke::new(1.0, field_stroke));
                    field_frame.show(ui, |ui| {
                        let response = ui.add_sized(
                            [(avail - 16.0).max(80.0), 18.0],
                            egui::TextEdit::singleline(&mut self.omnibox_text)
                                .id(self.omnibox_id)
                                .frame(egui::Frame::NONE)
                                .hint_text("Search or enter URL (Ctrl+L)"),
                        );
                        if self.focus_omnibox_next_frame {
                            response.request_focus();
                            self.focus_omnibox_next_frame = false;
                        }
                        omnibox_focused = response.has_focus();
                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            submit_navigation = true;
                        }
                    });

                    ui.add_space(4.0);

                    let (star_icon, star_tint) = if is_bookmarked {
                        (IconKind::StarFilled, Some(theme::STATUS_WARNING))
                    } else {
                        (IconKind::StarOutline, None)
                    };
                    if icon_button(ui, star_icon, active_id.is_some(), false, star_tint)
                        .on_hover_text("Toggle bookmark (Ctrl+D)")
                        .clicked()
                    {
                        toggle_bookmark = true;
                    }

                    let shield_tint = if self.config.ad_block_enabled {
                        theme::STATUS_SUCCESS
                    } else {
                        theme::TEXT_MUTED
                    };
                    if icon_button(
                        ui,
                        IconKind::Shield,
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
                        tab_blocked,
                        self.tabs.total_blocked_count()
                    ))
                    .clicked()
                    {
                        toggle_blocker = true;
                    }
                    if tab_blocked > 0 {
                        ui.label(
                            egui::RichText::new(format!("{tab_blocked}"))
                                .monospace()
                                .size(11.0)
                                .color(theme::STATUS_SUCCESS),
                        );
                    }

                    let ram_label = format!("{:.0} MB", self.memory.total_mb);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(ram_label)
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
                        sleeping_count
                    ));

                    if icon_button(
                        ui,
                        IconKind::Sleep,
                        idle_sleepable_count > 0,
                        false,
                        (idle_sleepable_count > 0).then_some(theme::STATUS_WARNING),
                    )
                    .on_hover_text(format!("Sleep {idle_sleepable_count} idle background tabs"))
                    .clicked()
                    {
                        trigger_sleep_idle = true;
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
                });
            });

        if toggle_bookmark {
            self.toggle_bookmark_active();
        }
        if toggle_blocker {
            self.toggle_ad_block();
        }
        if trigger_sleep_idle {
            self.sleep_all_idle();
        }
        if submit_navigation {
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

        self.process_cef_events(&ctx, omnibox_focused);

        egui::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(theme::BG_CANVAS))
            .show(ui, |ui| {
                let Some(tab_id) = self.active_tab_id() else {
                    return;
                };
                if is_sleeping {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(120.0);
                            ui.label(
                                egui::RichText::new("Tab is asleep to save memory")
                                    .size(16.0)
                                    .color(theme::TEXT_PRIMARY),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(&current_url)
                                    .size(12.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                            ui.add_space(12.0);
                            if ui.button("Wake Tab").clicked() {
                                self.wake_tab(tab_id);
                            }
                        });
                    });
                    return;
                }

                let rect = ui.available_rect_before_wrap();
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

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.runtime.shutdown_cleanly();
    }
}
