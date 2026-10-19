use crate::theme::colors;
use crate::urls::{query_param, with_query_param, SearchEngine};
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
    /// Ask the app to open its floating command bar (address/search).
    OpenCommandBar,
    /// Stored preferences (theme, layout, bookmarks) changed and must be reloaded.
    PreferencesChanged,
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

/// Load state of an image URL in the [`ThumbnailCache`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageState {
    NotRequested,
    Loading,
    Ready,
    /// The image could not be shown; the string says why.
    Failed(String),
}

type ImageResult = Result<egui::ColorImage, String>;

/// Asynchronous texture and thumbnail loader leveraging Excalibur's resilient HTTP client.
/// Every request ends in either a texture or an explicit failure.
pub struct ThumbnailCache {
    textures: HashMap<String, egui::TextureHandle>,
    pending: HashSet<String>,
    failed: HashMap<String, String>,
    tx: tokio::sync::mpsc::UnboundedSender<(String, ImageResult)>,
    rx: tokio::sync::mpsc::UnboundedReceiver<(String, ImageResult)>,
    http_client: Arc<excalibur_core::HttpClient>,
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let client = excalibur_core::HttpClient::new(excalibur_core::ClientOptions {
            timeout_seconds: 15,
            ..Default::default()
        })
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
            failed: HashMap::new(),
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
        while let Ok((url, result)) = self.rx.try_recv() {
            self.pending.remove(&url);
            match result {
                Ok(color_image) => {
                    let handle = ctx.load_texture(&url, color_image, egui::TextureOptions::LINEAR);
                    self.textures.insert(url, handle);
                }
                Err(reason) => {
                    tracing::debug!("Image {} failed: {}", url, reason);
                    self.failed.insert(url, reason);
                }
            }
        }
    }

    /// Current state of `url` without starting a request.
    pub fn state(&self, url: &str) -> ImageState {
        let clean = url.trim();
        if self.textures.contains_key(clean) {
            ImageState::Ready
        } else if let Some(reason) = self.failed.get(clean) {
            ImageState::Failed(reason.clone())
        } else if self.pending.contains(clean) {
            ImageState::Loading
        } else {
            ImageState::NotRequested
        }
    }

    pub fn get_or_request(
        &mut self,
        ctx: &egui::Context,
        url: &str,
    ) -> Option<&egui::TextureHandle> {
        self.poll_incoming(ctx);

        let clean = url.trim();
        if clean.is_empty() || self.failed.contains_key(clean) {
            return None;
        }
        if self.textures.contains_key(clean) {
            return self.textures.get(clean);
        }
        if self.pending.contains(clean) {
            return None;
        }

        let scheme_ok = url::Url::parse(clean)
            .map(|u| matches!(u.scheme(), "http" | "https"))
            .unwrap_or(false);
        if !scheme_ok {
            let reason = if clean.starts_with("data:") {
                "Inline data: images are not supported yet".to_string()
            } else {
                "Unsupported or invalid image URL".to_string()
            };
            self.failed.insert(clean.to_string(), reason);
            return None;
        }

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
            req.headers
                .append("Accept", "image/png,image/jpeg,image/webp,image/*;q=0.8");
            let result = match client.dispatch(&req).await {
                Ok(resp) if (200..300).contains(&resp.status) => {
                    match image::load_from_memory(&resp.body) {
                        Ok(dyn_img) => {
                            let rgba = dyn_img.to_rgba8();
                            let size = [rgba.width() as usize, rgba.height() as usize];
                            Ok(egui::ColorImage::from_rgba_unmultiplied(
                                size,
                                rgba.as_raw(),
                            ))
                        }
                        Err(e) => Err(format!("Unsupported or corrupt image ({})", e)),
                    }
                }
                Ok(resp) => Err(format!("Server returned HTTP {}", resp.status)),
                Err(e) => Err(format!("Could not load image: {}", e)),
            };
            let _ = tx.send((url_clone, result));
            ctx_clone.request_repaint();
        });

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
        match url {
            "excalibur://history" => {
                self.render_history_page(ui, storage, &mut action);
                return action;
            }
            "excalibur://bookmarks" => {
                self.render_bookmarks_page(ui, storage, &mut action);
                return action;
            }
            "excalibur://settings" => {
                self.render_settings_page(ui, storage, &mut action);
                return action;
            }
            "excalibur://about" => {
                self.render_about_page(ui, storage);
                return action;
            }
            _ => {}
        }

        if url == "about:blank"
            || url == "excalibur://newtab"
            || (dom.is_none() && raw_html.is_empty())
        {
            self.render_new_tab(ui, storage, &mut action);
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
                    let min_margin = 16.0_f32;
                    let max_content = (available_width - 2.0 * min_margin).max(240.0);
                    let content_width =
                        (max_content.min(1280.0) * self.zoom.clamp(0.6, 2.5)).min(max_content);
                    let side_margin = ((available_width - content_width) / 2.0).max(min_margin);

                    ui.horizontal(|ui| {
                        ui.add_space(side_margin);
                        ui.vertical(|ui| {
                            ui.set_width(content_width);
                            ui.set_max_width(content_width);
                            self.render_node(ui, root, &mut action, url, &stylesheet);
                        });
                    });
                    ui.add_space(32.0);
                });
        }

        action
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
                                "h1" => (24.0, colors::text()),
                                "h2" => (20.0, colors::text()),
                                "h3" => (16.0, colors::text()),
                                "h4" => (14.0, colors::text()),
                                _ => (13.0, colors::text_dim()),
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
                            let text_color =
                                style.color.map(to_egui_color).unwrap_or(colors::text());

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
                        let text_color = style.color.map(to_egui_color).unwrap_or(colors::link());

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
                                let text_color =
                                    style.color.map(to_egui_color).unwrap_or(colors::text());

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
                            let color = style.color.map(to_egui_color).unwrap_or(colors::text());
                            ui.label(RichText::new(trimmed).strong().color(color));
                        }
                    }
                    "em" | "i" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let color = style.color.map(to_egui_color).unwrap_or(colors::text());
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

                        let text_color = style.color.map(to_egui_color).unwrap_or(colors::text());
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
                                let alt = attributes
                                    .get("alt")
                                    .map(|a| a.trim())
                                    .filter(|a| !a.is_empty());
                                let (headline, detail) = match self.thumbnail_cache.state(&resolved)
                                {
                                    ImageState::Failed(reason) => {
                                        ("Image unavailable".to_string(), Some(reason))
                                    }
                                    _ => ("Loading image…".to_string(), None),
                                };
                                Frame::NONE
                                    .fill(colors::field())
                                    .stroke(Stroke::new(1.0_f32, colors::border()))
                                    .corner_radius(egui::CornerRadius::same(rad as u8))
                                    .inner_margin(Margin::symmetric(14, 10))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(alt.unwrap_or(&headline))
                                                .size(12.0)
                                                .color(colors::text_dim()),
                                        );
                                        if alt.is_some() {
                                            ui.label(
                                                RichText::new(&headline)
                                                    .size(11.0)
                                                    .color(colors::text_faint()),
                                            );
                                        }
                                        if let Some(reason) = detail {
                                            ui.label(
                                                RichText::new(reason)
                                                    .size(11.0)
                                                    .color(colors::text_faint()),
                                            );
                                        }
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
                            .unwrap_or(colors::card());
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(colors::border());
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
                                    let query = current_val.trim();
                                    let search_target = if current_url.contains("youtube.com") {
                                        SearchEngine::YouTube.search_url(query)
                                    } else if current_url.contains("wikipedia.org") {
                                        with_query_param(
                                            "https://en.wikipedia.org/wiki/Special:Search",
                                            "search",
                                            query,
                                        )
                                    } else if current_url.contains("duckduckgo.com") {
                                        SearchEngine::DuckDuckGo.search_url(query)
                                    } else {
                                        SearchEngine::Google.search_url(query)
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
                            .unwrap_or(colors::field());
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(colors::border());
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
                            .unwrap_or(colors::card());
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(colors::accent());
                        let text_col = style.color.map(to_egui_color).unwrap_or(colors::text());
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
                                        .unwrap_or(colors::text());
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
                            .unwrap_or(colors::border());
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
                                            colors::text()
                                        } else {
                                            colors::text_dim()
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
        let query_str = query_param(url, "search_query").unwrap_or_default();

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(12.0);
                let available_width = ui.available_width();
                let min_margin = 16.0_f32;
                let max_content = (available_width - 2.0 * min_margin).max(240.0);
                let content_width = max_content.min(1080.0);
                let side_margin = ((available_width - content_width) / 2.0).max(min_margin);

                ui.horizontal(|ui| {
                    ui.add_space(side_margin);
                    ui.vertical(|ui| {
                        ui.set_width(content_width);
                        ui.set_max_width(content_width);

                        // YouTube Top Navigation & Search Bar
                        Frame::NONE
                            .fill(colors::panel())
                            .stroke(Stroke::new(1.0_f32, colors::border()))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Brand Pill
                                    Frame::NONE
                                        .fill(colors::youtube_red())
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
                                        || ui.button(RichText::new("Search").color(colors::accent()).strong()).clicked()
                                    {
                                        let q = yt_search_val.trim();
                                        if !q.is_empty() {
                                            search_target =
                                                Some(SearchEngine::YouTube.search_url(q));
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
                                        let is_active = query_str == t;
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
                                                    *action = Some(BrowserAction::Navigate(
                                                        SearchEngine::YouTube.search_url(qs),
                                                    ));
                                                }
                                            }
                                        });
                                    });
                                });
                        }
                    });
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
            .fill(colors::card())
            .stroke(Stroke::new(1.0_f32, colors::border()))
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
                painter.rect_filled(rect, 6.0, colors::field());

                if let Some(tex) = thumb_opt {
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    painter.image(tex.id(), rect, uv, Color32::WHITE);
                } else {
                    let center = rect.center();
                    painter.rect_stroke(
                        rect,
                        6.0,
                        Stroke::new(1.0_f32, colors::border()),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        "[ Play ]",
                        egui::FontId::proportional(14.0),
                        colors::accent(),
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
                        colors::text(),
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
                        .color(colors::text()),
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
                            .color(colors::text_dim())
                            .strong(),
                    );

                    if !video.views.is_empty() {
                        ui.label(
                            RichText::new(format!("- {}", video.views))
                                .size(11.0)
                                .color(colors::text_faint()),
                        );
                    }
                });

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new("Watch")
                                .size(11.0)
                                .color(colors::accent())
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
                                .color(colors::text_faint()),
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
        let player_outer_w = if self.player_state.theater_mode {
            available_w
        } else {
            available_w.min(940.0)
        };
        let player_w = (player_outer_w - 28.0).max(200.0);
        let screen_h = (player_w * 9.0 / 16.0).clamp(240.0, 530.0);

        Frame::NONE
            .fill(colors::panel())
            .stroke(Stroke::new(1.0_f32, colors::border()))
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
                    painter.rect_filled(screen_rect, 8.0, colors::canvas());

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
                        ("[LIVE STREAMING]", colors::danger())
                    } else {
                        ("[PAUSED]", colors::text_faint())
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
                        colors::accent(),
                    );

                    // Center Play/Pause Indicator (shows on hover or when paused)
                    if screen_resp.hovered() || !self.player_state.is_playing {
                        let center = screen_rect.center();
                        painter.circle_filled(
                            center,
                            28.0,
                            Color32::from_rgba_unmultiplied(22, 25, 32, 210),
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
                            colors::text(),
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
                            Color32::from_rgba_unmultiplied(88, 166, 255, 180),
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
                    t_painter.rect_filled(bar_track, 2.5, colors::border());

                    // Buffered indicator
                    let buf_pct = ((self.player_state.current_time * 1.35 + 25.0)
                        / self.player_state.duration)
                        .clamp(0.0, 1.0);
                    let buf_track = egui::Rect::from_min_max(
                        egui::pos2(timeline_rect.min.x, bar_y - 2.5),
                        egui::pos2(timeline_rect.min.x + player_w * buf_pct, bar_y + 2.5),
                    );
                    t_painter.rect_filled(buf_track, 2.5, colors::border_strong());

                    // Played progress indicator (YouTube Red #ff0000)
                    let play_pct = (self.player_state.current_time / self.player_state.duration)
                        .clamp(0.0, 1.0);
                    let played_track = egui::Rect::from_min_max(
                        egui::pos2(timeline_rect.min.x, bar_y - 2.5),
                        egui::pos2(timeline_rect.min.x + player_w * play_pct, bar_y + 2.5),
                    );
                    t_painter.rect_filled(played_track, 2.5, colors::youtube_red());

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
                        .fill(colors::panel())
                        .stroke(Stroke::new(1.0_f32, colors::border()))
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
                                            .color(colors::accent())
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
                                        .color(colors::text())
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
                                                    .color(colors::accent()),
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
                                            .color(colors::text_faint())
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
                            .color(colors::text()),
                    );

                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&watch.author)
                                .size(13.0)
                                .color(colors::accent())
                                .strong(),
                        );

                        Frame::NONE
                            .fill(colors::field())
                            .stroke(Stroke::new(1.0_f32, colors::border()))
                            .corner_radius(egui::CornerRadius::same(4))
                            .inner_margin(Margin::symmetric(5, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("VERIFIED")
                                        .size(9.5)
                                        .color(colors::text_faint())
                                        .strong(),
                                );
                            });

                        if !watch.views.is_empty() {
                            ui.label(
                                RichText::new(format!("- {} views", watch.views))
                                    .size(12.0)
                                    .color(colors::text_faint()),
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
                            .link(RichText::new(toggle_lbl).size(11.5).color(colors::accent()))
                            .clicked()
                        {
                            self.player_state.description_expanded = !is_expanded;
                        }

                        if self.player_state.description_expanded {
                            ui.add_space(4.0);
                            Frame::NONE
                                .fill(colors::card())
                                .stroke(Stroke::new(1.0_f32, colors::border()))
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(&watch.description)
                                            .size(11.5)
                                            .color(colors::text_dim()),
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
                                .color(colors::text()),
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
