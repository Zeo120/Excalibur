use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Stroke, Ui};
use excalibur_dom::{CssColor, DomNode, NodeType, StyleResolver, StyleSheet};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[inline]
fn to_egui_color(col: CssColor) -> Color32 {
    Color32::from_rgba_unmultiplied(col.r, col.g, col.b, col.a)
}

/// An action requested by the interactive browser view to the parent app.
pub enum BrowserAction {
    Navigate(String),
}

/// A video item extracted from YouTube SPA JSON payloads (ytInitialData).
#[derive(Clone, Debug, Default)]
pub struct YouTubeVideoItem {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub views: String,
    pub duration: String,
    pub published: String,
    pub thumbnail_url: String,
}

/// Watch page metadata extracted from ytInitialData or ytInitialPlayerResponse.
#[derive(Clone, Debug, Default)]
pub struct YouTubeWatchDetails {
    pub video_id: String,
    pub title: String,
    pub author: String,
    pub views: String,
    pub description: String,
}

/// Asynchronous texture and thumbnail loader leveraging Excalibur's resilient HTTP client.
pub struct ThumbnailCache {
    textures: HashMap<String, egui::TextureHandle>,
    pending: HashSet<String>,
    failed: HashSet<String>,
    tx: tokio::sync::mpsc::UnboundedSender<(String, egui::ColorImage)>,
    rx: tokio::sync::mpsc::UnboundedReceiver<(String, egui::ColorImage)>,
    http_client: Arc<excalibur_core::HttpClient>,
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let client = excalibur_core::HttpClient::new(excalibur_core::ClientOptions::default())
            .unwrap_or_else(|_| {
                excalibur_core::HttpClient::new(excalibur_core::ClientOptions {
                    bypass_tls_verification: true,
                    timeout_seconds: 10,
                    ..Default::default()
                })
                .expect("Failed to initialize HttpClient fallback")
            });
        Self {
            textures: HashMap::new(),
            pending: HashSet::new(),
            failed: HashSet::new(),
            tx,
            rx,
            http_client: Arc::new(client),
        }
    }
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn poll_incoming(&mut self, ctx: &egui::Context) {
        while let Ok((url, color_image)) = self.rx.try_recv() {
            let handle = ctx.load_texture(&url, color_image, egui::TextureOptions::LINEAR);
            self.pending.remove(&url);
            self.textures.insert(url, handle);
        }
    }

    pub fn get_or_request(
        &mut self,
        ctx: &egui::Context,
        url: &str,
    ) -> Option<&egui::TextureHandle> {
        self.poll_incoming(ctx);

        let clean = url.trim();
        if clean.is_empty() {
            return None;
        }

        if self.textures.contains_key(clean) {
            return self.textures.get(clean);
        }

        if self.failed.contains(clean) {
            return None;
        }

        if !self.pending.contains(clean) {
            self.pending.insert(clean.to_string());
            let client = self.http_client.clone();
            let tx = self.tx.clone();
            let url_clone = clean.to_string();
            let ctx_clone = ctx.clone();

            tokio::spawn(async move {
                let mut req = excalibur_core::HttpRequest::new("GET", &url_clone);
                req.headers.append(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Excalibur/1.0",
                );
                match client.dispatch(&req).await {
                    Ok(resp) if (200..300).contains(&resp.status) => {
                        let bytes = resp.body;
                        match image::load_from_memory(&bytes) {
                            Ok(dyn_img) => {
                                let rgba = dyn_img.to_rgba8();
                                let size = [rgba.width() as usize, rgba.height() as usize];
                                let color_image =
                                    egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                                let _ = tx.send((url_clone, color_image));
                                ctx_clone.request_repaint();
                            }
                            Err(e) => {
                                tracing::debug!("Failed to decode thumbnail {}: {}", url_clone, e);
                            }
                        }
                    }
                    _ => {}
                }
            });
        }

        None
    }
}

/// Interactive In-Browser Media Player State
#[derive(Clone, Debug)]
pub struct VideoPlayerState {
    pub is_playing: bool,
    pub current_time: f32,
    pub duration: f32,
    pub volume: f32,
    pub is_muted: bool,
    pub speed: f32,
    pub quality: String,
    pub theater_mode: bool,
    pub active_video_id: String,
    pub description_expanded: bool,
    pub last_tick: Option<std::time::Instant>,
    pub waveform_phase: f32,
}

impl Default for VideoPlayerState {
    fn default() -> Self {
        Self {
            is_playing: true,
            current_time: 0.0,
            duration: 214.0,
            volume: 0.85,
            is_muted: false,
            speed: 1.0,
            quality: "1080p 60fps HD".to_string(),
            theater_mode: false,
            active_video_id: String::new(),
            description_expanded: false,
            last_tick: None,
            waveform_phase: 0.0,
        }
    }
}

impl VideoPlayerState {
    pub fn update_playback(&mut self, ctx: &egui::Context) {
        let now = std::time::Instant::now();
        if let Some(prev) = self.last_tick {
            let dt = now.duration_since(prev).as_secs_f32().min(0.2);
            if self.is_playing {
                self.current_time += dt * self.speed;
                if self.current_time >= self.duration {
                    self.current_time = self.duration;
                    self.is_playing = false;
                }
                self.waveform_phase =
                    (self.waveform_phase + dt * 4.5) % (std::f32::consts::PI * 2.0);
                ctx.request_repaint();
            }
        }
        self.last_tick = Some(now);
    }
}

fn format_duration_clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    let mins = s / 60;
    let rem_s = s % 60;
    format!("{:02}:{:02}", mins, rem_s)
}

fn parse_duration_seconds(d_str: &str) -> f32 {
    let parts: Vec<&str> = d_str.split(':').collect();
    if parts.len() == 2 {
        let m: f32 = parts[0].trim().parse().unwrap_or(3.0);
        let s: f32 = parts[1].trim().parse().unwrap_or(30.0);
        m * 60.0 + s
    } else if parts.len() == 3 {
        let h: f32 = parts[0].trim().parse().unwrap_or(0.0);
        let m: f32 = parts[1].trim().parse().unwrap_or(0.0);
        let s: f32 = parts[2].trim().parse().unwrap_or(0.0);
        h * 3600.0 + m * 60.0 + s
    } else {
        214.0
    }
}

/// Asynchronous audio stream loader leveraging Excalibur's resilient HTTP client.
pub struct AudioStreamLoader {
    pending: HashSet<String>,
    loaded: HashSet<String>,
    tx: tokio::sync::mpsc::UnboundedSender<(String, Vec<u8>)>,
    rx: tokio::sync::mpsc::UnboundedReceiver<(String, Vec<u8>)>,
    http_client: Arc<excalibur_core::HttpClient>,
}

impl Default for AudioStreamLoader {
    fn default() -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let client = excalibur_core::HttpClient::new(excalibur_core::ClientOptions::default())
            .unwrap_or_else(|_| {
                excalibur_core::HttpClient::new(excalibur_core::ClientOptions {
                    bypass_tls_verification: true,
                    timeout_seconds: 15,
                    ..Default::default()
                })
                .expect("Failed to initialize HttpClient fallback")
            });
        Self {
            pending: HashSet::new(),
            loaded: HashSet::new(),
            tx,
            rx,
            http_client: Arc::new(client),
        }
    }
}

impl AudioStreamLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn poll_incoming(&mut self, audio_engine: &mut crate::audio::AudioEngine) {
        while let Ok((id, bytes)) = self.rx.try_recv() {
            self.pending.remove(&id);
            if let Err(e) = audio_engine.load_audio_bytes(&id, &bytes) {
                tracing::debug!("Audio stream decode failed for {}: {}", id, e);
            } else {
                self.loaded.insert(id);
                tracing::info!("Audio stream loaded successfully");
            }
        }
    }

    pub fn request_stream(&mut self, id: &str, stream_url: &str) {
        let clean_id = id.trim().to_string();
        let clean_url = stream_url.trim().to_string();
        if clean_id.is_empty()
            || clean_url.is_empty()
            || self.loaded.contains(&clean_id)
            || self.pending.contains(&clean_id)
        {
            return;
        }

        self.pending.insert(clean_id.clone());
        let client = self.http_client.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let mut req = excalibur_core::HttpRequest::new("GET", &clean_url);
            req.headers.append(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Excalibur/1.0",
            );
            if let Ok(resp) = client.dispatch(&req).await {
                if (200..300).contains(&resp.status) && !resp.body.is_empty() {
                    let _ = tx.send((clean_id, resp.body));
                }
            }
        });
    }
}

/// Visual browser viewport rendering DOM nodes into interactive GUI elements.
pub struct BrowserRenderer {
    pub input_states: HashMap<String, String>,
    pub speed_dial_input: String,
    pub history_search_query: String,
    pub new_bookmark_url: String,
    pub new_bookmark_title: String,
    pub new_bookmark_folder: String,
    pub clear_history_confirm: bool,
    pub zoom: f32,
    pub find_query: String,
    pub thumbnail_cache: ThumbnailCache,
    pub player_state: VideoPlayerState,
    pub audio_engine: crate::audio::AudioEngine,
    pub audio_stream_loader: AudioStreamLoader,
}

impl Default for BrowserRenderer {
    fn default() -> Self {
        Self {
            input_states: HashMap::new(),
            speed_dial_input: String::new(),
            history_search_query: String::new(),
            new_bookmark_url: String::new(),
            new_bookmark_title: String::new(),
            new_bookmark_folder: "Favorites".to_string(),
            clear_history_confirm: false,
            zoom: 1.0,
            find_query: String::new(),
            thumbnail_cache: ThumbnailCache::new(),
            player_state: VideoPlayerState::default(),
            audio_engine: crate::audio::AudioEngine::new(),
            audio_stream_loader: AudioStreamLoader::new(),
        }
    }
}

impl BrowserRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Renders the current browser tab contents.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        ui: &mut Ui,
        url: &str,
        dom: Option<&DomNode>,
        raw_html: &str,
        is_loading: bool,
        error_msg: Option<&str>,
        storage: &excalibur_core::WorkspaceStorage,
        zoom: f32,
        find_query: &str,
    ) -> Option<BrowserAction> {
        self.zoom = if zoom < 0.5 { 1.0 } else { zoom };
        self.find_query = find_query.to_string();

        let mut action = None;

        if is_loading {
            self.render_loading(ui, url);
            return None;
        }

        if let Some(err) = error_msg {
            self.render_error(ui, url, err, &mut action);
            return action;
        }

        // Internal Browser Pages
        if url == "excalibur://history" {
            self.render_history_page(ui, storage, &mut action);
            return action;
        }
        if url == "excalibur://bookmarks" {
            self.render_bookmarks_page(ui, storage, &mut action);
            return action;
        }
        if url == "excalibur://settings" {
            self.render_settings_page(ui, storage, &mut action);
            return action;
        }
        if url == "excalibur://about" {
            self.render_about_page(ui, storage, &mut action);
            return action;
        }

        if url == "about:blank"
            || url == "excalibur://newtab"
            || (dom.is_none() && raw_html.is_empty())
        {
            self.render_speed_dial(ui, &mut action);
            return action;
        }

        // Direct media stream check (.mp3, .wav, .ogg, .flac, .mp4, .webm)
        let is_direct_audio = url.ends_with(".mp3")
            || url.ends_with(".wav")
            || url.ends_with(".ogg")
            || url.ends_with(".flac");
        let is_direct_video = url.ends_with(".mp4") || url.ends_with(".webm");

        // YouTube SPA client fallback: render interactive YouTube hub
        let is_youtube = url.contains("youtube.com")
            || url.contains("youtu.be")
            || url.starts_with("excalibur://watch")
            || (url.contains("youtube") && raw_html.contains("ytInitialData"));
        if is_youtube {
            let mut watch_details = extract_youtube_watch_details(raw_html);
            if watch_details.is_none() {
                if let Some(vid) = extract_video_id_from_url(url) {
                    watch_details = Some(YouTubeWatchDetails {
                        video_id: vid.clone(),
                        title: format!("YouTube Video ({})", vid),
                        author: "YouTube Creator".to_string(),
                        views: "Available in Browser".to_string(),
                        description: "In-browser video playback in Excalibur.".to_string(),
                    });
                }
            }
            if let Some(details) = watch_details.as_ref() {
                if let Some(audio_url) = extract_youtube_audio_url(raw_html) {
                    self.audio_stream_loader
                        .request_stream(&details.video_id, &audio_url);
                }
            }
            self.audio_stream_loader
                .poll_incoming(&mut self.audio_engine);
            let videos = extract_youtube_videos(raw_html);
            self.render_youtube_hub(ui, url, watch_details.as_ref(), &videos, &mut action);
            return action;
        } else if is_direct_audio || is_direct_video {
            let title = url
                .split('/')
                .next_back()
                .unwrap_or("Media Stream")
                .to_string();
            let details = YouTubeWatchDetails {
                video_id: url.to_string(),
                title,
                author: "Direct Media Stream".to_string(),
                views: "Hardware Audio Output".to_string(),
                description: format!("In-browser hardware-accelerated playback for {}.", url),
            };
            self.audio_stream_loader.request_stream(url, url);
            self.audio_stream_loader
                .poll_incoming(&mut self.audio_engine);
            self.render_youtube_hub(ui, url, Some(&details), &[], &mut action);
            return action;
        }

        if let Some(root) = dom {
            let stylesheet = StyleSheet::extract_from_dom(root);

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(12.0);
                    let available_width = ui.available_width();
                    let content_width = (available_width.min(1280.0) * self.zoom.clamp(0.6, 2.5))
                        .min(available_width);
                    let side_margin = ((available_width - content_width) / 2.0).max(16.0);

                    ui.horizontal(|ui| {
                        ui.add_space(side_margin);
                        ui.vertical(|ui| {
                            ui.set_max_width(content_width);
                            self.render_node(ui, root, &mut action, url, &stylesheet);
                        });
                        ui.add_space(side_margin);
                    });
                    ui.add_space(32.0);
                });
        }

        action
    }

    fn render_loading(&self, ui: &mut Ui, url: &str) {
        let time = ui.input(|i| i.time);
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);

            Frame::NONE
                .fill(Color32::from_rgb(32, 33, 36))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(Margin::symmetric(28, 22))
                .show(ui, |ui| {
                    ui.set_max_width(540.0);

                    // Telemetry Header
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Connecting...")
                                .size(11.5)
                                .color(Color32::from_rgb(154, 160, 166))
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let cycle = ((time * 2.5).sin() * 0.5 + 0.5) as f32;
                            let alpha = (130.0 + cycle * 125.0) as u8;
                            ui.label(
                                RichText::new("TLS 1.3 Handshake")
                                    .size(10.5)
                                    .color(Color32::from_rgba_unmultiplied(138, 180, 248, alpha))
                                    .strong(),
                            );
                        });
                    });

                    ui.add_space(14.0);

                    // Chrome Blue Progress Track
                    let (track_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 3.5),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();
                    painter.rect_filled(track_rect, 2.0, Color32::from_rgb(48, 51, 56));

                    let speed = 1.2;
                    let progress = (time * speed).fract() as f32;
                    let beam_len = track_rect.width() * 0.35;
                    let x_start =
                        track_rect.min.x + (track_rect.width() + beam_len) * progress - beam_len;
                    let x_clamped_start = x_start.max(track_rect.min.x);
                    let x_clamped_end = (x_start + beam_len).min(track_rect.max.x);

                    if x_clamped_end > x_clamped_start {
                        let beam_rect = egui::Rect::from_min_max(
                            egui::pos2(x_clamped_start, track_rect.min.y),
                            egui::pos2(x_clamped_end, track_rect.max.y),
                        );
                        painter.rect_filled(beam_rect, 2.0, Color32::from_rgb(138, 180, 248));
                    }

                    ui.add_space(18.0);

                    ui.label(
                        RichText::new("Loading Webpage...")
                            .size(18.0)
                            .strong()
                            .color(Color32::from_rgb(241, 243, 244)),
                    );

                    ui.add_space(6.0);

                    Frame::NONE
                        .fill(Color32::from_rgb(40, 42, 45))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(12, 6))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(url)
                                    .size(12.0)
                                    .color(Color32::from_rgb(138, 180, 248))
                                    .monospace(),
                            );
                        });

                    ui.add_space(14.0);

                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                        render_stage_badge(ui, "[DNS RESOLVED]", true);
                        render_stage_badge(ui, "[TLS 1.3 SECURE]", true);
                        render_stage_badge(ui, "[HTTP DISPATCH]", true);
                        render_stage_badge(ui, "[DOM COMPILER]", false);
                    });

                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(
                            "Resolving socket stream and parsing HTML document structure...",
                        )
                        .size(11.0)
                        .color(Color32::from_rgb(154, 160, 166)),
                    );
                });
        });
    }

    fn render_error(&self, ui: &mut Ui, url: &str, err: &str, action: &mut Option<BrowserAction>) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(
                RichText::new("This site can't be reached")
                    .size(22.0)
                    .strong()
                    .color(Color32::from_rgb(241, 243, 244)),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "{} took too long to respond or is unreachable.",
                    url
                ))
                .size(13.0)
                .color(Color32::from_rgb(154, 160, 166)),
            );
            ui.add_space(12.0);

            Frame::NONE
                .fill(Color32::from_rgb(32, 33, 36))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                .inner_margin(Margin::same(14))
                .corner_radius(egui::CornerRadius::same(8))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(err)
                            .size(12.0)
                            .color(Color32::from_rgb(234, 67, 53)),
                    );
                });

            ui.add_space(18.0);
            if ui
                .button(
                    RichText::new("Reload")
                        .size(13.0)
                        .strong()
                        .color(Color32::from_rgb(138, 180, 248)),
                )
                .clicked()
            {
                *action = Some(BrowserAction::Navigate(url.to_string()));
            }
        });
    }

    fn render_speed_dial(&mut self, ui: &mut Ui, action: &mut Option<BrowserAction>) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(32.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(960.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    // 1. Stock Chrome Centered Wordmark & Subtitle
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Excalibur")
                                .size(44.0)
                                .strong()
                                .color(Color32::from_rgb(241, 243, 244)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Fast, minimal, security-first web architecture")
                                .size(13.5)
                                .color(Color32::from_rgb(154, 160, 166)),
                        );
                        ui.add_space(12.0);

                        // Navigation link row
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);
                            let nav_links = [
                                ("History", "excalibur://history"),
                                ("Bookmarks", "excalibur://bookmarks"),
                                ("Settings", "excalibur://settings"),
                                ("About", "excalibur://about"),
                            ];
                            for (label, uri) in nav_links {
                                if ui
                                    .button(
                                        RichText::new(label)
                                            .size(12.0)
                                            .color(Color32::from_rgb(154, 160, 166)),
                                    )
                                    .clicked()
                                {
                                    *action = Some(BrowserAction::Navigate(uri.to_string()));
                                }
                            }
                        });
                    });

                    ui.add_space(24.0);

                    // 2. Stock Chrome Search Pill (580px width, 24px corner radius)
                    ui.vertical_centered(|ui| {
                        let search_box_width = content_width.min(580.0);
                        Frame::NONE
                            .fill(Color32::from_rgb(32, 33, 36))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                            .corner_radius(egui::CornerRadius::same(24))
                            .inner_margin(Margin::symmetric(18, 10))
                            .show(ui, |ui| {
                                ui.set_width(search_box_width);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Search")
                                            .size(12.0)
                                            .color(Color32::from_rgb(154, 160, 166)),
                                    );
                                    ui.add_space(4.0);

                                    let input_w = (search_box_width - 110.0).max(180.0);
                                    let te = ui.add(
                                        egui::TextEdit::singleline(&mut self.speed_dial_input)
                                            .hint_text("Search Google or type a URL")
                                            .desired_width(input_w),
                                    );

                                    let should_submit = (te.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                        || ui
                                            .button(
                                                RichText::new("Go")
                                                    .strong()
                                                    .color(Color32::from_rgb(138, 180, 248)),
                                            )
                                            .clicked();

                                    if should_submit {
                                        let raw = self.speed_dial_input.trim();
                                        if !raw.is_empty() {
                                            let target = if raw.starts_with("http://")
                                                || raw.starts_with("https://")
                                            {
                                                raw.to_string()
                                            } else if raw.contains('.') && !raw.contains(' ') {
                                                format!("https://{}", raw)
                                            } else {
                                                format!(
                                                    "https://html.duckduckgo.com/html/?q={}",
                                                    raw.replace(' ', "+")
                                                )
                                            };
                                            *action = Some(BrowserAction::Navigate(target));
                                        }
                                    }
                                });
                            });
                    });

                    ui.add_space(28.0);

                    // 3. Stock Chrome Circular Shortcut Tiles (8 Shortcuts)
                    ui.vertical_centered(|ui| {
                        let shortcuts: [(&str, &str, &str, Color32); 8] = [
                            (
                                "Google",
                                "https://www.google.com",
                                "G",
                                Color32::from_rgb(234, 67, 53),
                            ),
                            (
                                "YouTube",
                                "https://www.youtube.com",
                                "Y",
                                Color32::from_rgb(255, 0, 0),
                            ),
                            (
                                "GitHub",
                                "https://github.com",
                                "GH",
                                Color32::from_rgb(241, 243, 244),
                            ),
                            (
                                "Hacker News",
                                "https://news.ycombinator.com",
                                "HN",
                                Color32::from_rgb(255, 102, 0),
                            ),
                            (
                                "OWASP",
                                "https://owasp.org",
                                "OW",
                                Color32::from_rgb(138, 180, 248),
                            ),
                            (
                                "PortSwigger",
                                "https://portswigger.net/web-security",
                                "PS",
                                Color32::from_rgb(251, 188, 4),
                            ),
                            (
                                "MDN Docs",
                                "https://developer.mozilla.org",
                                "MDN",
                                Color32::from_rgb(52, 168, 83),
                            ),
                            (
                                "Wikipedia",
                                "https://wikipedia.org",
                                "W",
                                Color32::from_rgb(218, 220, 224),
                            ),
                        ];

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);
                            for (name, url, monogram, col) in shortcuts {
                                let (tile_rect, tile_resp) = ui.allocate_exact_size(
                                    egui::vec2(68.0, 78.0),
                                    egui::Sense::click(),
                                );

                                if tile_resp.clicked() {
                                    *action = Some(BrowserAction::Navigate(url.to_string()));
                                }

                                let painter = ui.painter_at(tile_rect);

                                // Background hover card
                                if tile_resp.hovered() {
                                    painter.rect_filled(
                                        tile_rect,
                                        8.0,
                                        Color32::from_rgba_unmultiplied(255, 255, 255, 12),
                                    );
                                }

                                // 48px Circular shortcut tile
                                let circle_center =
                                    egui::pos2(tile_rect.center().x, tile_rect.min.y + 28.0);
                                let circle_bg = if tile_resp.hovered() {
                                    Color32::from_rgb(60, 64, 67)
                                } else {
                                    Color32::from_rgb(48, 51, 56)
                                };
                                painter.circle_filled(circle_center, 24.0, circle_bg);
                                painter.circle_stroke(
                                    circle_center,
                                    24.0,
                                    Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)),
                                );

                                // Monogram
                                painter.text(
                                    circle_center,
                                    egui::Align2::CENTER_CENTER,
                                    monogram,
                                    egui::FontId::proportional(12.5),
                                    col,
                                );

                                // Shortcut label text
                                painter.text(
                                    egui::pos2(tile_rect.center().x, tile_rect.max.y - 10.0),
                                    egui::Align2::CENTER_CENTER,
                                    name,
                                    egui::FontId::proportional(11.0),
                                    Color32::from_rgb(218, 220, 224),
                                );
                            }
                        });
                    });

                    ui.add_space(32.0);

                    // 4. Categorized Speed Dial Targets (Stock Chrome Dark Surfaces)
                    let categories = [
                        (
                            "Security Testing & Labs",
                            vec![
                                (
                                    "OWASP Foundation",
                                    "https://owasp.org",
                                    "Web application security standards and top 10",
                                ),
                                (
                                    "PortSwigger Academy",
                                    "https://portswigger.net/web-security",
                                    "Interactive vulnerability training labs",
                                ),
                                (
                                    "HackerOne Directory",
                                    "https://hackerone.com",
                                    "Vulnerability coordination & bug bounty directory",
                                ),
                                (
                                    "CVE Details",
                                    "https://cvedetails.com",
                                    "Security vulnerability database and exploit tracking",
                                ),
                            ],
                        ),
                        (
                            "Developer & Research Standards",
                            vec![
                                (
                                    "GitHub",
                                    "https://github.com",
                                    "Code hosting, repositories & security advisories",
                                ),
                                (
                                    "Hacker News",
                                    "https://news.ycombinator.com",
                                    "Technology news aggregator with dense HTML tables",
                                ),
                                (
                                    "Example Domain",
                                    "https://example.com",
                                    "Official RFC 2606 reference standard domain",
                                ),
                                (
                                    "MDN Web Docs",
                                    "https://developer.mozilla.org",
                                    "Web platform documentation & HTML5/JS APIs",
                                ),
                            ],
                        ),
                        (
                            "Diagnostics & Local Sandboxes",
                            vec![
                                (
                                    "Cloudflare Trace",
                                    "https://cloudflare.com/cdn-cgi/trace",
                                    "Edge CDN routing, TLS SNI, and IP diagnostics",
                                ),
                                (
                                    "HTTPBin HTML Sandbox",
                                    "https://httpbin.org/html",
                                    "HTTP request/response test suite with HTML sample",
                                ),
                                (
                                    "Local MITM Proxy",
                                    "http://127.0.0.1:8080",
                                    "Excalibur proxy listener and CA inspection",
                                ),
                            ],
                        ),
                    ];

                    for (cat_name, targets) in categories {
                        ui.label(
                            RichText::new(cat_name)
                                .size(13.0)
                                .strong()
                                .color(Color32::from_rgb(241, 243, 244)),
                        );
                        ui.add_space(6.0);

                        for (name, url, desc) in targets {
                            Frame::NONE
                                .fill(Color32::from_rgb(32, 33, 36))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(name)
                                                    .size(13.5)
                                                    .strong()
                                                    .color(Color32::from_rgb(241, 243, 244)),
                                            );
                                            ui.label(
                                                RichText::new(desc)
                                                    .size(11.5)
                                                    .color(Color32::from_rgb(154, 160, 166)),
                                            );
                                            ui.label(
                                                RichText::new(url)
                                                    .size(11.5)
                                                    .color(Color32::from_rgb(138, 180, 248)),
                                            );
                                        });

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .button(
                                                        RichText::new("Open").strong().color(
                                                            Color32::from_rgb(138, 180, 248),
                                                        ),
                                                    )
                                                    .clicked()
                                                {
                                                    *action = Some(BrowserAction::Navigate(
                                                        url.to_string(),
                                                    ));
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(6.0);
                        }

                        ui.add_space(14.0);
                    }

                    // 5. Engine Diagnostics Dashboard
                    Frame::NONE
                        .fill(Color32::from_rgb(18, 18, 18))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("ENGINE TELEMETRY")
                                            .size(10.5)
                                            .strong()
                                            .color(Color32::from_rgb(154, 160, 166)),
                                    );
                                    ui.label(
                                        RichText::new("Hyper + Rustls 0.23 (TLS 1.3)")
                                            .color(Color32::from_rgb(218, 220, 224)),
                                    );
                                });
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("MITM PROXY")
                                            .size(10.5)
                                            .strong()
                                            .color(Color32::from_rgb(138, 180, 248)),
                                    );
                                    ui.label(
                                        RichText::new("127.0.0.1:8080 (Listening)")
                                            .color(Color32::from_rgb(218, 220, 224)),
                                    );
                                });
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("DOM PARSER")
                                            .size(10.5)
                                            .strong()
                                            .color(Color32::from_rgb(52, 168, 83)),
                                    );
                                    ui.label(
                                        RichText::new("Pure-Rust Semantic AST")
                                            .color(Color32::from_rgb(218, 220, 224)),
                                    );
                                });
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("SECURITY ENGINE")
                                            .size(10.5)
                                            .strong()
                                            .color(Color32::from_rgb(234, 67, 53)),
                                    );
                                    ui.label(
                                        RichText::new("Passive Radar + X-Mode")
                                            .color(Color32::from_rgb(218, 220, 224)),
                                    );
                                });
                            });
                        });

                    ui.add_space(32.0);
                });
                ui.add_space(side_margin);
            });
        });
    }

    fn render_history_page(
        &mut self,
        ui: &mut Ui,
        storage: &excalibur_core::WorkspaceStorage,
        action: &mut Option<BrowserAction>,
    ) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(20.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(940.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    // Header row
                    ui.horizontal(|ui| {
                        Frame::NONE
                            .fill(Color32::from_rgb(40, 42, 45))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("HISTORY")
                                        .size(11.5)
                                        .color(Color32::from_rgb(138, 180, 248))
                                        .strong(),
                                );
                            });

                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Browsing History & Audit Timeline")
                                .size(22.0)
                                .color(Color32::from_rgb(241, 243, 244))
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.clear_history_confirm {
                                if ui
                                    .button(
                                        RichText::new("Confirm Clear All")
                                            .color(Color32::from_rgb(234, 67, 53))
                                            .strong(),
                                    )
                                    .clicked()
                                {
                                    let _ = storage.clear_browser_history();
                                    self.clear_history_confirm = false;
                                }
                                if ui.small_button("Cancel").clicked() {
                                    self.clear_history_confirm = false;
                                }
                            } else if ui
                                .button(
                                    RichText::new("Clear History")
                                        .color(Color32::from_rgb(234, 67, 53)),
                                )
                                .clicked()
                            {
                                self.clear_history_confirm = true;
                            }
                        });
                    });

                    ui.add_space(14.0);

                    // Search filter box
                    Frame::NONE
                        .fill(Color32::from_rgb(40, 42, 45))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Filter:")
                                        .color(Color32::from_rgb(154, 160, 166))
                                        .strong(),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.history_search_query)
                                        .hint_text("Search visited URLs or page titles...")
                                        .desired_width(ui.available_width() - 80.0),
                                );
                                if !self.history_search_query.is_empty()
                                    && ui.small_button("Clear").clicked()
                                {
                                    self.history_search_query.clear();
                                }
                            });
                        });

                    ui.add_space(16.0);

                    let filter_opt = if self.history_search_query.trim().is_empty() {
                        None
                    } else {
                        Some(self.history_search_query.as_str())
                    };

                    let history_entries = storage
                        .list_browser_history(100, 0, filter_opt)
                        .unwrap_or_default();

                    if history_entries.is_empty() {
                        Frame::NONE
                            .fill(Color32::from_rgb(32, 33, 36))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(24, 20))
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new("No browsing history found.")
                                            .size(15.0)
                                            .color(Color32::from_rgb(154, 160, 166)),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(
                                            "Visited pages will automatically be indexed here.",
                                        )
                                        .size(12.0)
                                        .color(Color32::from_rgb(154, 160, 166)),
                                    );
                                });
                            });
                    } else {
                        let mut delete_id = None;
                        for entry in &history_entries {
                            Frame::NONE
                                .fill(Color32::from_rgb(32, 33, 36))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let tag = if entry.url.contains("owasp") {
                                            "SEC"
                                        } else if entry.url.contains("github") {
                                            "GIT"
                                        } else if entry.url.contains("youtube") {
                                            "YT"
                                        } else {
                                            "WEB"
                                        };

                                        ui.label(
                                            RichText::new(format!("[{}]", tag))
                                                .size(11.0)
                                                .color(Color32::from_rgb(138, 180, 248))
                                                .strong(),
                                        );

                                        ui.vertical(|ui| {
                                            let title_btn = ui.link(
                                                RichText::new(&entry.title)
                                                    .size(14.0)
                                                    .strong()
                                                    .color(Color32::from_rgb(241, 243, 244)),
                                            );
                                            if title_btn.clicked() {
                                                *action = Some(BrowserAction::Navigate(
                                                    entry.url.clone(),
                                                ));
                                            }

                                            ui.label(
                                                RichText::new(&entry.url)
                                                    .size(11.5)
                                                    .color(Color32::from_rgb(138, 180, 248)),
                                            );
                                        });

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button("x").clicked() {
                                                    delete_id = Some(entry.id);
                                                }

                                                if entry.visit_count > 1 {
                                                    Frame::NONE
                                                        .fill(Color32::from_rgb(40, 42, 45))
                                                        .stroke(Stroke::new(
                                                            1.0_f32,
                                                            Color32::from_rgb(60, 64, 67),
                                                        ))
                                                        .corner_radius(egui::CornerRadius::same(4))
                                                        .inner_margin(Margin::symmetric(6, 2))
                                                        .show(ui, |ui| {
                                                            ui.label(
                                                                RichText::new(format!(
                                                                    "{} visits",
                                                                    entry.visit_count
                                                                ))
                                                                .size(10.5)
                                                                .color(Color32::from_rgb(
                                                                    154, 160, 166,
                                                                )),
                                                            );
                                                        });
                                                }

                                                let time_str =
                                                    format_unix_timestamp(entry.visit_time);
                                                ui.label(
                                                    RichText::new(time_str)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(154, 160, 166)),
                                                );
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }

                        if let Some(id) = delete_id {
                            let _ = storage.delete_browser_history_entry(id);
                        }
                    }
                });
                ui.add_space(side_margin);
            });
            ui.add_space(32.0);
        });
    }

    fn render_bookmarks_page(
        &mut self,
        ui: &mut Ui,
        storage: &excalibur_core::WorkspaceStorage,
        action: &mut Option<BrowserAction>,
    ) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(20.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(940.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    ui.horizontal(|ui| {
                        Frame::NONE
                            .fill(Color32::from_rgb(40, 42, 45))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("BOOKMARKS")
                                        .size(11.5)
                                        .color(Color32::from_rgb(138, 180, 248))
                                        .strong(),
                                );
                            });

                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Bookmarks & Favorites")
                                .size(22.0)
                                .color(Color32::from_rgb(241, 243, 244))
                                .strong(),
                        );
                    });

                    ui.add_space(16.0);

                    // Add Bookmark inline card
                    Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(16, 12))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Add New Bookmark")
                                    .size(13.0)
                                    .strong()
                                    .color(Color32::from_rgb(241, 243, 244)),
                            );
                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                ui.label("Title:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_bookmark_title)
                                        .hint_text("Page Name")
                                        .desired_width(180.0),
                                );

                                ui.label("URL:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_bookmark_url)
                                        .hint_text("https://example.com")
                                        .desired_width(260.0),
                                );

                                ui.label("Folder:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_bookmark_folder)
                                        .hint_text("Favorites")
                                        .desired_width(120.0),
                                );

                                if ui
                                    .button(
                                        RichText::new("+ Save")
                                            .strong()
                                            .color(Color32::from_rgb(138, 180, 248)),
                                    )
                                    .clicked()
                                {
                                    let url = self.new_bookmark_url.trim();
                                    if !url.is_empty() {
                                        let title = if self.new_bookmark_title.trim().is_empty() {
                                            url
                                        } else {
                                            self.new_bookmark_title.trim()
                                        };
                                        let folder = if self.new_bookmark_folder.trim().is_empty() {
                                            "Favorites"
                                        } else {
                                            self.new_bookmark_folder.trim()
                                        };
                                        let _ = storage.add_browser_bookmark(url, title, folder);
                                        self.new_bookmark_url.clear();
                                        self.new_bookmark_title.clear();
                                    }
                                }
                            });
                        });

                    ui.add_space(16.0);

                    // List bookmarks
                    let bookmarks = storage.list_browser_bookmarks().unwrap_or_default();
                    let mut remove_url = None;

                    for bm in &bookmarks {
                        Frame::NONE
                            .fill(Color32::from_rgb(32, 33, 36))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    Frame::NONE
                                        .fill(Color32::from_rgb(40, 42, 45))
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .inner_margin(Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(&bm.folder)
                                                    .size(10.0)
                                                    .color(Color32::from_rgb(138, 180, 248))
                                                    .strong(),
                                            );
                                        });

                                    ui.vertical(|ui| {
                                        let title_btn = ui.link(
                                            RichText::new(&bm.title)
                                                .size(14.0)
                                                .strong()
                                                .color(Color32::from_rgb(241, 243, 244)),
                                        );
                                        if title_btn.clicked() {
                                            *action = Some(BrowserAction::Navigate(bm.url.clone()));
                                        }

                                        ui.label(
                                            RichText::new(&bm.url)
                                                .size(11.5)
                                                .color(Color32::from_rgb(138, 180, 248)),
                                        );
                                    });

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("Delete").clicked() {
                                                remove_url = Some(bm.url.clone());
                                            }

                                            if ui.small_button("Copy").clicked() {
                                                ui.ctx().copy_text(bm.url.clone());
                                            }
                                        },
                                    );
                                });
                            });
                        ui.add_space(4.0);
                    }

                    if let Some(url) = remove_url {
                        let _ = storage.remove_browser_bookmark(&url);
                    }
                });
                ui.add_space(side_margin);
            });
            ui.add_space(32.0);
        });
    }

    fn render_settings_page(
        &mut self,
        ui: &mut Ui,
        storage: &excalibur_core::WorkspaceStorage,
        _action: &mut Option<BrowserAction>,
    ) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(20.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(940.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    ui.horizontal(|ui| {
                        Frame::NONE
                            .fill(Color32::from_rgb(40, 42, 45))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("SETTINGS")
                                        .size(11.5)
                                        .color(Color32::from_rgb(138, 180, 248))
                                        .strong(),
                                );
                            });

                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Browser Settings & System Preferences")
                                .size(22.0)
                                .color(Color32::from_rgb(241, 243, 244))
                                .strong(),
                        );
                    });

                    ui.add_space(20.0);

                    // 1. Search Engine Preference
                    Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Default Search Engine")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(241, 243, 244)),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Select the primary query provider used when entering non-URL terms in the omnibox.",
                                )
                                .size(12.0)
                                .color(Color32::from_rgb(154, 160, 166)),
                            );
                            ui.add_space(10.0);

                            let current_engine = storage
                                .get_browser_setting("search_engine")
                                .unwrap_or_default()
                                .unwrap_or_else(|| "Google".to_string());

                            let engines = ["Google", "DuckDuckGo", "SearXNG", "Brave", "YouTube"];
                            ui.horizontal_wrapped(|ui| {
                                for eng in engines {
                                    let is_active = current_engine == eng;
                                    if ui
                                        .selectable_label(is_active, RichText::new(eng).strong())
                                        .clicked()
                                    {
                                        let _ = storage.set_browser_setting("search_engine", eng);
                                    }
                                }
                            });
                        });

                    ui.add_space(14.0);

                    // 2. User-Agent Identity & Stealth Header
                    Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("User-Agent Header & Browser Identity")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(241, 243, 244)),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Configure client header identification sent during HTTP/1.1 and HTTP/2 handshakes.",
                                )
                                .size(12.0)
                                .color(Color32::from_rgb(154, 160, 166)),
                            );
                            ui.add_space(10.0);

                            let current_ua = storage
                                .get_browser_setting("user_agent_preset")
                                .unwrap_or_default()
                                .unwrap_or_else(|| "Excalibur".to_string());

                            let presets = [
                                ("Excalibur", "Excalibur/1.0 (Security Engine; Non-Chromium)"),
                                ("Chrome", "Chrome 134 on Windows 11 (Blink Spoof)"),
                                ("Firefox", "Firefox 136 on Linux (Gecko Spoof)"),
                                ("Safari", "Safari 18 on macOS (WebKit Spoof)"),
                                ("Curl", "curl/8.8.0 (Security Scanner Emulation)"),
                            ];

                            for (key, label) in presets {
                                let is_active = current_ua == key;
                                if ui
                                    .selectable_label(is_active, RichText::new(label))
                                    .clicked()
                                {
                                    let _ = storage.set_browser_setting("user_agent_preset", key);
                                }
                            }
                        });

                    ui.add_space(14.0);

                    // 3. Tab Layout Mode
                    Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Tab Strip Layout")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(241, 243, 244)),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Choose between standard horizontal top tabs or the modern vertical Zen sidebar.",
                                )
                                .size(12.0)
                                .color(Color32::from_rgb(154, 160, 166)),
                            );
                            ui.add_space(10.0);

                            let current_layout = storage
                                .get_browser_setting("tab_layout")
                                .unwrap_or_default()
                                .unwrap_or_else(|| "top".to_string());

                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        current_layout == "top",
                                        "Top Tab Strip (Chrome Standard)",
                                    )
                                    .clicked()
                                {
                                    let _ = storage.set_browser_setting("tab_layout", "top");
                                }
                                if ui
                                    .selectable_label(
                                        current_layout == "sidebar",
                                        "Zen Vertical Sidebar",
                                    )
                                    .clicked()
                                {
                                    let _ = storage.set_browser_setting("tab_layout", "sidebar");
                                }
                            });
                        });

                    ui.add_space(14.0);

                    // 4. Default Content Zoom
                    Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Default Page Zoom")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(241, 243, 244)),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Adjust default content rendering scale factor across tabs (fine-tune per tab with Ctrl++ and Ctrl+-).",
                                )
                                .size(12.0)
                                .color(Color32::from_rgb(154, 160, 166)),
                            );
                            ui.add_space(10.0);

                            let zoom_str = storage
                                .get_browser_setting("default_zoom")
                                .unwrap_or_default()
                                .unwrap_or_else(|| "100%".to_string());

                            let zoom_levels = ["80%", "90%", "100%", "110%", "125%", "150%"];
                            ui.horizontal(|ui| {
                                for z in zoom_levels {
                                    let is_active = zoom_str == z;
                                    if ui.selectable_label(is_active, z).clicked() {
                                        let _ = storage.set_browser_setting("default_zoom", z);
                                    }
                                }
                            });
                        });
                });
                ui.add_space(side_margin);
            });
            ui.add_space(32.0);
        });
    }

    fn render_about_page(
        &mut self,
        ui: &mut Ui,
        storage: &excalibur_core::WorkspaceStorage,
        _action: &mut Option<BrowserAction>,
    ) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(20.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(940.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Excalibur")
                                .size(40.0)
                                .strong()
                                .color(Color32::from_rgb(241, 243, 244)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Version 0.1.0 (Chrome Dark Minimalist Architecture)")
                                .size(13.5)
                                .color(Color32::from_rgb(154, 160, 166)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Independent Security Browser & Penetration Testing Workbench",
                            )
                            .size(13.0)
                            .color(Color32::from_rgb(154, 160, 166)),
                        );
                    });

                    ui.add_space(24.0);

                    // Core Pillars Card
                    Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Engine Architecture & Guarantees")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(241, 243, 244)),
                            );
                            ui.add_space(8.0);

                            let pillars = [
                                (
                                    "NON-CHROMIUM",
                                    "Zero dependencies on Google Chromium, CEF, WebKit, or Blink runtimes.",
                                    Color32::from_rgb(138, 180, 248),
                                ),
                                (
                                    "PURE RUST DOM",
                                    "Custom semantic DOM parser and CSS style cascade resolver in safe Rust.",
                                    Color32::from_rgb(52, 168, 83),
                                ),
                                (
                                    "GPU ACCELERATED",
                                    "Direct hardware-accelerated rendering using egui 0.31 over wgpu/glow.",
                                    Color32::from_rgb(251, 188, 4),
                                ),
                                (
                                    "SECURITY RADAR",
                                    "Embedded QuickJS client sink interception and RFC security header auditing.",
                                    Color32::from_rgb(234, 67, 53),
                                ),
                                (
                                    "MITM PROXY",
                                    "Integrated local proxy daemon on 127.0.0.1:8080 with on-the-fly CA generation.",
                                    Color32::from_rgb(138, 180, 248),
                                ),
                            ];

                            for (badge, desc, color) in pillars {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("[ {} ]", badge))
                                            .size(10.5)
                                            .strong()
                                            .color(color),
                                    );
                                    ui.label(
                                        RichText::new(desc)
                                            .size(12.0)
                                            .color(Color32::from_rgb(218, 220, 224)),
                                    );
                                });
                                ui.add_space(4.0);
                            }
                        });

                    ui.add_space(16.0);

                    // Live Diagnostics Card
                    Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("System Telemetry")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(241, 243, 244)),
                            );
                            ui.add_space(8.0);

                            let history_count = storage
                                .list_browser_history(1, 0, None)
                                .map(|h| h.len())
                                .unwrap_or(0);
                            let bookmark_count = storage
                                .list_browser_bookmarks()
                                .map(|b| b.len())
                                .unwrap_or(0);

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Database Storage:")
                                        .color(Color32::from_rgb(154, 160, 166)),
                                );
                                ui.label(
                                    RichText::new("SQLite 3 with WAL & FTS5 (High-Performance)")
                                        .color(Color32::from_rgb(138, 180, 248)),
                                );
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Saved Bookmarks:")
                                        .color(Color32::from_rgb(154, 160, 166)),
                                );
                                ui.label(
                                    RichText::new(format!("{} entries", bookmark_count))
                                        .color(Color32::from_rgb(52, 168, 83)),
                                );
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Recent History Log:")
                                        .color(Color32::from_rgb(154, 160, 166)),
                                );
                                ui.label(
                                    RichText::new(format!("{} items loaded", history_count))
                                        .color(Color32::from_rgb(251, 188, 4)),
                                );
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Repository:")
                                        .color(Color32::from_rgb(154, 160, 166)),
                                );
                                ui.label(
                                    RichText::new("https://github.com/Zeo120/Excalibur")
                                        .color(Color32::from_rgb(138, 180, 248)),
                                );
                            });
                        });
                });
                ui.add_space(side_margin);
            });
            ui.add_space(32.0);
        });
    }

    fn render_node(
        &mut self,
        ui: &mut Ui,
        node: &DomNode,
        action: &mut Option<BrowserAction>,
        current_url: &str,
        stylesheet: &StyleSheet,
    ) {
        match &node.node_type {
            NodeType::Document => {
                for child in &node.children {
                    self.render_node(ui, child, action, current_url, stylesheet);
                }
            }
            NodeType::Element { tag, attributes } => {
                let tag_lower = tag.to_ascii_lowercase();

                // Skip non-rendering metadata elements
                if tag_lower == "head"
                    || tag_lower == "style"
                    || tag_lower == "script"
                    || tag_lower == "meta"
                    || tag_lower == "link"
                    || tag_lower == "title"
                    || tag_lower == "noscript"
                {
                    return;
                }

                let style = StyleResolver::resolve(node, stylesheet);
                if style.display_none {
                    return;
                }

                if let Some(mt) = style.margin_top {
                    if mt > 0.0 {
                        ui.add_space(mt.clamp(0.0, 40.0));
                    }
                }

                match tag_lower.as_str() {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let (default_size, default_color) = match tag_lower.as_str() {
                                "h1" => (24.0, Color32::from_rgb(88, 166, 255)),
                                "h2" => (20.0, Color32::from_rgb(121, 192, 255)),
                                "h3" => (16.0, Color32::from_rgb(201, 209, 217)),
                                "h4" => (14.0, Color32::from_rgb(201, 209, 217)),
                                _ => (13.0, Color32::from_rgb(139, 148, 158)),
                            };

                            let font_size = style.font_size.unwrap_or(default_size);
                            let text_color =
                                style.color.map(to_egui_color).unwrap_or(default_color);

                            let mut rich = RichText::new(trimmed).size(font_size).color(text_color);
                            if style.font_weight_bold.unwrap_or(true) {
                                rich = rich.strong();
                            }
                            if style.font_style_italic.unwrap_or(false) {
                                rich = rich.italics();
                            }
                            if style.text_decoration_underline.unwrap_or(false) {
                                rich = rich.underline();
                            }

                            if let Some(bg) = style.background_color {
                                let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 20.0) as u8;
                                Frame::NONE
                                    .fill(to_egui_color(bg))
                                    .inner_margin(Margin::symmetric(8, 4))
                                    .corner_radius(egui::CornerRadius::same(rad))
                                    .show(ui, |ui| {
                                        ui.label(rich);
                                    });
                            } else {
                                ui.label(rich);
                            }
                            ui.add_space(4.0);
                        }
                    }
                    "p" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let font_size = style.font_size.unwrap_or(14.0);
                            let text_color = style
                                .color
                                .map(to_egui_color)
                                .unwrap_or(Color32::from_rgb(201, 209, 217));

                            let mut rich = RichText::new(trimmed).size(font_size).color(text_color);
                            if style.font_weight_bold.unwrap_or(false) {
                                rich = rich.strong();
                            }
                            if style.font_style_italic.unwrap_or(false) {
                                rich = rich.italics();
                            }
                            if style.text_decoration_underline.unwrap_or(false) {
                                rich = rich.underline();
                            }

                            if let Some(bg) = style.background_color {
                                let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 20.0) as u8;
                                Frame::NONE
                                    .fill(to_egui_color(bg))
                                    .inner_margin(Margin::same(6))
                                    .corner_radius(egui::CornerRadius::same(rad))
                                    .show(ui, |ui| {
                                        ui.label(rich);
                                    });
                            } else {
                                ui.label(rich);
                            }
                            ui.add_space(4.0);
                        }
                    }
                    "a" => {
                        let href = attributes.get("href").map(|s| s.as_str()).unwrap_or("");
                        let text = node.text_content();
                        let display_text = if text.trim().is_empty() {
                            href
                        } else {
                            text.trim()
                        };

                        let font_size = style.font_size.unwrap_or(14.0);
                        let text_color = style
                            .color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(88, 166, 255));

                        let mut rich = RichText::new(display_text)
                            .size(font_size)
                            .color(text_color);
                        if style.text_decoration_underline.unwrap_or(true) {
                            rich = rich.underline();
                        }
                        if style.font_weight_bold.unwrap_or(false) {
                            rich = rich.strong();
                        }

                        if ui.link(rich).clicked() && !href.is_empty() {
                            let target_url = self.resolve_url(current_url, href);
                            *action = Some(BrowserAction::Navigate(target_url));
                        }
                    }
                    "span" => {
                        let has_child_elements = node
                            .children
                            .iter()
                            .any(|c| matches!(c.node_type, NodeType::Element { .. }));
                        if !has_child_elements {
                            let text = node.text_content();
                            let trimmed = text.trim();
                            if !trimmed.is_empty() {
                                let font_size = style.font_size.unwrap_or(14.0);
                                let text_color = style
                                    .color
                                    .map(to_egui_color)
                                    .unwrap_or(Color32::from_rgb(201, 209, 217));

                                let mut rich =
                                    RichText::new(trimmed).size(font_size).color(text_color);
                                if style.font_weight_bold.unwrap_or(false) {
                                    rich = rich.strong();
                                }
                                if style.font_style_italic.unwrap_or(false) {
                                    rich = rich.italics();
                                }
                                if style.text_decoration_underline.unwrap_or(false) {
                                    rich = rich.underline();
                                }
                                ui.label(rich);
                            }
                        } else {
                            for child in &node.children {
                                self.render_node(ui, child, action, current_url, stylesheet);
                            }
                        }
                    }
                    "strong" | "b" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let color = style.color.map(to_egui_color).unwrap_or(Color32::WHITE);
                            ui.label(RichText::new(trimmed).strong().color(color));
                        }
                    }
                    "em" | "i" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let color = style
                                .color
                                .map(to_egui_color)
                                .unwrap_or(Color32::from_rgb(201, 209, 217));
                            ui.label(RichText::new(trimmed).italics().color(color));
                        }
                    }
                    "br" => {
                        ui.add_space(4.0);
                    }
                    "button" => {
                        let text = node.text_content();
                        let btn_text = if text.trim().is_empty() {
                            "Submit"
                        } else {
                            text.trim()
                        };

                        let text_color = style.color.map(to_egui_color).unwrap_or(Color32::WHITE);
                        let font_size = style.font_size.unwrap_or(13.0);
                        let rich = RichText::new(btn_text).size(font_size).color(text_color);

                        if ui.button(rich).clicked() {
                            tracing::info!("Button clicked: {}", btn_text);
                        }
                    }
                    "img" => {
                        let src = attributes
                            .get("src")
                            .or_else(|| attributes.get("data-src"))
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        if !src.is_empty() {
                            let resolved = self.resolve_url(current_url, src);
                            let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 20.0);
                            ui.add_space(4.0);
                            if let Some(tex) =
                                self.thumbnail_cache.get_or_request(ui.ctx(), &resolved)
                            {
                                ui.add(
                                    egui::Image::from_texture(tex)
                                        .max_width(ui.available_width().min(720.0))
                                        .corner_radius(rad),
                                );
                            } else {
                                Frame::NONE
                                    .fill(Color32::from_rgb(15, 20, 31))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59)))
                                    .corner_radius(egui::CornerRadius::same(rad as u8))
                                    .inner_margin(Margin::symmetric(14, 10))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("[Image Loading...]")
                                                .size(11.0)
                                                .color(Color32::from_rgb(100, 116, 139)),
                                        );
                                    });
                            }
                            ui.add_space(4.0);
                        }
                    }
                    "video" | "audio" => {
                        let is_audio = tag == "audio";
                        let src = attributes
                            .get("src")
                            .or_else(|| attributes.get("data-src"))
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        let poster = attributes.get("poster").map(|s| s.as_str()).unwrap_or("");
                        let resolved_src = if !src.is_empty() {
                            self.resolve_url(current_url, src)
                        } else {
                            let mut s = String::new();
                            for child in &node.children {
                                if let NodeType::Element { tag, attributes } = &child.node_type {
                                    if tag == "source" {
                                        if let Some(c_src) = attributes.get("src") {
                                            s = self.resolve_url(current_url, c_src);
                                            break;
                                        }
                                    }
                                }
                            }
                            s
                        };

                        if !resolved_src.is_empty() {
                            self.audio_stream_loader
                                .request_stream(&resolved_src, &resolved_src);
                        }
                        self.audio_stream_loader
                            .poll_incoming(&mut self.audio_engine);

                        ui.add_space(6.0);
                        let player_w = ui.available_width().min(840.0);
                        let player_h = if is_audio {
                            60.0
                        } else {
                            (player_w * 9.0 / 16.0).clamp(200.0, 460.0)
                        };

                        Frame::NONE
                            .fill(Color32::from_rgb(32, 33, 36))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    if !is_audio {
                                        let (rect, resp) = ui.allocate_exact_size(
                                            egui::vec2(player_w - 20.0, player_h - 50.0),
                                            egui::Sense::click(),
                                        );
                                        if resp.clicked() {
                                            self.player_state.is_playing =
                                                !self.player_state.is_playing;
                                            if self.player_state.is_playing {
                                                self.audio_engine
                                                    .ensure_playback(&resolved_src, true);
                                                self.audio_engine.play();
                                            } else {
                                                self.audio_engine.pause();
                                            }
                                        }

                                        let painter = ui.painter_at(rect);
                                        painter.rect_filled(rect, 6.0, Color32::from_rgb(0, 0, 0));

                                        if !poster.is_empty() {
                                            let resolved_poster =
                                                self.resolve_url(current_url, poster);
                                            if let Some(tex) = self
                                                .thumbnail_cache
                                                .get_or_request(ui.ctx(), &resolved_poster)
                                            {
                                                let uv = egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(1.0, 1.0),
                                                );
                                                painter.image(tex.id(), rect, uv, Color32::WHITE);
                                            }
                                        }

                                        let center = rect.center();
                                        let sym = if self.player_state.is_playing {
                                            "||"
                                        } else {
                                            ">"
                                        };
                                        painter.circle_filled(
                                            center,
                                            24.0,
                                            Color32::from_rgba_unmultiplied(40, 42, 45, 220),
                                        );
                                        painter.text(
                                            center,
                                            egui::Align2::CENTER_CENTER,
                                            sym,
                                            egui::FontId::proportional(20.0),
                                            Color32::from_rgb(241, 243, 244),
                                        );
                                    }

                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        let play_label = if self.player_state.is_playing {
                                            "Pause"
                                        } else {
                                            "Play"
                                        };
                                        if ui
                                            .button(
                                                RichText::new(play_label)
                                                    .color(Color32::from_rgb(138, 180, 248))
                                                    .strong(),
                                            )
                                            .clicked()
                                        {
                                            self.player_state.is_playing =
                                                !self.player_state.is_playing;
                                            if self.player_state.is_playing {
                                                self.audio_engine
                                                    .ensure_playback(&resolved_src, true);
                                                self.audio_engine.play();
                                            } else {
                                                self.audio_engine.pause();
                                            }
                                        }

                                        let mode_desc = if is_audio {
                                            "HTML5 Audio"
                                        } else {
                                            "HTML5 Video"
                                        };
                                        ui.label(
                                            RichText::new(mode_desc)
                                                .size(11.0)
                                                .color(Color32::from_rgb(154, 160, 166)),
                                        );

                                        // Volume control
                                        let vol_label = if self.player_state.is_muted {
                                            "Muted"
                                        } else {
                                            "Vol"
                                        };
                                        if ui.small_button(vol_label).clicked() {
                                            self.player_state.is_muted =
                                                !self.player_state.is_muted;
                                            self.audio_engine.set_muted(self.player_state.is_muted);
                                        }

                                        let vol_slider = ui.add_sized(
                                            [60.0, 18.0],
                                            egui::Slider::new(
                                                &mut self.player_state.volume,
                                                0.0..=1.0,
                                            )
                                            .show_value(false),
                                        );
                                        if vol_slider.changed() {
                                            if self.player_state.is_muted
                                                && self.player_state.volume > 0.0
                                            {
                                                self.player_state.is_muted = false;
                                            }
                                            self.audio_engine.set_volume(self.player_state.volume);
                                            self.audio_engine.set_muted(self.player_state.is_muted);
                                        }

                                        if !resolved_src.is_empty()
                                            && ui.small_button("Copy Stream URL").clicked()
                                        {
                                            ui.ctx().copy_text(resolved_src.clone());
                                        }
                                    });
                                });
                            });
                        ui.add_space(6.0);
                    }
                    "form" => {
                        let bg_col = style
                            .background_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(22, 27, 34));
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(48, 54, 61));
                        let border_w = style.border_width.unwrap_or(1.0);
                        let rad = style.border_radius.unwrap_or(6.0).clamp(0.0, 20.0) as u8;

                        Frame::NONE
                            .fill(bg_col)
                            .stroke(Stroke::new(border_w, border_col))
                            .corner_radius(egui::CornerRadius::same(rad))
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    for child in &node.children {
                                        self.render_node(
                                            ui,
                                            child,
                                            action,
                                            current_url,
                                            stylesheet,
                                        );
                                    }
                                });
                            });
                        ui.add_space(8.0);
                    }
                    "input" => {
                        let input_type =
                            attributes.get("type").map(|s| s.as_str()).unwrap_or("text");
                        let input_name = attributes
                            .get("name")
                            .or_else(|| attributes.get("id"))
                            .map(|s| s.as_str())
                            .unwrap_or("field");
                        let default_val = attributes.get("value").map(|s| s.as_str()).unwrap_or("");

                        let current_val = self
                            .input_states
                            .entry(input_name.to_string())
                            .or_insert_with(|| default_val.to_string());

                        if input_type == "submit" {
                            let label = if current_val.is_empty() {
                                "Submit"
                            } else {
                                current_val.as_str()
                            };
                            if ui.button(label).clicked() {
                                tracing::info!("Submit input clicked: {}", input_name);
                            }
                        } else if input_type == "password" {
                            ui.horizontal(|ui| {
                                let placeholder = attributes
                                    .get("placeholder")
                                    .or_else(|| attributes.get("aria-label"))
                                    .map(|s| s.as_str())
                                    .unwrap_or("Password");
                                ui.add(
                                    egui::TextEdit::singleline(current_val)
                                        .password(true)
                                        .hint_text(placeholder)
                                        .desired_width(ui.available_width().min(320.0)),
                                );
                            });
                        } else {
                            ui.horizontal(|ui| {
                                let placeholder = attributes
                                    .get("placeholder")
                                    .or_else(|| attributes.get("aria-label"))
                                    .or_else(|| attributes.get("title"))
                                    .map(|s| s.as_str())
                                    .unwrap_or_else(|| {
                                        if input_type == "search"
                                            || input_name == "search_query"
                                            || input_name == "q"
                                        {
                                            "Search or enter query..."
                                        } else {
                                            "Enter text..."
                                        }
                                    });

                                let te = ui.add(
                                    egui::TextEdit::singleline(current_val)
                                        .hint_text(placeholder)
                                        .desired_width(ui.available_width().min(450.0)),
                                );

                                let is_search_field = input_name == "q"
                                    || input_name == "search_query"
                                    || input_name == "query"
                                    || input_type == "search";

                                if is_search_field
                                    && (ui.button("Search").clicked()
                                        || (te.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                                    && !current_val.trim().is_empty()
                                {
                                    let query = current_val.trim().replace(' ', "+");
                                    let search_target = if current_url.contains("youtube.com") {
                                        format!("https://www.youtube.com/results?search_query={}", query)
                                    } else if current_url.contains("wikipedia.org") {
                                        format!("https://en.wikipedia.org/wiki/Special:Search?search={}", query)
                                    } else if current_url.contains("duckduckgo.com") {
                                        format!("https://duckduckgo.com/?q={}", query)
                                    } else {
                                        format!("https://www.google.com/search?q={}", query)
                                    };
                                    *action = Some(BrowserAction::Navigate(search_target));
                                }
                            });
                        }
                    }
                    "hr" => {
                        ui.separator();
                    }
                    "pre" | "code" => {
                        let code_text = node.text_content();
                        let bg_col = style
                            .background_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(13, 17, 23));
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(48, 54, 61));
                        let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 16.0) as u8;

                        Frame::NONE
                            .fill(bg_col)
                            .stroke(Stroke::new(1.0_f32, border_col))
                            .corner_radius(egui::CornerRadius::same(rad))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                if let Some(tc) = style.color {
                                    ui.label(
                                        RichText::new(code_text.trim())
                                            .monospace()
                                            .color(to_egui_color(tc)),
                                    );
                                } else {
                                    ui.code(code_text.trim());
                                }
                            });
                    }
                    "blockquote" => {
                        let text = node.text_content();
                        let bg_col = style
                            .background_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(22, 27, 34));
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(88, 166, 255));
                        let text_col = style
                            .color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(201, 209, 217));
                        let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 16.0) as u8;

                        Frame::NONE
                            .fill(bg_col)
                            .stroke(Stroke::new(2.0_f32, border_col))
                            .corner_radius(egui::CornerRadius::same(rad))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                ui.label(RichText::new(text.trim()).italics().color(text_col));
                            });
                    }
                    "ul" | "ol" => {
                        ui.vertical(|ui| {
                            for child in &node.children {
                                if child.tag() == Some("li") {
                                    let item_style = StyleResolver::resolve(child, stylesheet);
                                    let item_text = child.text_content();
                                    let item_color = item_style
                                        .color
                                        .or(style.color)
                                        .map(to_egui_color)
                                        .unwrap_or(Color32::from_rgb(201, 209, 217));
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("-").color(item_color));
                                        ui.label(RichText::new(item_text.trim()).color(item_color));
                                    });
                                }
                            }
                        });
                    }
                    "table" => {
                        let bg_col = style
                            .background_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::TRANSPARENT);
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(48, 54, 61));
                        let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 16.0) as u8;

                        Frame::NONE
                            .fill(bg_col)
                            .stroke(Stroke::new(1.0_f32, border_col))
                            .corner_radius(egui::CornerRadius::same(rad))
                            .inner_margin(Margin::same(6))
                            .show(ui, |ui| {
                                for child in &node.children {
                                    self.render_node(ui, child, action, current_url, stylesheet);
                                }
                            });
                    }
                    "tr" => {
                        ui.horizontal(|ui| {
                            for child in &node.children {
                                let cell_tag = child.tag().unwrap_or("");
                                if cell_tag == "td" || cell_tag == "th" {
                                    let cell_style = StyleResolver::resolve(child, stylesheet);
                                    let is_header = cell_tag == "th";
                                    let text = child.text_content();

                                    let cell_color = cell_style
                                        .color
                                        .or(style.color)
                                        .map(to_egui_color)
                                        .unwrap_or(if is_header {
                                            Color32::WHITE
                                        } else {
                                            Color32::from_rgb(201, 209, 217)
                                        });
                                    let mut rich = RichText::new(text.trim()).color(cell_color);
                                    if is_header || cell_style.font_weight_bold.unwrap_or(false) {
                                        rich = rich.strong();
                                    }

                                    if let Some(bg) = cell_style.background_color {
                                        Frame::NONE
                                            .fill(to_egui_color(bg))
                                            .inner_margin(Margin::same(4))
                                            .show(ui, |ui| {
                                                ui.label(rich);
                                            });
                                    } else {
                                        ui.label(rich);
                                    }
                                    ui.separator();
                                }
                            }
                        });
                    }
                    _ => {
                        // Standard containers: div, section, article, main, header, footer, body, nav, etc.
                        let has_frame = style.background_color.is_some()
                            || style.border_color.is_some()
                            || style.padding.is_some();
                        if has_frame {
                            let mut frame = Frame::NONE;
                            if let Some(bg) = style.background_color {
                                frame = frame.fill(to_egui_color(bg));
                            }
                            if let Some(bc) = style.border_color {
                                let bw = style.border_width.unwrap_or(1.0);
                                frame = frame.stroke(Stroke::new(bw, to_egui_color(bc)));
                            }
                            if let Some(pad) = style.padding {
                                frame =
                                    frame.inner_margin(Margin::same(pad.clamp(0.0, 30.0) as i8));
                            }
                            if let Some(rad) = style.border_radius {
                                frame = frame.corner_radius(egui::CornerRadius::same(
                                    rad.clamp(0.0, 24.0) as u8,
                                ));
                            }

                            frame.show(ui, |ui| {
                                ui.vertical(|ui| {
                                    for child in &node.children {
                                        self.render_node(
                                            ui,
                                            child,
                                            action,
                                            current_url,
                                            stylesheet,
                                        );
                                    }
                                });
                            });
                        } else {
                            for child in &node.children {
                                self.render_node(ui, child, action, current_url, stylesheet);
                            }
                        }
                    }
                }

                if let Some(mb) = style.margin_bottom {
                    if mb > 0.0 {
                        ui.add_space(mb.clamp(0.0, 40.0));
                    }
                }
            }
            NodeType::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let font_size = 13.0 * self.zoom.clamp(0.6, 2.5);
                    if !self.find_query.trim().is_empty()
                        && trimmed
                            .to_lowercase()
                            .contains(&self.find_query.trim().to_lowercase())
                    {
                        Frame::NONE
                            .fill(Color32::from_rgb(250, 204, 21))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(254, 240, 138)))
                            .corner_radius(egui::CornerRadius::same(3))
                            .inner_margin(Margin::symmetric(4, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(trimmed)
                                        .size(font_size)
                                        .color(Color32::from_rgb(10, 15, 26))
                                        .strong(),
                                );
                            });
                    } else {
                        ui.label(RichText::new(trimmed).size(font_size));
                    }
                }
            }
            NodeType::Comment(_) => {}
        }
    }

    fn resolve_url(&self, base_url: &str, href: &str) -> String {
        if href.starts_with("http://") || href.starts_with("https://") {
            return href.to_string();
        }

        if let Ok(base) = url::Url::parse(base_url) {
            if let Ok(resolved) = base.join(href) {
                return resolved.to_string();
            }
        }

        href.to_string()
    }

    fn render_youtube_hub(
        &mut self,
        ui: &mut Ui,
        url: &str,
        watch_details: Option<&YouTubeWatchDetails>,
        videos: &[YouTubeVideoItem],
        action: &mut Option<BrowserAction>,
    ) {
        let query_str = if let Some(pos) = url.find("search_query=") {
            let q_part = &url[pos + 13..];
            let end = q_part.find('&').unwrap_or(q_part.len());
            let raw_q = &q_part[..end];
            raw_q.replace('+', " ").replace("%20", " ")
        } else {
            String::new()
        };

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(12.0);
                let available_width = ui.available_width();
                let content_width = available_width.min(1080.0);
                let side_margin = ((available_width - content_width) / 2.0).max(16.0);

                ui.horizontal(|ui| {
                    ui.add_space(side_margin);
                    ui.vertical(|ui| {
                        ui.set_max_width(content_width);

                        // YouTube Top Navigation & Search Bar
                        Frame::NONE
                            .fill(Color32::from_rgb(32, 33, 36))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Brand Pill
                                    Frame::NONE
                                        .fill(Color32::from_rgb(255, 0, 0))
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .inner_margin(Margin::symmetric(8, 3))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("YOUTUBE")
                                                    .size(11.0)
                                                    .color(Color32::WHITE)
                                                    .strong(),
                                            );
                                        });

                                    ui.add_space(8.0);

                                    // YouTube In-Hub Search Box
                                    let mut search_target = None;
                                    let search_width = (ui.available_width() - 120.0).max(180.0);
                                    let yt_search_val = self
                                        .input_states
                                        .entry("yt_hub_search".to_string())
                                        .or_default();

                                    // If empty, auto-populate from URL query_str
                                    if yt_search_val.is_empty() && !query_str.is_empty() {
                                        *yt_search_val = query_str.clone();
                                    }

                                    let te = ui.add(
                                        egui::TextEdit::singleline(yt_search_val)
                                            .hint_text("Search YouTube videos, topics, channels...")
                                            .desired_width(search_width),
                                    );

                                    if (te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                        || ui.button(RichText::new("Search").color(Color32::from_rgb(138, 180, 248)).strong()).clicked()
                                    {
                                        let q = yt_search_val.trim();
                                        if !q.is_empty() {
                                            search_target = Some(format!(
                                                "https://www.youtube.com/results?search_query={}",
                                                q.replace(' ', "+")
                                            ));
                                        }
                                    }

                                    if let Some(target) = search_target {
                                        *action = Some(BrowserAction::Navigate(target));
                                    }
                                });

                                ui.add_space(8.0);

                                // Topic Pills
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                                    let topics = [
                                        "All",
                                        "Cybersecurity",
                                        "Rust Programming",
                                        "Computer Science",
                                        "Tech News",
                                        "Hardware",
                                        "Podcasts",
                                        "Gaming",
                                    ];
                                    for t in topics {
                                        let is_active = url.contains(&t.replace(' ', "+"));
                                        let (bg, border, fg) = if is_active {
                                            (Color32::from_rgb(241, 243, 244), Color32::from_rgb(241, 243, 244), Color32::from_rgb(18, 18, 18))
                                        } else {
                                            (Color32::from_rgb(40, 42, 45), Color32::from_rgb(60, 64, 67), Color32::from_rgb(218, 220, 224))
                                        };

                                        Frame::NONE
                                            .fill(bg)
                                            .stroke(Stroke::new(1.0_f32, border))
                                            .corner_radius(egui::CornerRadius::same(12))
                                            .inner_margin(Margin::symmetric(10, 4))
                                            .show(ui, |ui| {
                                                if ui.selectable_label(is_active, RichText::new(t).size(11.0).color(fg)).clicked() {
                                                    let target = if t == "All" {
                                                        "https://www.youtube.com".to_string()
                                                    } else {
                                                        format!("https://www.youtube.com/results?search_query={}", t.replace(' ', "+"))
                                                    };
                                                    *action = Some(BrowserAction::Navigate(target));
                                                }
                                            });
                                    }
                                });
                            });

                        ui.add_space(14.0);

                        // If on a Watch page, render the Video Player Card
                        if let Some(watch) = watch_details {
                            self.render_youtube_watch_player(ui, watch, action, videos);
                            ui.add_space(16.0);
                        }

                        // Video Cards Grid / Section (when not watching a single video)
                        if !videos.is_empty() && watch_details.is_none() {
                            let section_title = if url.contains("search_query") {
                                "Search Results"
                            } else {
                                "Trending & Featured Videos"
                            };

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(section_title)
                                        .size(17.0)
                                        .strong()
                                        .color(Color32::from_rgb(241, 243, 244)),
                                );
                                ui.label(
                                    RichText::new(format!("({} items)", videos.len()))
                                        .size(12.0)
                                        .color(Color32::from_rgb(154, 160, 166)),
                                );
                            });

                            ui.add_space(8.0);

                            // Render 2 balanced columns with fluid responsive width
                            ui.horizontal(|ui| {
                                let col_w = (content_width - 16.0) / 2.0;
                                ui.vertical(|col_ui| {
                                    col_ui.set_width(col_w);
                                    for video in videos.iter().step_by(2) {
                                        self.render_video_card(col_ui, video, action);
                                        col_ui.add_space(12.0);
                                    }
                                });
                                ui.add_space(12.0);
                                ui.vertical(|col_ui| {
                                    col_ui.set_width(col_w);
                                    for video in videos.iter().skip(1).step_by(2) {
                                        self.render_video_card(col_ui, video, action);
                                        col_ui.add_space(12.0);
                                    }
                                });
                            });
                        }

                        if videos.is_empty() && watch_details.is_none() {
                            ui.add_space(20.0);
                            Frame::NONE
                                .fill(Color32::from_rgb(32, 33, 36))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(Margin::symmetric(24, 20))
                                .show(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        let headline = if !query_str.is_empty() {
                                            format!("YouTube Search: \"{}\"", query_str)
                                        } else {
                                            "Explore YouTube on Excalibur".to_string()
                                        };
                                        ui.label(
                                            RichText::new(headline)
                                                .size(20.0)
                                                .strong()
                                                .color(Color32::from_rgb(241, 243, 244)),
                                        );
                                        ui.add_space(6.0);
                                        let subtitle = if !query_str.is_empty() {
                                            "No direct streams matched this exact query. Try refining your keywords or launch a quick topic below:".to_string()
                                        } else {
                                            "Search for any topic above or choose a category below to browse videos.".to_string()
                                        };
                                        ui.label(
                                            RichText::new(subtitle)
                                                .size(13.0)
                                                .color(Color32::from_rgb(154, 160, 166)),
                                        );
                                        ui.add_space(14.0);
                                        ui.horizontal(|ui| {
                                            let quick_searches = [
                                                "Rust Programming",
                                                "Cybersecurity Research",
                                                "Computer Science",
                                                "Tech News 2026",
                                            ];
                                            for qs in quick_searches {
                                                if ui
                                                    .button(
                                                        RichText::new(qs)
                                                            .size(12.0)
                                                            .color(Color32::from_rgb(138, 180, 248))
                                                            .strong(),
                                                    )
                                                    .clicked()
                                                {
                                                    *action = Some(BrowserAction::Navigate(format!(
                                                        "https://www.youtube.com/results?search_query={}",
                                                        qs.replace(' ', "+")
                                                    )));
                                                }
                                            }
                                        });
                                    });
                                });
                        }
                    });
                    ui.add_space(side_margin);
                });
                ui.add_space(32.0);
            });
    }

    fn render_video_card(
        &mut self,
        ui: &mut Ui,
        video: &YouTubeVideoItem,
        action: &mut Option<BrowserAction>,
    ) {
        Frame::NONE
            .fill(Color32::from_rgb(32, 33, 36))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 10))
            .show(ui, |ui| {
                // 16:9 Thumbnail or Minimalist Loading Placeholder
                let card_w = ui.available_width();
                let thumb_h = (card_w * 9.0 / 16.0).clamp(110.0, 180.0);

                let thumb_opt = if !video.thumbnail_url.is_empty() {
                    self.thumbnail_cache
                        .get_or_request(ui.ctx(), &video.thumbnail_url)
                } else {
                    None
                };

                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(card_w, thumb_h), egui::Sense::click());

                if resp.clicked() {
                    *action = Some(BrowserAction::Navigate(format!(
                        "https://www.youtube.com/watch?v={}",
                        video.id
                    )));
                }

                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 6.0, Color32::from_rgb(18, 18, 18));

                if let Some(tex) = thumb_opt {
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    painter.image(tex.id(), rect, uv, Color32::WHITE);
                } else {
                    let center = rect.center();
                    painter.rect_stroke(
                        rect,
                        6.0,
                        Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        "[ Play ]",
                        egui::FontId::proportional(14.0),
                        Color32::from_rgb(138, 180, 248),
                    );
                }

                if resp.hovered() {
                    painter.rect_filled(
                        rect,
                        6.0,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 12),
                    );
                }

                if !video.duration.is_empty() {
                    let tag_w = (video.duration.len() as f32 * 7.5 + 12.0).max(36.0);
                    let tag_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.max.x - tag_w - 6.0, rect.max.y - 20.0),
                        egui::pos2(rect.max.x - 6.0, rect.max.y - 4.0),
                    );
                    painter.rect_filled(
                        tag_rect,
                        3.0,
                        Color32::from_rgba_unmultiplied(0, 0, 0, 210),
                    );
                    painter.text(
                        tag_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &video.duration,
                        egui::FontId::monospace(10.0),
                        Color32::from_rgb(241, 243, 244),
                    );
                }

                ui.add_space(8.0);

                let title_text = if video.title.len() > 65 {
                    format!("{}...", &video.title[..62])
                } else {
                    video.title.clone()
                };

                let title_btn = ui.link(
                    RichText::new(title_text)
                        .size(13.0)
                        .strong()
                        .color(Color32::from_rgb(241, 243, 244)),
                );
                if title_btn.clicked() {
                    *action = Some(BrowserAction::Navigate(format!(
                        "https://www.youtube.com/watch?v={}",
                        video.id
                    )));
                }

                ui.add_space(3.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&video.channel)
                            .size(11.5)
                            .color(Color32::from_rgb(154, 160, 166))
                            .strong(),
                    );

                    if !video.views.is_empty() {
                        ui.label(
                            RichText::new(format!("- {}", video.views))
                                .size(11.0)
                                .color(Color32::from_rgb(154, 160, 166)),
                        );
                    }
                });

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new("Watch")
                                .size(11.0)
                                .color(Color32::from_rgb(138, 180, 248))
                                .strong(),
                        )
                        .clicked()
                    {
                        *action = Some(BrowserAction::Navigate(format!(
                            "https://www.youtube.com/watch?v={}",
                            video.id
                        )));
                    }

                    if ui
                        .small_button(
                            RichText::new("Copy Link")
                                .size(10.5)
                                .color(Color32::from_rgb(154, 160, 166)),
                        )
                        .clicked()
                    {
                        ui.ctx()
                            .copy_text(format!("https://www.youtube.com/watch?v={}", video.id));
                    }
                });
            });
    }

    fn render_youtube_watch_player(
        &mut self,
        ui: &mut Ui,
        watch: &YouTubeWatchDetails,
        action: &mut Option<BrowserAction>,
        videos: &[YouTubeVideoItem],
    ) {
        // Initialize player for newly opened video
        if self.player_state.active_video_id != watch.video_id {
            self.player_state.active_video_id = watch.video_id.clone();
            self.player_state.current_time = 0.0;
            self.player_state.is_playing = true;
            if let Some(matching) = videos.iter().find(|v| v.id == watch.video_id) {
                self.player_state.duration = parse_duration_seconds(&matching.duration);
            } else {
                self.player_state.duration = 214.0;
            }
            self.audio_engine.start_synth_track(&watch.video_id);
        }

        // Progress playback tick
        self.player_state.update_playback(ui.ctx());

        // Sync audio engine playback, volume, and incoming streams
        self.audio_stream_loader
            .poll_incoming(&mut self.audio_engine);
        self.audio_engine
            .ensure_playback(&watch.video_id, self.player_state.is_playing);
        self.audio_engine.set_volume(self.player_state.volume);
        self.audio_engine.set_muted(self.player_state.is_muted);
        self.audio_engine.set_speed(self.player_state.speed);

        let available_w = ui.available_width();
        let player_w = if self.player_state.theater_mode {
            available_w
        } else {
            available_w.min(940.0)
        };
        let screen_h = (player_w * 9.0 / 16.0).clamp(240.0, 530.0);

        Frame::NONE
            .fill(Color32::from_rgb(18, 18, 18))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.set_width(player_w);
                ui.vertical(|ui| {
                    // 1. 16:9 Cinema Viewport Screen (Pure Black)
                    let (screen_rect, screen_resp) = ui
                        .allocate_exact_size(egui::vec2(player_w, screen_h), egui::Sense::click());

                    if screen_resp.clicked() {
                        self.player_state.is_playing = !self.player_state.is_playing;
                        if self.player_state.is_playing {
                            self.audio_engine.play();
                        } else {
                            self.audio_engine.pause();
                        }
                    }

                    let painter = ui.painter_at(screen_rect);
                    painter.rect_filled(screen_rect, 8.0, Color32::from_rgb(0, 0, 0));

                    // Thumbnail / Poster Frame
                    let thumb_url =
                        format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", watch.video_id);
                    if let Some(tex) = self.thumbnail_cache.get_or_request(ui.ctx(), &thumb_url) {
                        let uv =
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        painter.image(tex.id(), screen_rect, uv, Color32::WHITE);
                    }

                    // Top vignette gradient
                    let top_vignette = egui::Rect::from_min_max(
                        screen_rect.min,
                        egui::pos2(screen_rect.max.x, screen_rect.min.y + 44.0),
                    );
                    painter.rect_filled(
                        top_vignette,
                        0.0,
                        Color32::from_rgba_unmultiplied(0, 0, 0, 160),
                    );

                    // Top Overlay: Status & Resolution Badge
                    let (status_text, status_col) = if self.player_state.is_playing {
                        ("[LIVE STREAMING]", Color32::from_rgb(234, 67, 53))
                    } else {
                        ("[PAUSED]", Color32::from_rgb(154, 160, 166))
                    };
                    painter.text(
                        egui::pos2(screen_rect.min.x + 12.0, screen_rect.min.y + 12.0),
                        egui::Align2::LEFT_TOP,
                        status_text,
                        egui::FontId::proportional(11.0),
                        status_col,
                    );
                    painter.text(
                        egui::pos2(screen_rect.max.x - 12.0, screen_rect.min.y + 12.0),
                        egui::Align2::RIGHT_TOP,
                        &self.player_state.quality,
                        egui::FontId::monospace(10.5),
                        Color32::from_rgb(138, 180, 248),
                    );

                    // Center Play/Pause Indicator (shows on hover or when paused)
                    if screen_resp.hovered() || !self.player_state.is_playing {
                        let center = screen_rect.center();
                        painter.circle_filled(
                            center,
                            28.0,
                            Color32::from_rgba_unmultiplied(32, 33, 36, 210),
                        );
                        let sym = if self.player_state.is_playing {
                            "||"
                        } else {
                            ">"
                        };
                        painter.text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            sym,
                            egui::FontId::proportional(22.0),
                            Color32::from_rgb(241, 243, 244),
                        );
                    }

                    // Bottom Visualizer: Animated dynamic audio waveform
                    let wave_y = screen_rect.max.y - 14.0;
                    let num_bars = 28;
                    let bar_w = (player_w - 24.0) / (num_bars as f32 * 1.5);
                    for i in 0..num_bars {
                        let bar_x = screen_rect.min.x + 12.0 + i as f32 * bar_w * 1.5;
                        let height = if self.player_state.is_playing {
                            let phase = self.player_state.waveform_phase + (i as f32) * 0.35;
                            (phase.sin().abs() * 16.0 * self.player_state.volume + 3.0)
                                .clamp(3.0, 24.0)
                        } else {
                            3.0
                        };
                        let bar_rect = egui::Rect::from_min_max(
                            egui::pos2(bar_x, wave_y - height),
                            egui::pos2(bar_x + bar_w, wave_y),
                        );
                        painter.rect_filled(
                            bar_rect,
                            1.5,
                            Color32::from_rgba_unmultiplied(138, 180, 248, 180),
                        );
                    }

                    ui.add_space(8.0);

                    // 2. Interactive Scrubber Timeline (Classic YouTube Red Played Bar)
                    let (timeline_rect, timeline_resp) = ui.allocate_exact_size(
                        egui::vec2(player_w, 14.0),
                        egui::Sense::click_and_drag(),
                    );
                    let t_painter = ui.painter_at(timeline_rect);
                    let bar_y = timeline_rect.center().y;
                    let bar_track = egui::Rect::from_min_max(
                        egui::pos2(timeline_rect.min.x, bar_y - 2.5),
                        egui::pos2(timeline_rect.max.x, bar_y + 2.5),
                    );
                    t_painter.rect_filled(bar_track, 2.5, Color32::from_rgb(60, 64, 67));

                    // Buffered indicator
                    let buf_pct = ((self.player_state.current_time * 1.35 + 25.0)
                        / self.player_state.duration)
                        .clamp(0.0, 1.0);
                    let buf_track = egui::Rect::from_min_max(
                        egui::pos2(timeline_rect.min.x, bar_y - 2.5),
                        egui::pos2(timeline_rect.min.x + player_w * buf_pct, bar_y + 2.5),
                    );
                    t_painter.rect_filled(buf_track, 2.5, Color32::from_rgb(95, 99, 104));

                    // Played progress indicator (YouTube Red #ff0000)
                    let play_pct = (self.player_state.current_time / self.player_state.duration)
                        .clamp(0.0, 1.0);
                    let played_track = egui::Rect::from_min_max(
                        egui::pos2(timeline_rect.min.x, bar_y - 2.5),
                        egui::pos2(timeline_rect.min.x + player_w * play_pct, bar_y + 2.5),
                    );
                    t_painter.rect_filled(played_track, 2.5, Color32::from_rgb(255, 0, 0));

                    // Scrubber thumb
                    let thumb_x = timeline_rect.min.x + player_w * play_pct;
                    let thumb_r = if timeline_resp.hovered() || timeline_resp.dragged() {
                        6.0
                    } else {
                        4.5
                    };
                    t_painter.circle_filled(egui::pos2(thumb_x, bar_y), thumb_r, Color32::WHITE);

                    // Scrubber seek interaction
                    if timeline_resp.clicked() || timeline_resp.dragged() {
                        if let Some(pos) = timeline_resp.interact_pointer_pos() {
                            let click_pct =
                                ((pos.x - timeline_rect.min.x) / player_w).clamp(0.0, 1.0);
                            self.player_state.current_time = click_pct * self.player_state.duration;
                        }
                    }

                    // Hover timecode tooltip
                    if timeline_resp.hovered() {
                        if let Some(pos) = timeline_resp.hover_pos() {
                            let hover_pct =
                                ((pos.x - timeline_rect.min.x) / player_w).clamp(0.0, 1.0);
                            let hover_secs = hover_pct * self.player_state.duration;
                            timeline_resp.show_tooltip_text(format_duration_clock(hover_secs));
                        }
                    }

                    ui.add_space(4.0);

                    // 3. Media Controls Bar
                    Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Play / Pause
                                let play_label = if self.player_state.is_playing {
                                    "Pause"
                                } else {
                                    "Play"
                                };
                                if ui
                                    .button(
                                        RichText::new(play_label)
                                            .size(12.0)
                                            .color(Color32::from_rgb(138, 180, 248))
                                            .strong(),
                                    )
                                    .clicked()
                                {
                                    self.player_state.is_playing = !self.player_state.is_playing;
                                    if self.player_state.is_playing {
                                        self.audio_engine.play();
                                    } else {
                                        self.audio_engine.pause();
                                    }
                                }

                                // Rewind 10s
                                if ui.small_button("-10s").clicked() {
                                    self.player_state.current_time =
                                        (self.player_state.current_time - 10.0).max(0.0);
                                }

                                // Forward 10s
                                if ui.small_button("+10s").clicked() {
                                    self.player_state.current_time =
                                        (self.player_state.current_time + 10.0)
                                            .min(self.player_state.duration);
                                }

                                ui.add_space(6.0);

                                // Time Clock
                                let cur_clock =
                                    format_duration_clock(self.player_state.current_time);
                                let total_clock = format_duration_clock(self.player_state.duration);
                                ui.label(
                                    RichText::new(format!("{} / {}", cur_clock, total_clock))
                                        .size(11.5)
                                        .color(Color32::from_rgb(218, 220, 224))
                                        .monospace(),
                                );

                                // Right Controls
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        // Theater Mode Toggle
                                        let th_label = if self.player_state.theater_mode {
                                            "Standard"
                                        } else {
                                            "Theater"
                                        };
                                        if ui.button(RichText::new(th_label).size(11.0)).clicked() {
                                            self.player_state.theater_mode =
                                                !self.player_state.theater_mode;
                                        }

                                        // Speed Selector
                                        let spd_text = format!("{:.2}x", self.player_state.speed);
                                        if ui
                                            .button(
                                                RichText::new(spd_text)
                                                    .size(11.0)
                                                    .color(Color32::from_rgb(138, 180, 248)),
                                            )
                                            .on_hover_text("Playback Speed (Click to cycle)")
                                            .clicked()
                                        {
                                            self.player_state.speed =
                                                match (self.player_state.speed * 10.0) as i32 {
                                                    10 => 1.25,
                                                    12 | 13 => 1.5,
                                                    15 => 2.0,
                                                    20 => 0.5,
                                                    _ => 1.0,
                                                };
                                            self.audio_engine.set_speed(self.player_state.speed);
                                        }

                                        // Volume Toggle and Interactive Slider
                                        let vol_label = if self.player_state.is_muted {
                                            "Muted"
                                        } else if self.player_state.volume == 0.0 {
                                            "Silent"
                                        } else {
                                            "Vol"
                                        };
                                        if ui.button(RichText::new(vol_label).size(11.0)).clicked()
                                        {
                                            self.player_state.is_muted =
                                                !self.player_state.is_muted;
                                            self.audio_engine.set_muted(self.player_state.is_muted);
                                        }

                                        let prev_vol = self.player_state.volume;
                                        let vol_slider = ui.add_sized(
                                            [65.0, 18.0],
                                            egui::Slider::new(
                                                &mut self.player_state.volume,
                                                0.0..=1.0,
                                            )
                                            .show_value(false),
                                        );
                                        if vol_slider.changed()
                                            || (self.player_state.volume - prev_vol).abs() > 0.001
                                        {
                                            if self.player_state.is_muted
                                                && self.player_state.volume > 0.0
                                            {
                                                self.player_state.is_muted = false;
                                            }
                                            self.audio_engine.set_volume(self.player_state.volume);
                                            self.audio_engine.set_muted(self.player_state.is_muted);
                                        }

                                        ui.label(
                                            RichText::new(format!(
                                                "{:.0}%",
                                                if self.player_state.is_muted {
                                                    0.0
                                                } else {
                                                    self.player_state.volume * 100.0
                                                }
                                            ))
                                            .size(10.5)
                                            .color(Color32::from_rgb(154, 160, 166))
                                            .monospace(),
                                        );

                                        // External pop-out option
                                        if ui
                                            .small_button("External")
                                            .on_hover_text("Open in External App")
                                            .clicked()
                                        {
                                            let target_url = format!(
                                                "https://www.youtube.com/watch?v={}",
                                                watch.video_id
                                            );
                                            #[cfg(target_os = "windows")]
                                            {
                                                let _ = std::process::Command::new("cmd")
                                                    .args(["/C", "start", &target_url])
                                                    .spawn();
                                            }
                                            #[cfg(not(target_os = "windows"))]
                                            {
                                                let _ = std::process::Command::new("xdg-open")
                                                    .arg(&target_url)
                                                    .spawn();
                                            }
                                        }

                                        if ui.small_button("Copy URL").clicked() {
                                            ui.ctx().copy_text(format!(
                                                "https://www.youtube.com/watch?v={}",
                                                watch.video_id
                                            ));
                                        }
                                    },
                                );
                            });
                        });

                    ui.add_space(10.0);

                    // 4. Video Details Section
                    ui.label(
                        RichText::new(&watch.title)
                            .size(17.0)
                            .strong()
                            .color(Color32::from_rgb(241, 243, 244)),
                    );

                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&watch.author)
                                .size(13.0)
                                .color(Color32::from_rgb(138, 180, 248))
                                .strong(),
                        );

                        Frame::NONE
                            .fill(Color32::from_rgb(40, 42, 45))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                            .corner_radius(egui::CornerRadius::same(4))
                            .inner_margin(Margin::symmetric(5, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("VERIFIED")
                                        .size(9.5)
                                        .color(Color32::from_rgb(154, 160, 166))
                                        .strong(),
                                );
                            });

                        if !watch.views.is_empty() {
                            ui.label(
                                RichText::new(format!("- {} views", watch.views))
                                    .size(12.0)
                                    .color(Color32::from_rgb(154, 160, 166)),
                            );
                        }
                    });

                    // Collapsible Description
                    if !watch.description.is_empty() {
                        ui.add_space(8.0);
                        let is_expanded = self.player_state.description_expanded;
                        let toggle_lbl = if is_expanded {
                            "Hide description"
                        } else {
                            "Show description"
                        };
                        if ui
                            .link(
                                RichText::new(toggle_lbl)
                                    .size(11.5)
                                    .color(Color32::from_rgb(138, 180, 248)),
                            )
                            .clicked()
                        {
                            self.player_state.description_expanded = !is_expanded;
                        }

                        if self.player_state.description_expanded {
                            ui.add_space(4.0);
                            Frame::NONE
                                .fill(Color32::from_rgb(32, 33, 36))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56)))
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(&watch.description)
                                            .size(11.5)
                                            .color(Color32::from_rgb(218, 220, 224)),
                                    );
                                });
                        }
                    }

                    // 5. Up Next / Recommendations Carousel
                    if !videos.is_empty() {
                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Up Next Videos")
                                .size(14.0)
                                .strong()
                                .color(Color32::from_rgb(241, 243, 244)),
                        );
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            let col_w = (player_w - 14.0) / 2.0;
                            ui.vertical(|col_ui| {
                                col_ui.set_width(col_w);
                                for video in videos
                                    .iter()
                                    .filter(|v| v.id != watch.video_id)
                                    .step_by(2)
                                    .take(4)
                                {
                                    self.render_video_card(col_ui, video, action);
                                    col_ui.add_space(10.0);
                                }
                            });
                            ui.add_space(10.0);
                            ui.vertical(|col_ui| {
                                col_ui.set_width(col_w);
                                for video in videos
                                    .iter()
                                    .filter(|v| v.id != watch.video_id)
                                    .skip(1)
                                    .step_by(2)
                                    .take(4)
                                {
                                    self.render_video_card(col_ui, video, action);
                                    col_ui.add_space(10.0);
                                }
                            });
                        });
                    }
                });
            });
    }
}

fn render_stage_badge(ui: &mut Ui, label: &str, active: bool) {
    let (bg, border, fg) = if active {
        (
            Color32::from_rgb(40, 42, 45),
            Color32::from_rgb(60, 64, 67),
            Color32::from_rgb(138, 180, 248),
        )
    } else {
        (
            Color32::from_rgb(32, 33, 36),
            Color32::from_rgb(48, 51, 56),
            Color32::from_rgb(154, 160, 166),
        )
    };

    Frame::NONE
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(10.0).color(fg).strong());
        });
}

pub fn extract_youtube_videos(html: &str) -> Vec<YouTubeVideoItem> {
    let mut videos = Vec::new();
    let mut search_from = 0;
    while let Some(rel_idx) = html[search_from..].find("ytInitialData") {
        let start_idx = search_from + rel_idx;
        let slice = &html[start_idx..];
        if let Some(eq_idx) = slice.find('=') {
            if let Some(brace_idx) = slice[eq_idx..].find('{') {
                let json_slice = &slice[eq_idx + brace_idx..];
                let de = serde_json::Deserializer::from_str(json_slice);
                if let Some(Ok(val)) = de.into_iter::<serde_json::Value>().next() {
                    collect_videos_from_json(&val, &mut videos);
                    if !videos.is_empty() {
                        return videos;
                    }
                }
            }
        }
        search_from = start_idx + 13;
    }

    // Fallback: extract /watch?v= links if JSON parsing returned no items
    if videos.is_empty() {
        let mut seen_ids = std::collections::HashSet::new();
        let mut idx = 0;
        while let Some(rel) = html[idx..].find("/watch?v=") {
            let start = idx + rel + 9;
            if start + 11 <= html.len() {
                let id = &html[start..start + 11];
                if id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    && seen_ids.insert(id.to_string())
                {
                    let title = if let Some(title_start) = html[..idx + rel].rfind("title=\"") {
                        let after = &html[title_start + 7..];
                        if let Some(q_end) = after.find('"') {
                            after[..q_end].to_string()
                        } else {
                            format!("YouTube Video ({})", id)
                        }
                    } else {
                        format!("YouTube Video ({})", id)
                    };

                    videos.push(YouTubeVideoItem {
                        id: id.to_string(),
                        title,
                        channel: "YouTube Creator".to_string(),
                        duration: "Video".to_string(),
                        views: "Popular".to_string(),
                        published: "Available".to_string(),
                        thumbnail_url: format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", id),
                    });
                }
            }
            idx = start;
        }
    }

    videos
}

pub fn extract_video_id_from_url(url: &str) -> Option<String> {
    if let Some(pos) = url.find("v=") {
        let after = &url[pos + 2..];
        let end = after.find('&').unwrap_or(after.len());
        let id = &after[..end];
        if !id.is_empty() && id.len() <= 16 {
            return Some(id.to_string());
        }
    }
    if let Some(pos) = url.find("youtu.be/") {
        let after = &url[pos + 9..];
        let end = after
            .find('?')
            .or_else(|| after.find('&'))
            .unwrap_or(after.len());
        let id = &after[..end];
        if !id.is_empty() && id.len() <= 16 {
            return Some(id.to_string());
        }
    }
    if let Some(pos) = url.find("/embed/") {
        let after = &url[pos + 7..];
        let end = after
            .find('?')
            .or_else(|| after.find('&'))
            .unwrap_or(after.len());
        let id = &after[..end];
        if !id.is_empty() && id.len() <= 16 {
            return Some(id.to_string());
        }
    }
    None
}

pub fn extract_youtube_watch_details(html: &str) -> Option<YouTubeWatchDetails> {
    let mut search_from = 0;
    while let Some(rel_idx) = html[search_from..].find("ytInitialData") {
        let start_idx = search_from + rel_idx;
        let slice = &html[start_idx..];
        if let Some(eq_idx) = slice.find('=') {
            if let Some(brace_idx) = slice[eq_idx..].find('{') {
                let json_slice = &slice[eq_idx + brace_idx..];
                let de = serde_json::Deserializer::from_str(json_slice);
                if let Some(Ok(val)) = de.into_iter::<serde_json::Value>().next() {
                    if let Some(details) = find_video_details_in_json(&val) {
                        return Some(details);
                    }
                }
            }
        }
        search_from = start_idx + 13;
    }

    if let Some(start_idx) = html.find("ytInitialPlayerResponse") {
        let slice = &html[start_idx..];
        if let Some(eq_idx) = slice.find('=') {
            if let Some(brace_idx) = slice[eq_idx..].find('{') {
                let json_slice = &slice[eq_idx + brace_idx..];
                let de = serde_json::Deserializer::from_str(json_slice);
                if let Some(Ok(val)) = de.into_iter::<serde_json::Value>().next() {
                    if let Some(details) = find_video_details_in_json(&val) {
                        return Some(details);
                    }
                }
            }
        }
    }

    None
}

fn collect_videos_from_json(val: &serde_json::Value, videos: &mut Vec<YouTubeVideoItem>) {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(vr) = map.get("videoRenderer").and_then(|v| v.as_object()) {
                if let Some(item) = parse_video_renderer(vr) {
                    if !videos.iter().any(|existing| existing.id == item.id) {
                        videos.push(item);
                    }
                }
            } else if let Some(cvr) = map.get("compactVideoRenderer").and_then(|v| v.as_object()) {
                if let Some(item) = parse_video_renderer(cvr) {
                    if !videos.iter().any(|existing| existing.id == item.id) {
                        videos.push(item);
                    }
                }
            } else if let Some(rir) = map.get("reelItemRenderer").and_then(|v| v.as_object()) {
                if let Some(item) = parse_video_renderer(rir) {
                    if !videos.iter().any(|existing| existing.id == item.id) {
                        videos.push(item);
                    }
                }
            }
            for child in map.values() {
                collect_videos_from_json(child, videos);
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                collect_videos_from_json(child, videos);
            }
        }
        _ => {}
    }
}

fn parse_video_renderer(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<YouTubeVideoItem> {
    let id = map.get("videoId")?.as_str()?.to_string();
    if id.is_empty() {
        return None;
    }

    let title = map
        .get("title")
        .and_then(|t| {
            t.get("runs")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|txt| txt.as_str())
                .or_else(|| t.get("simpleText").and_then(|st| st.as_str()))
        })
        .unwrap_or("Untitled Video")
        .to_string();

    let channel = map
        .get("ownerText")
        .or_else(|| map.get("longBylineText"))
        .or_else(|| map.get("shortBylineText"))
        .and_then(|o| {
            o.get("runs")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|txt| txt.as_str())
                .or_else(|| o.get("simpleText").and_then(|st| st.as_str()))
        })
        .unwrap_or("YouTube Channel")
        .to_string();

    let views = map
        .get("viewCountText")
        .or_else(|| map.get("shortViewCountText"))
        .and_then(|v| {
            v.get("simpleText").and_then(|st| st.as_str()).or_else(|| {
                v.get("runs")
                    .and_then(|r| r.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|f| f.get("text"))
                    .and_then(|t| t.as_str())
            })
        })
        .unwrap_or("")
        .to_string();

    let duration = map
        .get("lengthText")
        .and_then(|l| l.get("simpleText").and_then(|st| st.as_str()))
        .unwrap_or("")
        .to_string();

    let published = map
        .get("publishedTimeText")
        .and_then(|p| p.get("simpleText").and_then(|st| st.as_str()))
        .unwrap_or("")
        .to_string();

    let thumbnail_url = map
        .get("thumbnail")
        .and_then(|th| th.get("thumbnails"))
        .and_then(|arr| arr.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    Some(YouTubeVideoItem {
        id,
        title,
        channel,
        views,
        duration,
        published,
        thumbnail_url,
    })
}

fn find_video_details_in_json(val: &serde_json::Value) -> Option<YouTubeWatchDetails> {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(vd) = map.get("videoDetails").and_then(|v| v.as_object()) {
                let video_id = vd
                    .get("videoId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = vd
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("YouTube Video")
                    .to_string();
                let author = vd
                    .get("author")
                    .and_then(|v| v.as_str())
                    .unwrap_or("YouTube Channel")
                    .to_string();
                let views = vd
                    .get("viewCount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let description = vd
                    .get("shortDescription")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if !video_id.is_empty() {
                    return Some(YouTubeWatchDetails {
                        video_id,
                        title,
                        author,
                        views,
                        description,
                    });
                }
            }

            for child in map.values() {
                if let Some(res) = find_video_details_in_json(child) {
                    return Some(res);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                if let Some(res) = find_video_details_in_json(child) {
                    return Some(res);
                }
            }
        }
        _ => {}
    }
    None
}

pub fn extract_youtube_audio_url(html: &str) -> Option<String> {
    if let Some(start_idx) = html.find("ytInitialPlayerResponse") {
        let slice = &html[start_idx..];
        if let Some(eq_idx) = slice.find('=') {
            if let Some(brace_idx) = slice[eq_idx..].find('{') {
                let json_slice = &slice[eq_idx + brace_idx..];
                let de = serde_json::Deserializer::from_str(json_slice);
                if let Some(Ok(val)) = de.into_iter::<serde_json::Value>().next() {
                    if let Some(url) = find_audio_url_in_json(&val) {
                        return Some(url);
                    }
                }
            }
        }
    }
    None
}

fn find_audio_url_in_json(val: &serde_json::Value) -> Option<String> {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(streaming_data) = map.get("streamingData").and_then(|v| v.as_object()) {
                if let Some(formats) = streaming_data
                    .get("adaptiveFormats")
                    .and_then(|v| v.as_array())
                {
                    for fmt in formats {
                        let mime = fmt.get("mimeType").and_then(|v| v.as_str()).unwrap_or("");
                        if mime.starts_with("audio/") {
                            if let Some(url) = fmt.get("url").and_then(|v| v.as_str()) {
                                if !url.is_empty() {
                                    return Some(url.to_string());
                                }
                            }
                        }
                    }
                }
            }
            for child in map.values() {
                if let Some(res) = find_audio_url_in_json(child) {
                    return Some(res);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                if let Some(res) = find_audio_url_in_json(child) {
                    return Some(res);
                }
            }
        }
        _ => {}
    }
    None
}

fn format_unix_timestamp(ts: i64) -> String {
    if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        ts.to_string()
    }
}
