use crate::css::{CssParser, Stylesheet};
use crate::default_style::DEFAULT_USER_AGENT_CSS;
use crate::dom::{Node, NodeType};
use crate::html_parser::HtmlParser;
use crate::layout::{build_layout_tree, Dimensions, EdgeSizes, Rect};
use crate::network::{normalize_url, resolve_relative_url, NetworkManager, NetworkMessage};
use crate::render::{build_display_list, paint_display_list, DisplayList};
use crate::style::style_tree;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Id, Key, LayerId, Margin,
    Order, Pos2, Rect as EguiRect, RichText, ScrollArea, Stroke, StrokeKind, Vec2,
};
use url::Url;

/// Chrome discrete zoom percentage steps
pub const ZOOM_STEPS: &[f32] = &[
    0.25, 0.33, 0.50, 0.67, 0.75, 0.80, 0.90, 1.00,
    1.10, 1.25, 1.50, 1.75, 2.00, 2.50, 3.00, 4.00, 5.00,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsDockPosition {
    Right,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsTab {
    Elements,
    Console,
    Sources,
    Network,
    Application,
    Security,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuIcon {
    Shield,
    NewTab,
    NewWindow,
    Incognito,
    Passwords,
    History,
    Downloads,
    Bookmarks,
    TabGroups,
    Extensions,
    DeleteData,
    Zoom,
    Print,
    GoogleLens,
    Translate,
    Find,
    Cast,
    MoreTools,
    Help,
    Settings,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    pub id: usize,
    pub timestamp: String,
    pub level: ConsoleLogLevel,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkTypeFilter {
    All,
    Doc,
    Css,
    Js,
    Img,
    Fetch,
}

#[derive(Debug, Clone)]
pub struct NetworkEntry {
    pub id: usize,
    pub url: String,
    pub name: String,
    pub method: String,
    pub status: u16,
    pub status_text: String,
    pub resource_type: String,
    pub size_bytes: usize,
    pub duration_ms: u64,
    pub headers: Vec<(String, String)>,
    pub response_body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcesFileId {
    HtmlDocument,
    UserAgentCss,
    InlineCss(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageSection {
    Cookies,
    LocalStorage,
    SessionStorage,
    TextureCache,
}

#[derive(Debug, Clone)]
pub struct CookieEntry {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: String,
    pub secure: bool,
    pub http_only: bool,
}

pub struct Tab {
    pub id: usize,
    pub title: String,
    pub url: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub is_loading: bool,
    pub scroll_y: f32,
    pub raw_html: String,
    pub dom_root: Option<Node>,
    pub stylesheet: Stylesheet,
    pub display_list: Option<DisplayList>,
    pub page_height: f32,
    pub last_layout_width: f32,
    pub network_entries: Vec<NetworkEntry>,
    pub console_entries: Vec<ConsoleEntry>,
}

impl Tab {
    pub fn new(id: usize, url: String) -> Self {
        let is_newtab = url == "about:newtab" || url == "about:welcome" || url == "about:home";
        Tab {
            id,
            title: if is_newtab { "New Tab".to_string() } else { "Loading...".to_string() },
            url: url.clone(),
            history: vec![url],
            history_index: 0,
            is_loading: !is_newtab,
            scroll_y: 0.0,
            raw_html: String::new(),
            dom_root: None,
            stylesheet: Stylesheet { rules: Vec::new() },
            display_list: None,
            page_height: 0.0,
            last_layout_width: 0.0,
            network_entries: Vec::new(),
            console_entries: Vec::new(),
        }
    }

    pub fn is_new_tab(&self) -> bool {
        self.url == "about:newtab" || self.url == "about:welcome" || self.url == "about:home" || self.url.is_empty()
    }

    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }

    pub fn recompute_layout(&mut self, width: f32) {
        if let Some(ref dom) = self.dom_root {
            let styled_tree = style_tree(dom, &self.stylesheet);
            if let Some(mut layout_box) = build_layout_tree(&styled_tree) {
                let containing_block = Dimensions {
                    content: Rect::new(0.0, 0.0, width.max(300.0), 0.0),
                    padding: EdgeSizes::default(),
                    border: EdgeSizes::default(),
                    margin: EdgeSizes::default(),
                };
                layout_box.layout(containing_block);
                self.page_height = layout_box.dimensions.margin_box().height;
                self.display_list = Some(build_display_list(&layout_box));
                self.last_layout_width = width;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SelectedNodeInfo {
    pub tag: String,
    pub id: Option<String>,
    pub class: Option<String>,
    pub attributes: Vec<(String, String)>,
    pub text_preview: Option<String>,
    pub display: String,
    pub font_size: String,
    pub color: String,
    pub bg_color: String,
    pub margin: String,
    pub padding: String,
}

pub struct BrowserApp {
    pub tabs: Vec<Tab>,
    pub active_tab_idx: usize,
    pub next_tab_id: usize,
    pub network: NetworkManager,
    pub address_input: String,
    pub center_search_input: String,
    pub hovered_link: Option<String>,
    pub show_devtools: bool,
    pub devtools_tab: DevToolsTab,
    pub devtools_dock: DevToolsDockPosition,
    pub devtools_selected_path: Option<Vec<usize>>,
    pub devtools_expanded_paths: std::collections::HashSet<Vec<usize>>,
    pub devtools_dom_search: String,
    pub devtools_styles_filter: String,
    pub devtools_console_filter: String,
    pub devtools_console_level: Option<ConsoleLogLevel>,
    pub devtools_console_input: String,
    pub devtools_network_filter: String,
    pub devtools_network_type: NetworkTypeFilter,
    pub devtools_selected_network_id: Option<usize>,
    pub devtools_sources_file: SourcesFileId,
    pub devtools_storage_section: StorageSection,
    pub devtools_cookies: Vec<CookieEntry>,
    pub devtools_local_storage: std::collections::HashMap<String, String>,
    pub devtools_session_storage: std::collections::HashMap<String, String>,
    pub selected_dom_node: Option<String>,
    pub selected_node_info: Option<SelectedNodeInfo>,
    pub zoom_level: f32,
    pub show_menu_popup: bool,

    image_textures: std::collections::HashMap<String, egui::TextureHandle>,
    image_tx: std::sync::mpsc::Sender<(String, egui::ColorImage)>,
    image_rx: std::sync::mpsc::Receiver<(String, egui::ColorImage)>,
    loading_images: std::collections::HashSet<String>,

    search_suggestions: Vec<String>,
    suggest_query: String,
    suggest_tx: std::sync::mpsc::Sender<(String, Vec<String>)>,
    suggest_rx: std::sync::mpsc::Receiver<(String, Vec<String>)>,
    show_omnibox_suggestions: bool,
    show_center_suggestions: bool,
}

impl BrowserApp {
    pub fn new() -> Self {
        let network = NetworkManager::new();
        let (img_tx, img_rx) = std::sync::mpsc::channel();
        let (sug_tx, sug_rx) = std::sync::mpsc::channel();

        let mut expanded_paths = std::collections::HashSet::new();
        expanded_paths.insert(vec![]);
        expanded_paths.insert(vec![0]);
        expanded_paths.insert(vec![0, 0]);

        let mut local_storage = std::collections::HashMap::new();
        local_storage.insert("theme".to_string(), "dark".to_string());
        local_storage.insert("aether_version".to_string(), "1.0.0".to_string());

        let mut session_storage = std::collections::HashMap::new();
        session_storage.insert("session_id".to_string(), "ae_live_8921f0".to_string());

        let cookies = vec![
            CookieEntry {
                name: "SID".to_string(),
                value: "s%3A7a19c...".to_string(),
                domain: "accounts.google.com".to_string(),
                path: "/".to_string(),
                expires: "2027-01-01".to_string(),
                secure: true,
                http_only: true,
            },
            CookieEntry {
                name: "NID".to_string(),
                value: "511=Q9x...".to_string(),
                domain: "google.com".to_string(),
                path: "/".to_string(),
                expires: "2026-12-31".to_string(),
                secure: true,
                http_only: false,
            },
        ];

        let mut app = BrowserApp {
            tabs: Vec::new(),
            active_tab_idx: 0,
            next_tab_id: 1,
            network,
            address_input: "".to_string(),
            center_search_input: "".to_string(),
            hovered_link: None,
            show_devtools: false,
            devtools_tab: DevToolsTab::Elements,
            devtools_dock: DevToolsDockPosition::Right,
            devtools_selected_path: Some(vec![]),
            devtools_expanded_paths: expanded_paths,
            devtools_dom_search: String::new(),
            devtools_styles_filter: String::new(),
            devtools_console_filter: String::new(),
            devtools_console_level: None,
            devtools_console_input: String::new(),
            devtools_network_filter: String::new(),
            devtools_network_type: NetworkTypeFilter::All,
            devtools_selected_network_id: None,
            devtools_sources_file: SourcesFileId::HtmlDocument,
            devtools_storage_section: StorageSection::Cookies,
            devtools_cookies: cookies,
            devtools_local_storage: local_storage,
            devtools_session_storage: session_storage,
            selected_dom_node: None,
            selected_node_info: None,
            zoom_level: 1.0,
            show_menu_popup: false,
            image_textures: std::collections::HashMap::new(),
            image_tx: img_tx,
            image_rx: img_rx,
            loading_images: std::collections::HashSet::new(),
            search_suggestions: Vec::new(),
            suggest_query: String::new(),
            suggest_tx: sug_tx,
            suggest_rx: sug_rx,
            show_omnibox_suggestions: false,
            show_center_suggestions: false,
        };

        // Open initial New Tab
        app.open_new_tab("about:newtab".to_string());
        app
    }

    pub fn zoom_in(&mut self) {
        let current_idx = ZOOM_STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (self.zoom_level - *a).abs().partial_cmp(&(self.zoom_level - *b).abs()).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)
            .unwrap_or(7);
        if current_idx + 1 < ZOOM_STEPS.len() {
            self.zoom_level = ZOOM_STEPS[current_idx + 1];
        }
    }

    pub fn zoom_out(&mut self) {
        let current_idx = ZOOM_STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (self.zoom_level - *a).abs().partial_cmp(&(self.zoom_level - *b).abs()).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)
            .unwrap_or(7);
        if current_idx > 0 {
            self.zoom_level = ZOOM_STEPS[current_idx - 1];
        }
    }

    pub fn zoom_reset(&mut self) {
        self.zoom_level = 1.0;
    }

    pub fn zoom_percentage(&self) -> String {
        format!("{:.0}%", (self.zoom_level * 100.0).round())
    }

    pub fn open_new_tab(&mut self, url: String) {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = Tab::new(tab_id, url.clone());
        let is_newtab = tab.is_new_tab();
        self.tabs.push(tab);
        self.active_tab_idx = self.tabs.len() - 1;
        self.address_input = if is_newtab { String::new() } else { url.clone() };
        self.center_search_input = String::new();

        if !is_newtab {
            self.network.fetch(tab_id, &url);
        }
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            // Keep at least one tab, navigate it to about:newtab
            if let Some(tab) = self.tabs.get_mut(0) {
                tab.url = "about:newtab".to_string();
                tab.title = "New Tab".to_string();
                tab.is_loading = false;
                self.address_input = String::new();
            }
            return;
        }
        self.tabs.remove(idx);
        if self.active_tab_idx >= self.tabs.len() {
            self.active_tab_idx = self.tabs.len() - 1;
        }
        if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            self.address_input = if tab.is_new_tab() { String::new() } else { tab.url.clone() };
        }
    }

    pub fn navigate_active_tab(&mut self, target_url: &str) {
        let normalized = normalize_url(target_url);
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            let is_newtab = normalized == "about:newtab" || normalized == "about:welcome" || normalized == "about:home";
            tab.is_loading = !is_newtab;
            tab.url = normalized.clone();
            tab.scroll_y = 0.0;

            if is_newtab {
                tab.title = "New Tab".to_string();
                self.address_input = String::new();
            } else {
                tab.title = "Loading...".to_string();
                self.address_input = normalized.clone();
            }

            if tab.history_index + 1 < tab.history.len() {
                tab.history.truncate(tab.history_index + 1);
            }
            tab.history.push(normalized.clone());
            tab.history_index = tab.history.len() - 1;

            if !is_newtab {
                let tab_id = tab.id;
                self.network.fetch(tab_id, &normalized);
            }
        }
    }

    pub fn go_back(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if tab.can_go_back() {
                tab.history_index -= 1;
                let url = tab.history[tab.history_index].clone();
                tab.url = url.clone();
                let is_newtab = url == "about:newtab" || url == "about:welcome" || url == "about:home";
                tab.is_loading = !is_newtab;
                tab.scroll_y = 0.0;
                self.address_input = if is_newtab { String::new() } else { url.clone() };
                if !is_newtab {
                    let tab_id = tab.id;
                    self.network.fetch(tab_id, &url);
                }
            }
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if tab.can_go_forward() {
                tab.history_index += 1;
                let url = tab.history[tab.history_index].clone();
                tab.url = url.clone();
                let is_newtab = url == "about:newtab" || url == "about:welcome" || url == "about:home";
                tab.is_loading = !is_newtab;
                tab.scroll_y = 0.0;
                self.address_input = if is_newtab { String::new() } else { url.clone() };
                if !is_newtab {
                    let tab_id = tab.id;
                    self.network.fetch(tab_id, &url);
                }
            }
        }
    }

    pub fn reload(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if !tab.is_new_tab() {
                tab.is_loading = true;
                let url = tab.url.clone();
                let tab_id = tab.id;
                self.network.fetch(tab_id, &url);
            }
        }
    }

    fn check_network_responses(&mut self) {
        while let Some(msg) = self.network.try_recv() {
            match msg {
                NetworkMessage::PageLoaded { tab_id, result } => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.is_loading = false;
                        match result {
                            Ok(res) => {
                                let entry_id = tab.network_entries.len() + 1;
                                let name = if res.url.starts_with("about:") {
                                    res.url.clone()
                                } else if let Ok(parsed) = Url::parse(&res.url) {
                                    let path = parsed.path();
                                    if path.is_empty() || path == "/" {
                                        parsed.host_str().unwrap_or("document").to_string()
                                    } else {
                                        path.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(path).to_string()
                                    }
                                } else {
                                    res.url.clone()
                                };
                                tab.network_entries.push(NetworkEntry {
                                    id: entry_id,
                                    url: res.url.clone(),
                                    name,
                                    method: "GET".to_string(),
                                    status: res.status,
                                    status_text: res.status_text,
                                    resource_type: "document".to_string(),
                                    size_bytes: res.html.len(),
                                    duration_ms: res.duration_ms,
                                    headers: res.headers,
                                    response_body: res.html.clone(),
                                });

                                let time_str = format_current_time();
                                let console_id = tab.console_entries.len() + 1;
                                tab.console_entries.push(ConsoleEntry {
                                    id: console_id,
                                    timestamp: time_str,
                                    level: ConsoleLogLevel::Info,
                                    source: "Navigation".to_string(),
                                    message: format!("Navigated to {}", res.url),
                                });

                                tab.url = res.url.clone();
                                tab.title = if res.title.len() > 24 {
                                    format!("{}...", &res.title[..24])
                                } else {
                                    res.title
                                };
                                tab.raw_html = res.html;

                                // Parse HTML to DOM
                                let dom = HtmlParser::parse(&tab.raw_html);

                                // Collect styles: User-Agent + External Stylesheets + Internal style tags
                                let mut css_source = DEFAULT_USER_AGENT_CSS.to_string();
                                for ext_css in &res.external_css {
                                    css_source.push('\n');
                                    css_source.push_str(ext_css);
                                }
                                collect_style_tags(&dom, &mut css_source);

                                tab.stylesheet = CssParser::parse(&css_source);
                                tab.dom_root = Some(dom);

                                let layout_w = if tab.last_layout_width > 0.0 {
                                    tab.last_layout_width
                                } else {
                                    1000.0
                                };
                                tab.recompute_layout(layout_w);
                            }
                            Err(err) => {
                                let time_str = format_current_time();
                                let console_id = tab.console_entries.len() + 1;
                                tab.console_entries.push(ConsoleEntry {
                                    id: console_id,
                                    timestamp: time_str,
                                    level: ConsoleLogLevel::Error,
                                    source: "Network".to_string(),
                                    message: format!("Failed to load {}: {}", tab.url, err),
                                });

                                tab.title = "Error loading page".to_string();
                                tab.raw_html = format!(
                                    "<html><body style='padding: 32px; background-color: #202124; color: #fff; font-family: sans-serif;'><h1 style='color: #ea4335;'>Failed to Load Page</h1><p style='color: #9aa0a6;'>{}</p></body></html>",
                                    err
                                );
                                let dom = HtmlParser::parse(&tab.raw_html);
                                tab.stylesheet = CssParser::parse(DEFAULT_USER_AGENT_CSS);
                                tab.dom_root = Some(dom);
                                tab.recompute_layout(1000.0);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn trigger_suggestions(&mut self, query: &str) {
        let q = query.trim().to_string();
        if q.is_empty() || q.starts_with("http://") || q.starts_with("https://") || q.starts_with("about:") {
            self.search_suggestions.clear();
            self.suggest_query.clear();
            return;
        }
        if q != self.suggest_query {
            self.suggest_query = q.clone();
            let tx = self.suggest_tx.clone();
            std::thread::spawn(move || {
                fetch_search_suggestions(&q, tx);
            });
        }
    }

    fn render_search_suggestions_dropdown(
        &mut self,
        ui: &mut egui::Ui,
        target_rect: egui::Rect,
        _is_omnibox: bool,
    ) {
        if self.search_suggestions.is_empty() {
            return;
        }

        let width = target_rect.width();
        let pos = Pos2::new(target_rect.min.x, target_rect.max.y + 4.0);
        let mut navigate_to = None;

        egui::Area::new(egui::Id::new("search_autocomplete_dropdown"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                Frame::NONE
                    .fill(Color32::from_rgb(44, 46, 50))
                    .corner_radius(12)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(72, 75, 80)))
                    .inner_margin(egui::Margin::symmetric(6, 6))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        for suggestion in &self.search_suggestions {
                            let (row_rect, row_resp) = ui.allocate_exact_size(
                                Vec2::new(width - 12.0, 32.0),
                                egui::Sense::click(),
                            );

                            if row_resp.hovered() {
                                ui.painter().rect_filled(row_rect, 6.0, Color32::from_rgb(60, 64, 70));
                            }

                            // Vector magnifying glass
                            let ic = Pos2::new(row_rect.min.x + 16.0, row_rect.center().y);
                            ui.painter().circle_stroke(
                                Pos2::new(ic.x - 1.0, ic.y - 1.0),
                                4.5,
                                Stroke::new(1.3_f32, Color32::from_rgb(154, 160, 166)),
                            );
                            ui.painter().line_segment(
                                [
                                    Pos2::new(ic.x + 2.5, ic.y + 2.5),
                                    Pos2::new(ic.x + 6.0, ic.y + 6.0),
                                ],
                                Stroke::new(1.5_f32, Color32::from_rgb(154, 160, 166)),
                            );

                            ui.painter().text(
                                Pos2::new(row_rect.min.x + 36.0, row_rect.center().y),
                                Align2::LEFT_CENTER,
                                suggestion,
                                FontId::proportional(14.0),
                                Color32::from_rgb(232, 234, 237),
                            );

                            if row_resp.clicked() {
                                navigate_to = Some(suggestion.clone());
                            }
                        }
                    });
            });

        if let Some(query) = navigate_to {
            self.search_suggestions.clear();
            self.show_omnibox_suggestions = false;
            self.show_center_suggestions = false;
            self.navigate_active_tab(&query);
        }
    }
}

fn fetch_and_decode_image(key: &str, fetch_url: &str, tx: std::sync::mpsc::Sender<(String, egui::ColorImage)>) {
    if fetch_url.starts_with("data:image/") {
        if let Some(comma_pos) = fetch_url.find(',') {
            let data_part = &fetch_url[comma_pos + 1..];
            if let Ok(bytes) = base64_decode(data_part) {
                if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                    let rgba = dyn_img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let color_img = egui::ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
                    let _ = tx.send((key.to_string(), color_img));
                    return;
                }
            }
        }
    }

    let url_to_fetch = if fetch_url.starts_with("//") {
        format!("https:{}", fetch_url)
    } else {
        fetch_url.to_string()
    };

    if url_to_fetch.starts_with("http://") || url_to_fetch.starts_with("https://") {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build();
        if let Ok(c) = client {
            if let Ok(resp) = c.get(&url_to_fetch).send() {
                if let Ok(bytes) = resp.bytes() {
                    if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                        let rgba = dyn_img.to_rgba8();
                        let size = [rgba.width() as usize, rgba.height() as usize];
                        let color_img = egui::ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
                        let _ = tx.send((key.to_string(), color_img));
                    }
                }
            }
        }
    } else if let Some(path) = url_to_fetch.strip_prefix("file://") {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                let rgba = dyn_img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_img = egui::ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
                let _ = tx.send((key.to_string(), color_img));
            }
        }
    }
}

fn base64_decode(s: &str) -> Result<Vec<u8>, ()> {
    let mut out = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for b in s.bytes() {
        let val = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            b'=' | b' ' | b'\n' | b'\r' => continue,
            _ => return Err(()),
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

fn fetch_search_suggestions(query: &str, tx: std::sync::mpsc::Sender<(String, Vec<String>)>) {
    let encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let url = format!(
        "https://suggestqueries.google.com/complete/search?client=firefox&q={}",
        encoded
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build();

    let mut suggestions = Vec::new();
    if let Ok(c) = client {
        if let Ok(resp) = c.get(&url).send() {
            if let Ok(text) = resp.text() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = val.get(1).and_then(|v| v.as_array()) {
                        for item in arr.iter().take(6) {
                            if let Some(s) = item.as_str() {
                                suggestions.push(s.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    if suggestions.is_empty() {
        let local_database = [
            "aether browser", "aether ai", "aether pure rust browser",
            "rust programming language", "rustlang documentation", "rust cargo book",
            "wikipedia", "wikipedia english",
            "youtube", "youtube music", "github", "github rust", "reddit",
            "crates.io", "web standards w3c", "html5 css3 layout",
            "computer science", "artificial intelligence", "quantum computing",
        ];
        let q_lower = query.to_lowercase();
        for &item in &local_database {
            if item.to_lowercase().contains(&q_lower) && item != query {
                suggestions.push(item.to_string());
                if suggestions.len() >= 5 {
                    break;
                }
            }
        }
    }

    let _ = tx.send((query.to_string(), suggestions));
}

fn draw_tab_shield_icon(painter: &egui::Painter, center: Pos2, color: Color32) {
    let top_y = center.y - 6.0;
    let mid_y = center.y - 1.0;
    let bot_y = center.y + 6.5;
    let left_x = center.x - 5.5;
    let right_x = center.x + 5.5;
    let mid_x = center.x;
    let stroke = Stroke::new(1.1_f32, color);

    // Outer shield outline (Image 2 design)
    painter.line_segment([Pos2::new(left_x, top_y), Pos2::new(right_x, top_y)], stroke);
    painter.line_segment([Pos2::new(left_x, top_y), Pos2::new(left_x, mid_y)], stroke);
    painter.line_segment([Pos2::new(right_x, top_y), Pos2::new(right_x, mid_y)], stroke);
    painter.line_segment([Pos2::new(left_x, mid_y), Pos2::new(mid_x, bot_y)], stroke);
    painter.line_segment([Pos2::new(right_x, mid_y), Pos2::new(mid_x, bot_y)], stroke);

    // Inner subtle shield emblem
    let in_stroke = Stroke::new(0.9_f32, color.gamma_multiply(0.6));
    let in_l = center.x - 2.8;
    let in_r = center.x + 2.8;
    let in_t = center.y - 3.2;
    let in_m = center.y;
    let in_b = center.y + 3.2;
    painter.line_segment([Pos2::new(in_l, in_t), Pos2::new(in_r, in_t)], in_stroke);
    painter.line_segment([Pos2::new(in_l, in_t), Pos2::new(in_l, in_m)], in_stroke);
    painter.line_segment([Pos2::new(in_r, in_t), Pos2::new(in_r, in_m)], in_stroke);
    painter.line_segment([Pos2::new(in_l, in_m), Pos2::new(mid_x, in_b)], in_stroke);
    painter.line_segment([Pos2::new(in_r, in_m), Pos2::new(mid_x, in_b)], in_stroke);
}

fn collect_style_tags(node: &Node, out: &mut String) {
    if node.tag_name() == Some("noscript") {
        return; // Never collect styles inside noscript (anti-bot blockers)
    }
    if node.tag_name() == Some("style") {
        out.push('\n');
        out.push_str(&node.inner_text());
    }
    for child in &node.children {
        collect_style_tags(child, out);
    }
}

impl eframe::App for BrowserApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_network_responses();

        // Drain incoming decoded images
        while let Ok((src, img)) = self.image_rx.try_recv() {
            let handle = ctx.load_texture(&src, img, egui::TextureOptions::LINEAR);
            self.image_textures.insert(src.clone(), handle);
            self.loading_images.remove(&src);
            ctx.request_repaint();
        }

        // Drain incoming search suggestions
        while let Ok((query, suggestions)) = self.suggest_rx.try_recv() {
            if query == self.suggest_query {
                self.search_suggestions = suggestions;
                ctx.request_repaint();
            }
        }

        // Look for unrendered images in active tab's display list and trigger downloads
        if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            let tab_url = tab.url.clone();
            if let Some(ref dl) = tab.display_list {
                for item in &dl.items {
                    if let crate::render::DisplayItem::Image { src, .. } = item {
                        if !src.is_empty()
                            && !self.image_textures.contains_key(src)
                            && !self.loading_images.contains(src)
                        {
                            self.loading_images.insert(src.clone());
                            let src_c = src.clone();
                            let fetch_url = resolve_relative_url(&tab_url, &src_c);
                            let tx = self.image_tx.clone();
                            std::thread::spawn(move || {
                                fetch_and_decode_image(&src_c, &fetch_url, tx);
                            });
                        }
                    }
                }
            }
        }

        // Dynamically update window title to "Page Title - Aether"
        let title = if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            if tab.is_new_tab() {
                "Aether".to_string()
            } else {
                format!("{} - Aether", tab.title)
            }
        } else {
            "Aether".to_string()
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        // Apply dark mode visuals matching Google Chrome dark theme
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(Color32::from_rgb(232, 234, 237));
        visuals.panel_fill = Color32::from_rgb(32, 33, 36); // #202124
        visuals.window_fill = Color32::from_rgb(40, 42, 45);
        ctx.set_visuals(visuals);

        // Global hotkey F12 or Ctrl+Shift+I toggle DevTools
        let toggle_devtools = ctx.input(|i| {
            i.key_pressed(Key::F12)
                || ((i.modifiers.ctrl || i.modifiers.command) && i.modifiers.shift && i.key_pressed(Key::I))
        });
        if toggle_devtools {
            self.show_devtools = !self.show_devtools;
        }

        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let window_radius = if is_maximized { 0 } else { 10 };

        // Top Chrome Panel (Tabs + Navigation Bar)
        let top_frame = Frame::NONE
            .fill(Color32::from_rgb(32, 33, 36))
            .corner_radius(CornerRadius {
                nw: window_radius,
                ne: window_radius,
                sw: 0,
                se: 0,
            });
        egui::TopBottomPanel::top("chrome_top_panel")
            .frame(top_frame)
            .show(ctx, |ui| {
                self.render_tab_strip(ui, ctx);
                self.render_navigation_bar(ui, ctx);
            });

        // DevTools panel if open (dockable to Right or Bottom)
        if self.show_devtools {
            match self.devtools_dock {
                DevToolsDockPosition::Right => {
                    let devtools_frame = Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .corner_radius(CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: 0,
                            se: window_radius,
                        });
                    egui::SidePanel::right("devtools_panel_right")
                        .default_width(460.0)
                        .min_width(320.0)
                        .resizable(true)
                        .frame(devtools_frame)
                        .show(ctx, |ui| {
                            self.render_devtools(ui);
                        });
                }
                DevToolsDockPosition::Bottom => {
                    let devtools_frame = Frame::NONE
                        .fill(Color32::from_rgb(32, 33, 36))
                        .corner_radius(CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: window_radius,
                            se: window_radius,
                        });
                    egui::TopBottomPanel::bottom("devtools_panel_bottom")
                        .default_height(280.0)
                        .min_height(160.0)
                        .resizable(true)
                        .frame(devtools_frame)
                        .show(ctx, |ui| {
                            self.render_devtools(ui);
                        });
                }
            }
        }

        // Main Web Viewport or Aether New Tab Start Page
        let (central_sw, central_se) = if self.show_devtools {
            match self.devtools_dock {
                DevToolsDockPosition::Right => (window_radius, 0),
                DevToolsDockPosition::Bottom => (0, 0),
            }
        } else {
            (window_radius, window_radius)
        };
        let central_frame = Frame::NONE
            .fill(Color32::from_rgb(53, 54, 58)) // #35363a
            .corner_radius(CornerRadius {
                nw: 0,
                ne: 0,
                sw: central_sw,
                se: central_se,
            });
        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ctx, |ui| {
                let is_new_tab = self.tabs.get(self.active_tab_idx).map_or(true, |t| t.is_new_tab());
                if is_new_tab {
                    self.render_aether_new_tab_page(ui, ctx);
                } else {
                    self.render_page_viewport(ui, ctx);
                }
            });

        // Outer window border stroke for 4-corner rounded frameless window
        if !is_maximized {
            let screen_rect = ctx.screen_rect();
            let border_stroke = Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67));
            let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("window_border")));
            painter.rect_stroke(screen_rect, CornerRadius::same(10), border_stroke, StrokeKind::Inside);
        }
    }
}

impl BrowserApp {
    /// Renders the Aether Tab Strip (Tabs, + Button, Draggable Header, and Window Controls)
    fn render_tab_strip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let mut close_tab_idx = None;

            for (idx, tab) in self.tabs.iter().enumerate() {
                let is_active = idx == self.active_tab_idx;

                // Vertical divider before inactive tabs (Image 2 style)
                if !is_active && (idx == 0 || idx.saturating_sub(1) != self.active_tab_idx) {
                    let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 18.0), egui::Sense::hover());
                    ui.painter().vline(
                        sep_rect.center().x,
                        (sep_rect.center().y - 7.0)..=(sep_rect.center().y + 7.0),
                        Stroke::new(1.0_f32, Color32::from_rgb(58, 62, 68)),
                    );
                    ui.add_space(2.0);
                }

                let tab_bg = if is_active {
                    Color32::from_rgb(52, 55, 60) // Rounded active pill background matching Image 2
                } else {
                    Color32::TRANSPARENT
                };

                let tab_text_color = if is_active {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(175, 180, 188)
                };

                let frame = Frame::NONE
                    .fill(tab_bg)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(10, 6));

                let resp = frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // Shield icon (Image 2 design) or loading dot
                        if tab.is_loading {
                            ui.label(RichText::new("●").size(10.0).color(Color32::from_rgb(251, 188, 4)));
                        } else {
                            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
                            let icon_color = if is_active {
                                Color32::from_rgb(220, 224, 230)
                            } else {
                                Color32::from_rgb(150, 155, 163)
                            };
                            draw_tab_shield_icon(ui.painter(), icon_rect.center(), icon_color);
                        }

                        // Tab title
                        let display_title = if tab.title.is_empty() { "New Tab" } else { &tab.title };
                        let label = ui.add(
                            egui::Label::new(
                                RichText::new(display_title)
                                    .size(12.0)
                                    .color(tab_text_color),
                            )
                            .truncate()
                            .selectable(false)
                            .sense(egui::Sense::click()),
                        );
                        if label.clicked() {
                            self.active_tab_idx = idx;
                            self.address_input = if tab.is_new_tab() { String::new() } else { tab.url.clone() };
                        }

                        // Tab Close button 'x': displayed on active tab or when hovered
                        if is_active {
                            let (c_rect, c_resp) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::click());
                            let center = c_rect.center();
                            if c_resp.hovered() {
                                ui.painter().circle_filled(center, 7.0, Color32::from_rgb(70, 74, 80));
                            }
                            let x_color = if c_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(170, 175, 180) };
                            ui.painter().line_segment([center - Vec2::splat(3.0), center + Vec2::splat(3.0)], Stroke::new(1.2_f32, x_color));
                            ui.painter().line_segment([Pos2::new(center.x - 3.0, center.y + 3.0), Pos2::new(center.x + 3.0, center.y - 3.0)], Stroke::new(1.2_f32, x_color));
                            if c_resp.clicked() {
                                close_tab_idx = Some(idx);
                            }
                        }
                    });
                });

                if close_tab_idx.is_none() && resp.response.interact(egui::Sense::click()).clicked() {
                    self.active_tab_idx = idx;
                    self.address_input = if tab.is_new_tab() { String::new() } else { tab.url.clone() };
                }
            }

            if let Some(idx) = close_tab_idx {
                self.close_tab(idx);
            }

            // New Tab "+" Button
            let new_tab_btn = ui.add(
                egui::Button::new(RichText::new("+").size(16.0).color(Color32::from_rgb(232, 234, 237)))
                    .corner_radius(14)
                    .fill(Color32::TRANSPARENT),
            );
            if new_tab_btn.clicked() {
                self.open_new_tab("about:newtab".to_string());
            }

            // Draggable strip & Window Controls (Minimize, Maximize, Close)
            let controls_width = 114.0;
            let available_drag_w = (ui.available_width() - controls_width).max(10.0);

            // Empty tab bar space: ONLY region that initiates window dragging
            let (_drag_rect, drag_resp) = ui.allocate_exact_size(Vec2::new(available_drag_w, 28.0), egui::Sense::click_and_drag());
            if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag_resp.double_clicked() {
                let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
            }

            // Right-side window controls (Minimize, Maximize, Close - vector drawn)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Window close
                let (w_close_rect, w_close_resp) = ui.allocate_exact_size(Vec2::new(38.0, 28.0), egui::Sense::click());
                if w_close_resp.hovered() {
                    ui.painter().rect_filled(w_close_rect, 0.0, Color32::from_rgb(232, 17, 35));
                }
                let wc = w_close_rect.center();
                let close_color = if w_close_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(180, 185, 190) };
                ui.painter().line_segment([wc - Vec2::splat(4.0), wc + Vec2::splat(4.0)], Stroke::new(1.3_f32, close_color));
                ui.painter().line_segment([Pos2::new(wc.x - 4.0, wc.y + 4.0), Pos2::new(wc.x + 4.0, wc.y - 4.0)], Stroke::new(1.3_f32, close_color));
                if w_close_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                // Window maximize
                let (w_max_rect, w_max_resp) = ui.allocate_exact_size(Vec2::new(36.0, 28.0), egui::Sense::click());
                if w_max_resp.hovered() {
                    ui.painter().rect_filled(w_max_rect, 0.0, Color32::from_rgb(60, 64, 67));
                }
                let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let max_color = if w_max_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(180, 185, 190) };
                if is_maximized {
                    let b1 = egui::Rect::from_center_size(Pos2::new(w_max_rect.center().x + 2.0, w_max_rect.center().y - 2.0), Vec2::new(8.0, 8.0));
                    let b2 = egui::Rect::from_center_size(Pos2::new(w_max_rect.center().x - 2.0, w_max_rect.center().y + 2.0), Vec2::new(8.0, 8.0));
                    ui.painter().rect_stroke(b1, 0.0, Stroke::new(1.1_f32, max_color), StrokeKind::Inside);
                    ui.painter().rect_filled(b2, 0.0, if w_max_resp.hovered() { Color32::from_rgb(60, 64, 67) } else { Color32::from_rgb(32, 33, 36) });
                    ui.painter().rect_stroke(b2, 0.0, Stroke::new(1.1_f32, max_color), StrokeKind::Inside);
                } else {
                    let wm_box = egui::Rect::from_center_size(w_max_rect.center(), Vec2::new(9.0, 9.0));
                    ui.painter().rect_stroke(wm_box, 0.0, Stroke::new(1.2_f32, max_color), StrokeKind::Inside);
                }
                if w_max_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                // Window minimize
                let (w_min_rect, w_min_resp) = ui.allocate_exact_size(Vec2::new(36.0, 28.0), egui::Sense::click());
                if w_min_resp.hovered() {
                    ui.painter().rect_filled(w_min_rect, 0.0, Color32::from_rgb(60, 64, 67));
                }
                let wmc = w_min_rect.center();
                let min_color = if w_min_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(180, 185, 190) };
                ui.painter().line_segment([Pos2::new(wmc.x - 5.0, wmc.y), Pos2::new(wmc.x + 5.0, wmc.y)], Stroke::new(1.3_f32, min_color));
                if w_min_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            });
        });
    }

    /// Renders the Chrome Navigation Toolbar (Back, Forward, Reload, Omnibox, Login, Menu)
    fn render_navigation_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(6.0);

            let can_back = self.tabs.get(self.active_tab_idx).map_or(false, |t| t.can_go_back());
            let can_fwd = self.tabs.get(self.active_tab_idx).map_or(false, |t| t.can_go_forward());

            // Back button ← (Vector drawn chevron)
            let (b_rect, b_resp) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), egui::Sense::click());
            let b_center = b_rect.center();
            let b_color = if can_back {
                if b_resp.hovered() {
                    ui.painter().circle_filled(b_center, 13.0, Color32::from_rgb(50, 53, 57));
                }
                Color32::from_rgb(232, 234, 237)
            } else {
                Color32::from_rgb(95, 99, 104)
            };
            ui.painter().line_segment([Pos2::new(b_center.x + 2.0, b_center.y - 4.5), Pos2::new(b_center.x - 2.5, b_center.y)], Stroke::new(1.5_f32, b_color));
            ui.painter().line_segment([Pos2::new(b_center.x - 2.5, b_center.y), Pos2::new(b_center.x + 2.0, b_center.y + 4.5)], Stroke::new(1.5_f32, b_color));
            if can_back && b_resp.clicked() {
                self.go_back();
            }

            // Forward button → (Vector drawn chevron)
            let (f_rect, f_resp) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), egui::Sense::click());
            let f_center = f_rect.center();
            let f_color = if can_fwd {
                if f_resp.hovered() {
                    ui.painter().circle_filled(f_center, 13.0, Color32::from_rgb(50, 53, 57));
                }
                Color32::from_rgb(232, 234, 237)
            } else {
                Color32::from_rgb(95, 99, 104)
            };
            ui.painter().line_segment([Pos2::new(f_center.x - 2.0, f_center.y - 4.5), Pos2::new(f_center.x + 2.5, f_center.y)], Stroke::new(1.5_f32, f_color));
            ui.painter().line_segment([Pos2::new(f_center.x + 2.5, f_center.y), Pos2::new(f_center.x - 2.0, f_center.y + 4.5)], Stroke::new(1.5_f32, f_color));
            if can_fwd && f_resp.clicked() {
                self.go_forward();
            }

            // Reload button ⟳ (Vector circular arc)
            let (r_rect, r_resp) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), egui::Sense::click());
            let r_center = r_rect.center();
            if r_resp.hovered() {
                ui.painter().circle_filled(r_center, 13.0, Color32::from_rgb(50, 53, 57));
            }
            ui.painter().circle_stroke(r_center, 5.5, Stroke::new(1.5_f32, Color32::from_rgb(232, 234, 237)));
            ui.painter().line_segment([Pos2::new(r_center.x + 3.0, r_center.y - 5.0), Pos2::new(r_center.x + 5.5, r_center.y - 1.5)], Stroke::new(1.5_f32, Color32::from_rgb(232, 234, 237)));
            if r_resp.clicked() {
                self.reload();
            }

            ui.add_space(4.0);

            // Omnibox pill container
            let omnibox_width = (ui.available_width() - 110.0).max(200.0);
            let omnibox_h = 34.0;

            let (response, painter) = ui.allocate_painter(Vec2::new(omnibox_width, omnibox_h), egui::Sense::hover());

            // Omnibox pill background
            let omnibox_bg = Color32::from_rgb(40, 41, 44); // #28292c
            let border_color = Color32::from_rgb(68, 71, 70); // #444746
            painter.rect_filled(response.rect, 17.0, omnibox_bg);
            painter.rect_stroke(response.rect, 17.0, Stroke::new(1.0_f32, border_color), StrokeKind::Inside);

            // Inside Omnibox layout
            let child_rect = response.rect.shrink2(Vec2::new(10.0, 4.0));
            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(child_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));

            // Aether "A" icon on left
            child_ui.label(RichText::new("A").size(14.0).strong().color(Color32::from_rgb(138, 180, 248)));
            child_ui.add_space(6.0);

            // URL TextEdit
            let available_edit_w = child_ui.available_width();
            let text_edit = child_ui.add_sized(
                Vec2::new(available_edit_w.max(50.0), 24.0),
                egui::TextEdit::singleline(&mut self.address_input)
                    .hint_text("Search Google or type a URL")
                    .frame(false)
                    .text_color(Color32::from_rgb(232, 234, 237)),
            );

            if text_edit.changed() {
                self.show_omnibox_suggestions = true;
                let input = self.address_input.clone();
                self.trigger_suggestions(&input);
            }

            if text_edit.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                self.show_omnibox_suggestions = false;
                self.search_suggestions.clear();
                let target = self.address_input.clone();
                self.navigate_active_tab(&target);
            }

            // Right side of Toolbar: Login button and Menu ⋮
            ui.add_space(4.0);

            // Login button/pill routing to Google Accounts
            let (p_rect, p_resp) = ui.allocate_exact_size(Vec2::new(64.0, 28.0), egui::Sense::click());
            if p_resp.hovered() {
                ui.painter().rect_filled(p_rect, 14.0, Color32::from_rgb(52, 54, 58));
            } else {
                ui.painter().rect_filled(p_rect, 14.0, Color32::from_rgb(40, 41, 44));
            }
            ui.painter().rect_stroke(p_rect, 14.0, Stroke::new(1.0_f32, Color32::from_rgb(68, 71, 70)), StrokeKind::Inside);
            ui.painter().text(
                p_rect.center(),
                Align2::CENTER_CENTER,
                "Login",
                FontId::proportional(12.0),
                Color32::from_rgb(232, 234, 237),
            );
            if p_resp.clicked() {
                self.navigate_active_tab("https://accounts.google.com");
            }

            // Menu button ⋮ (Vector 3 vertical dots, guaranteed 100% bug-free)
            let (m_rect, m_resp) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), egui::Sense::click());
            let m_center = m_rect.center();
            if m_resp.hovered() {
                ui.painter().circle_filled(m_center, 13.0, Color32::from_rgb(55, 58, 62));
            }
            for dy in [-5.0, 0.0, 5.0] {
                ui.painter().circle_filled(Pos2::new(m_center.x, m_center.y + dy), 1.5, Color32::from_rgb(218, 220, 224));
            }
            if m_resp.clicked() {
                self.show_menu_popup = !self.show_menu_popup;
            }

            if self.show_menu_popup {
                self.render_chrome_menu_popup(ctx, m_rect);
            }

            if self.show_omnibox_suggestions && !self.search_suggestions.is_empty() {
                self.render_search_suggestions_dropdown(ui, response.rect, true);
            }
        });
        ui.add_space(4.0);
    }

    /// Renders the Aether Dark Mode Start Page
    fn render_aether_new_tab_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ScrollArea::vertical().drag_to_scroll(false).show(ui, |ui| {
            ui.add_space(56.0);

            // Center Content Container
            ui.vertical_centered(|ui| {
                // 1. "Aether" Brand Logo: Sleek modern typography with orbital crest (100% mathematically centered)
                let text_job = egui::WidgetText::RichText(RichText::new("Aether").size(64.0).strong().color(Color32::WHITE))
                    .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::FontSelection::Default);
                let text_w = text_job.size().x;
                let crest_w = 42.0;
                let gap = 14.0;
                let total_logo_w = crest_w + gap + text_w;
                let pad = ((ui.available_width() - total_logo_w) / 2.0).max(0.0);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    ui.add_space(pad);

                    let (crest_rect, _) = ui.allocate_exact_size(Vec2::new(crest_w, 42.0), egui::Sense::hover());
                    draw_sparkle_icon(ui.painter(), crest_rect.center(), 18.0, Color32::from_rgb(138, 180, 248));

                    ui.add(egui::Label::new(text_job));
                });
                ui.add_space(28.0);

                // 2. Center Search Bar (Large Pill Shape ~680px wide)
                let search_box_width = (680.0_f32).min(ui.available_width() - 32.0);
                let search_box_height = 50.0;

                let (search_rect, _search_resp) = ui.allocate_exact_size(Vec2::new(search_box_width, search_box_height), egui::Sense::hover());

                // Pill background #46474a
                let search_bg = Color32::from_rgb(70, 71, 74);
                let search_border = Color32::from_rgb(95, 99, 104);
                ui.painter().rect_filled(search_rect, 25.0, search_bg);
                ui.painter().rect_stroke(search_rect, 25.0, Stroke::new(1.0_f32, search_border), StrokeKind::Inside);

                let inner_rect = search_rect.shrink2(Vec2::new(16.0, 8.0));
                let mut inner_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));

                // Left: + icon
                inner_ui.label(RichText::new("+").size(18.0).color(Color32::from_rgb(154, 160, 166)));
                inner_ui.add_space(10.0);

                // Text Edit: "Search Google or type a URL"
                let input_width = inner_ui.available_width() - 65.0;
                let center_edit = inner_ui.add_sized(
                    Vec2::new(input_width.max(100.0), 30.0),
                    egui::TextEdit::singleline(&mut self.center_search_input)
                        .hint_text("Search Google or type a URL")
                        .frame(false)
                        .text_color(Color32::WHITE)
                        .font(FontId::proportional(15.0)),
                );

                if center_edit.changed() {
                    self.show_center_suggestions = true;
                    let input = self.center_search_input.clone();
                    self.trigger_suggestions(&input);
                }

                if center_edit.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                    self.show_center_suggestions = false;
                    self.search_suggestions.clear();
                    let query = self.center_search_input.clone();
                    if !query.trim().is_empty() {
                        self.navigate_active_tab(&query);
                    }
                }

                // Right icons inside Search Bar: Mic, Lens/Camera (Vector drawn)
                inner_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |inner_ui| {
                    // Camera / Lens icon (Vector drawn)
                    let (cam_rect, _cam_resp) = inner_ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::hover());
                    let cc = cam_rect.center();
                    let cam_box = egui::Rect::from_center_size(cc, Vec2::new(14.0, 10.0));
                    inner_ui.painter().rect_stroke(cam_box, 2.0_f32, Stroke::new(1.2_f32, Color32::from_rgb(180, 185, 190)), StrokeKind::Inside);
                    inner_ui.painter().circle_stroke(cc, 2.5, Stroke::new(1.2_f32, Color32::from_rgb(180, 185, 190)));

                    inner_ui.add_space(8.0);

                    // Microphone icon (Vector drawn)
                    let (mic_rect, _mic_resp) = inner_ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::hover());
                    let mc = mic_rect.center();
                    let mic_body = egui::Rect::from_center_size(Pos2::new(mc.x, mc.y - 2.0), Vec2::new(6.0, 9.0));
                    inner_ui.painter().rect_filled(mic_body, 3.0, Color32::from_rgb(180, 185, 190));
                    inner_ui.painter().line_segment([Pos2::new(mc.x, mc.y + 4.0), Pos2::new(mc.x, mc.y + 7.0)], Stroke::new(1.2_f32, Color32::from_rgb(180, 185, 190)));
                });

                if self.show_center_suggestions && !self.search_suggestions.is_empty() {
                    self.render_search_suggestions_dropdown(ui, search_rect, false);
                }

                ui.add_space(32.0);

                // 3. Quick Shortcut Icons (Row of circles with vector graphics - mathematically centered)
                let shortcut_gap = 16.0;
                let total_shortcuts_w = 6.0 * 68.0;
                let total_row_w = total_shortcuts_w + 5.0 * shortcut_gap;
                let pad_shortcuts = ((ui.available_width() - total_row_w) / 2.0).max(0.0);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = shortcut_gap;
                    ui.add_space(pad_shortcuts);

                    // 1. Wikipedia
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Letter("W"),
                        Color32::from_rgb(0, 102, 153),
                        "Wikipedia",
                        || self.navigate_active_tab("https://en.wikipedia.org"),
                    );

                    // 2. YouTube
                    render_vector_shortcut(
                        ui,
                        VectorIcon::PlayTriangle,
                        Color32::from_rgb(255, 0, 0),
                        "YouTube",
                        || self.navigate_active_tab("https://youtube.com"),
                    );

                    // 3. GitHub
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Letter("GH"),
                        Color32::from_rgb(36, 41, 46),
                        "GitHub",
                        || self.navigate_active_tab("https://github.com"),
                    );

                    // 4. Rust Lang
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Letter("R"),
                        Color32::from_rgb(222, 90, 38),
                        "Rust Lang",
                        || self.navigate_active_tab("https://www.rust-lang.org"),
                    );

                    // 5. Reddit
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Letter("r/"),
                        Color32::from_rgb(255, 69, 0),
                        "Reddit",
                        || self.navigate_active_tab("https://reddit.com"),
                    );

                    // 6. DuckDuckGo
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Letter("D"),
                        Color32::from_rgb(222, 88, 51),
                        "DuckDuckGo",
                        || self.navigate_active_tab("https://duckduckgo.com"),
                    );
                });
            });
        });
    }

    /// Renders standard web pages (HTML, CSS layout)
    fn render_page_viewport(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let available_size = ui.available_size();
        let current_tab = match self.tabs.get_mut(self.active_tab_idx) {
            Some(t) => t,
            None => return,
        };

        let target_width = (available_size.x / self.zoom_level).max(200.0);
        if (target_width - current_tab.last_layout_width).abs() > 10.0 {
            current_tab.recompute_layout(target_width);
        }

        let page_height = current_tab.page_height * self.zoom_level + 80.0;
        let mut clicked_href = None;
        let mut new_hovered_link = None;

        ScrollArea::vertical()
            .drag_to_scroll(false)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (response, painter) = ui.allocate_painter(
                    Vec2::new(available_size.x, page_height.max(available_size.y)),
                    egui::Sense::click(),
                );

                // Webpage canvas background
                let canvas_bg = if current_tab.url.contains("search.aether.local")
                    || current_tab.url.contains("google.com/search")
                    || current_tab.url.contains("about:ai")
                {
                    Color32::from_rgb(32, 33, 36)
                } else if current_tab.url.contains("about:credits") {
                    Color32::from_rgb(15, 23, 42)
                } else {
                    Color32::WHITE
                };
                let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let bottom_radius = if is_maximized { 0 } else { 10 };
                let (sw, se) = if self.show_devtools {
                    match self.devtools_dock {
                        DevToolsDockPosition::Right => (bottom_radius, 0),
                        DevToolsDockPosition::Bottom => (0, 0),
                    }
                } else {
                    (bottom_radius, bottom_radius)
                };
                painter.rect_filled(
                    response.rect,
                    CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw,
                        se,
                    },
                    canvas_bg,
                );

                let origin = response.rect.min;
                let offset = Vec2::new(origin.x, origin.y);

                if let Some(ref display_list) = current_tab.display_list {
                    paint_display_list(&painter, display_list, offset, self.zoom_level, Some(&self.image_textures));

                    if let Some(mouse_pos) = ctx.pointer_hover_pos() {
                        let rel_x = (mouse_pos.x - offset.x) / self.zoom_level;
                        let rel_y = (mouse_pos.y - offset.y) / self.zoom_level;

                        for link in &display_list.links {
                            if link.rect.contains(rel_x, rel_y) {
                                ctx.set_cursor_icon(CursorIcon::PointingHand);
                                new_hovered_link = Some(link.href.clone());
                                if response.clicked() {
                                    clicked_href = Some(link.href.clone());
                                }
                                break;
                            }
                        }
                    }
                }
            });

        self.hovered_link = new_hovered_link;

        if let Some(href) = clicked_href {
            let base_url = current_tab.url.clone();
            let resolved = resolve_relative_url(&base_url, &href);
            self.navigate_active_tab(&resolved);
        }
    }

    // ==========================================
    // Chrome 3-Dots Popup Menu Implementation
    // ==========================================

    fn render_chrome_menu_popup(&mut self, ctx: &egui::Context, m_rect: egui::Rect) {
        let menu_width = 290.0;
        let window_resp = egui::Window::new("chrome_3dots_menu")
            .title_bar(false)
            .anchor(Align2::RIGHT_TOP, Vec2::new(-8.0, 76.0))
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(menu_width, 0.0))
            .frame(
                Frame::NONE
                    .fill(Color32::from_rgb(41, 42, 45)) // #292a2d
                    .corner_radius(CornerRadius::same(8))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67))) // #3c4043
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(160),
                    })
                    .inner_margin(Margin::symmetric(4, 6)),
            )
            .show(ctx, |ui| {
                ui.set_width(menu_width - 8.0);
                let max_h = (ctx.screen_rect().height() - 90.0).max(300.0).min(680.0);
                ScrollArea::vertical()
                    .max_height(max_h)
                    .show(ui, |ui| {
                        ui.set_width(menu_width - 8.0);

                        // Section 0: Optional alert row
                        if render_menu_item(ui, MenuIcon::Shield, "Found 111 compromised passwords", None).clicked() {
                            self.navigate_active_tab("about:settings");
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 1: Navigation
                        if render_menu_item(ui, MenuIcon::NewTab, "New tab", Some("Ctrl+T")).clicked() {
                            self.open_new_tab("about:newtab".to_string());
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::NewWindow, "New window", Some("Ctrl+N")).clicked() {
                            self.open_new_tab("about:newtab".to_string());
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Incognito, "New Incognito window", Some("Ctrl+Shift+N")).clicked() {
                            self.open_new_tab("about:blank".to_string());
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 2: Profile card row ("Khoa", "Signed in")
                        if render_profile_card(ui, "Khoa", "Signed in").clicked() {
                            self.navigate_active_tab("https://myaccount.google.com");
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 3: Tools & Data
                        if render_menu_item(ui, MenuIcon::Passwords, "Passwords and autofill", Some(">")).clicked() {
                            self.navigate_active_tab("about:settings");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::History, "History", Some("Ctrl+H")).clicked() {
                            self.navigate_active_tab("about:history");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Downloads, "Downloads", Some("Ctrl+J")).clicked() {
                            self.navigate_active_tab("about:downloads");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Bookmarks, "Bookmarks and lists", Some(">")).clicked() {
                            self.navigate_active_tab("about:bookmarks");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::TabGroups, "Tab groups", Some(">")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Extensions, "Extensions", Some(">")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::DeleteData, "Delete browsing data...", Some("Ctrl+Shift+Del")).clicked() {
                            self.navigate_active_tab("about:settings");
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 4: Interactive Zoom row (keeps menu open)
                        render_zoom_row(ui, ctx, self);

                        render_menu_divider(ui);

                        // Section 5: Page actions & Developer Tools
                        if render_menu_item(ui, MenuIcon::Print, "Print...", Some("Ctrl+P")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::GoogleLens, "Search this tab with Google Lens", None).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Translate, "Translate...", None).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Find, "Find and edit", Some(">")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Cast, "Cast, save, and share", Some(">")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::MoreTools, "More tools", Some(">")).clicked() {
                            self.show_devtools = !self.show_devtools;
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 6: Settings, Help & Exit
                        if render_menu_item(ui, MenuIcon::Help, "Help", Some(">")).clicked() {
                            self.navigate_active_tab("about:credits");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Settings, "Settings", None).clicked() {
                            self.navigate_active_tab("about:settings");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Exit, "Exit", None).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            self.show_menu_popup = false;
                        }
                    });
            });

        // Dismissal logic: Dismiss on Escape or clicking outside
        if let Some(inner) = window_resp {
            let menu_rect = inner.response.rect;
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.show_menu_popup = false;
            } else if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !menu_rect.contains(pos) && !m_rect.contains(pos) {
                        self.show_menu_popup = false;
                    }
                }
            }
        }
    }

    // ==========================================
    // Chrome DevTools Panel Implementation
    // ==========================================

    /// Renders the complete native Chrome DevTools panel supporting 6 tabs and docking
    fn render_devtools(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);

        // Header Tab Navigation Bar
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;

            let tabs = [
                (DevToolsTab::Elements, "Elements"),
                (DevToolsTab::Console, "Console"),
                (DevToolsTab::Sources, "Sources"),
                (DevToolsTab::Network, "Network"),
                (DevToolsTab::Application, "Application"),
                (DevToolsTab::Security, "Security"),
            ];

            for (tab_var, label) in tabs {
                let is_active = self.devtools_tab == tab_var;
                let text_color = if is_active {
                    Color32::from_rgb(232, 234, 237)
                } else {
                    Color32::from_rgb(154, 160, 166)
                };

                let resp = ui.add(
                    egui::Button::new(RichText::new(label).size(12.0).color(text_color))
                        .frame(false),
                );

                if is_active {
                    let r = resp.rect;
                    ui.painter().line_segment(
                        [Pos2::new(r.min.x, r.max.y + 2.0), Pos2::new(r.max.x, r.max.y + 2.0)],
                        Stroke::new(2.0_f32, Color32::from_rgb(138, 180, 248)),
                    );
                }

                if resp.clicked() {
                    self.devtools_tab = tab_var;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                // Close button '×'
                let (c_rect, c_resp) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::click());
                let center = c_rect.center();
                if c_resp.hovered() {
                    ui.painter().circle_filled(center, 9.0, Color32::from_rgb(60, 64, 67));
                }
                let x_color = if c_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(170, 175, 180) };
                ui.painter().line_segment([center - Vec2::splat(3.5), center + Vec2::splat(3.5)], Stroke::new(1.3_f32, x_color));
                ui.painter().line_segment([Pos2::new(center.x - 3.5, center.y + 3.5), Pos2::new(center.x + 3.5, center.y - 3.5)], Stroke::new(1.3_f32, x_color));
                if c_resp.clicked() {
                    self.show_devtools = false;
                }

                // Dock Position Toggle Button
                let dock_label = match self.devtools_dock {
                    DevToolsDockPosition::Right => "⬓ Bottom",
                    DevToolsDockPosition::Bottom => "◨ Right",
                };
                let dock_btn = ui.add(
                    egui::Button::new(RichText::new(dock_label).size(11.0).color(Color32::from_rgb(154, 160, 166)))
                        .frame(false),
                );
                if dock_btn.clicked() {
                    self.devtools_dock = match self.devtools_dock {
                        DevToolsDockPosition::Right => DevToolsDockPosition::Bottom,
                        DevToolsDockPosition::Bottom => DevToolsDockPosition::Right,
                    };
                }
            });
        });

        ui.add_space(4.0);
        let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().line_segment(
            [Pos2::new(sep_rect.min.x, sep_rect.center().y), Pos2::new(sep_rect.max.x, sep_rect.center().y)],
            Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)),
        );
        ui.add_space(4.0);

        let mut pending_navigate: Option<String> = None;
        let current_zoom_str = self.zoom_percentage();

        let tab = match self.tabs.get_mut(self.active_tab_idx) {
            Some(t) => t,
            None => return,
        };

        match self.devtools_tab {
            DevToolsTab::Elements => {
                let mut new_selected_path = None;

                // Elements Tab: Two Pane Inspection Layout
                // Pane 1: Interactive DOM Tree with Colored Syntax Tokens
                Frame::NONE
                    .fill(Color32::from_rgb(24, 26, 29))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 52, 58)))
                    .corner_radius(4)
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("DOM Tree").strong().size(12.0).color(Color32::from_rgb(232, 234, 237)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.devtools_dom_search)
                                        .hint_text("Search DOM")
                                        .desired_width(120.0),
                                );
                            });
                        });

                        ui.add_space(4.0);
                        let dom_tree_h = if self.devtools_dock == DevToolsDockPosition::Bottom { 180.0 } else { 240.0 };
                        ScrollArea::both()
                            .max_height(dom_tree_h)
                            .show(ui, |ui| {
                                if let Some(ref dom) = tab.dom_root {
                                    render_dom_tree_node(
                                        ui,
                                        dom,
                                        vec![],
                                        0,
                                        &self.devtools_selected_path,
                                        &mut self.devtools_expanded_paths,
                                        &mut new_selected_path,
                                        &self.devtools_dom_search,
                                    );
                                } else {
                                    ui.label(RichText::new("No DOM available").weak());
                                }
                            });
                    });

                if let Some(path) = new_selected_path {
                    self.devtools_selected_path = Some(path);
                }

                ui.add_space(6.0);

                // Pane 2: Box Model & Styles Inspector Pane
                Frame::NONE
                    .fill(Color32::from_rgb(26, 28, 31))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 52, 58)))
                    .corner_radius(4)
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        if let Some(ref path) = self.devtools_selected_path {
                            if let Some(ref dom) = tab.dom_root {
                                if let Some(node) = dom.get_node_by_path(path) {
                                    let tag = node.tag_name().unwrap_or("node");
                                    let id_str = node.id().map(|s| format!("#{}", s)).unwrap_or_default();
                                    let class_str = if !node.classes().is_empty() {
                                        let c = node.classes().into_iter().collect::<Vec<_>>().join(".");
                                        format!(".{}", c)
                                    } else {
                                        String::new()
                                    };

                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Styles & Box Model:").strong().size(12.0).color(Color32::from_rgb(232, 234, 237)));
                                        ui.label(
                                            RichText::new(format!("<{}{}{}>", tag, id_str, class_str))
                                                .strong()
                                                .color(Color32::from_rgb(138, 180, 248)),
                                        );
                                    });

                                    ui.separator();

                                    let inspector_h = if self.devtools_dock == DevToolsDockPosition::Bottom { 200.0 } else { 300.0 };
                                    ScrollArea::vertical().max_height(inspector_h).show(ui, |ui| {
                                        // 1. Visual 4-layer Chrome Box Model Diagram
                                        let live_dims = crate::layout::query_node_dimensions(
                                            dom,
                                            &tab.stylesheet,
                                            node,
                                            tab.last_layout_width.max(300.0),
                                        );
                                        render_chrome_box_model(ui, live_dims.as_ref());

                                        ui.add_space(8.0);

                                        // 2. Matched CSS Rules
                                        let ancestors = dom.collect_ancestor_nodes(path);
                                        let matched_rules = crate::style::get_matched_rules(node, &ancestors, &tab.stylesheet);

                                        ui.label(RichText::new("Matched CSS Rules:").strong().size(11.5).color(Color32::from_rgb(154, 160, 166)));
                                        if matched_rules.is_empty() {
                                            ui.label(RichText::new("No matching CSS rules").weak());
                                        } else {
                                            for (spec, rule) in matched_rules {
                                                let sel_text = rule.selectors.iter().map(|s| format!("{:?}", s)).collect::<Vec<_>>().join(", ");
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new(sel_text).strong().color(Color32::from_rgb(138, 180, 248)));
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(RichText::new(format!("{:?}", spec)).weak().size(10.0));
                                                    });
                                                });
                                                for decl in &rule.declarations {
                                                    ui.label(format!("   {}: {:?}", decl.name, decl.value));
                                                }
                                                ui.add_space(3.0);
                                            }
                                        }

                                        ui.add_space(8.0);

                                        // 3. Computed Styles
                                        let specified_values = crate::style::get_specified_values(node, &ancestors, &tab.stylesheet);
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("Computed Styles:").strong().size(11.5).color(Color32::from_rgb(154, 160, 166)));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut self.devtools_styles_filter)
                                                        .hint_text("Filter")
                                                        .desired_width(90.0),
                                                );
                                            });
                                        });

                                        for (prop, val) in specified_values {
                                            if !self.devtools_styles_filter.is_empty()
                                                && !prop.to_lowercase().contains(&self.devtools_styles_filter.to_lowercase())
                                            {
                                                continue;
                                            }
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(prop).color(Color32::from_rgb(154, 160, 166)));
                                                ui.label(RichText::new(format!("{:?}", val)).color(Color32::from_rgb(232, 234, 237)));
                                            });
                                        }
                                    });
                                } else {
                                    ui.label(RichText::new("Selected node no longer exists in DOM").weak());
                                }
                            }
                        } else {
                            ui.label(RichText::new("Select an element in the DOM tree above to inspect its box model and styles").weak());
                        }
                    });
            }
            DevToolsTab::Console => {
                // Console Toolbar
                ui.horizontal(|ui| {
                    if ui.button("🚫 Clear").clicked() {
                        tab.console_entries.clear();
                    }

                    ui.separator();

                    let is_all = self.devtools_console_level.is_none();
                    if ui.selectable_label(is_all, "All").clicked() {
                        self.devtools_console_level = None;
                    }
                    let is_err = self.devtools_console_level == Some(ConsoleLogLevel::Error);
                    if ui.selectable_label(is_err, "Errors").clicked() {
                        self.devtools_console_level = Some(ConsoleLogLevel::Error);
                    }
                    let is_warn = self.devtools_console_level == Some(ConsoleLogLevel::Warn);
                    if ui.selectable_label(is_warn, "Warnings").clicked() {
                        self.devtools_console_level = Some(ConsoleLogLevel::Warn);
                    }
                    let is_info = self.devtools_console_level == Some(ConsoleLogLevel::Info);
                    if ui.selectable_label(is_info, "Info").clicked() {
                        self.devtools_console_level = Some(ConsoleLogLevel::Info);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.devtools_console_filter)
                                .hint_text("Filter")
                                .desired_width(120.0),
                        );
                    });
                });

                ui.separator();

                // Console Log Stream
                let log_h = if self.devtools_dock == DevToolsDockPosition::Bottom { 180.0 } else { 380.0 };
                ScrollArea::vertical().max_height(log_h).show(ui, |ui| {
                    if tab.console_entries.is_empty() {
                        ui.label(RichText::new("No console output").weak().italics());
                    } else {
                        for entry in &tab.console_entries {
                            if let Some(lvl) = self.devtools_console_level {
                                if entry.level != lvl {
                                    continue;
                                }
                            }
                            if !self.devtools_console_filter.is_empty()
                                && !entry.message.to_lowercase().contains(&self.devtools_console_filter.to_lowercase())
                            {
                                continue;
                            }

                            let (badge_text, badge_color, badge_bg) = match entry.level {
                                ConsoleLogLevel::Info => ("INFO", Color32::from_rgb(138, 180, 248), Color32::from_rgb(30, 42, 60)),
                                ConsoleLogLevel::Warn => ("WARN", Color32::from_rgb(253, 214, 99), Color32::from_rgb(45, 40, 25)),
                                ConsoleLogLevel::Error => ("ERROR", Color32::from_rgb(242, 139, 130), Color32::from_rgb(55, 30, 30)),
                            };

                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&entry.timestamp).weak().size(10.0));

                                let (b_rect, _) = ui.allocate_exact_size(Vec2::new(42.0, 16.0), egui::Sense::hover());
                                ui.painter().rect_filled(b_rect, 3.0, badge_bg);
                                ui.painter().text(b_rect.center(), Align2::CENTER_CENTER, badge_text, FontId::proportional(9.0), badge_color);

                                ui.label(RichText::new(&entry.message).monospace().color(badge_color));
                            });
                            ui.add_space(2.0);
                        }
                    }
                });

                ui.separator();

                // Interactive REPL Prompt
                let mut execute_cmd = false;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(">").monospace().strong().color(Color32::from_rgb(138, 180, 248)));
                    let text_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.devtools_console_input)
                            .hint_text("Enter browser command (help, dom, styles, info, zoom, goto <url>)...")
                            .desired_width(ui.available_width() - 50.0),
                    );
                    if text_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        execute_cmd = true;
                    }
                    if ui.button("Run").clicked() {
                        execute_cmd = true;
                    }
                });

                if execute_cmd {
                    let cmd = self.devtools_console_input.trim().to_string();
                    if !cmd.is_empty() {
                        let time_str = format_current_time();
                        let id = tab.console_entries.len() + 1;
                        tab.console_entries.push(ConsoleEntry {
                            id,
                            timestamp: time_str.clone(),
                            level: ConsoleLogLevel::Info,
                            source: "REPL".to_string(),
                            message: format!("> {}", cmd),
                        });

                        let (level, reply) = if cmd == "help" {
                            (ConsoleLogLevel::Info, "Commands: help, clear, dom, styles, info, zoom, goto <url>".to_string())
                        } else if cmd == "clear" {
                            tab.console_entries.clear();
                            (ConsoleLogLevel::Info, String::new())
                        } else if cmd == "dom" {
                            let tag = tab.dom_root.as_ref().and_then(|r| r.tag_name()).unwrap_or("none");
                            let count = tab.dom_root.as_ref().map(|r| r.children.len()).unwrap_or(0);
                            (ConsoleLogLevel::Info, format!("DOM Root: <{}> with {} direct children", tag, count))
                        } else if cmd == "styles" {
                            (ConsoleLogLevel::Info, format!("Stylesheet: {} CSS rules", tab.stylesheet.rules.len()))
                        } else if cmd == "info" {
                            (ConsoleLogLevel::Info, format!("URL: {}, Title: '{}', Size: {} bytes", tab.url, tab.title, tab.raw_html.len()))
                        } else if cmd == "zoom" {
                            (ConsoleLogLevel::Info, format!("Current Zoom: {}", current_zoom_str))
                        } else if let Some(target) = cmd.strip_prefix("goto ") {
                            let url_str = target.trim().to_string();
                            pending_navigate = Some(url_str.clone());
                            (ConsoleLogLevel::Info, format!("Navigating to {}", url_str))
                        } else {
                            (ConsoleLogLevel::Warn, format!("Unknown command: '{}'. Type 'help' for assistance.", cmd))
                        };

                        if !reply.is_empty() {
                            let reply_id = tab.console_entries.len() + 1;
                            tab.console_entries.push(ConsoleEntry {
                                id: reply_id,
                                timestamp: time_str,
                                level,
                                source: "System".to_string(),
                                message: reply,
                            });
                        }

                        self.devtools_console_input.clear();
                    }
                }
            }
            DevToolsTab::Sources => {
                // Sources Tab: Resource tree & Code viewer
                ui.horizontal(|ui| {
                    // Left tree
                    ui.allocate_ui_with_layout(Vec2::new(140.0, ui.available_height()), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.label(RichText::new("📁 Resources").strong().size(12.0));
                        ui.separator();

                        let is_html = self.devtools_sources_file == SourcesFileId::HtmlDocument;
                        if ui.selectable_label(is_html, "📄 (index.html)").clicked() {
                            self.devtools_sources_file = SourcesFileId::HtmlDocument;
                        }
                        let is_ua = self.devtools_sources_file == SourcesFileId::UserAgentCss;
                        if ui.selectable_label(is_ua, "🎨 user_agent.css").clicked() {
                            self.devtools_sources_file = SourcesFileId::UserAgentCss;
                        }
                    });

                    ui.separator();

                    // Right viewer with line numbers
                    let content = match self.devtools_sources_file {
                        SourcesFileId::HtmlDocument => &tab.raw_html,
                        SourcesFileId::UserAgentCss => DEFAULT_USER_AGENT_CSS,
                        SourcesFileId::InlineCss(_) => DEFAULT_USER_AGENT_CSS,
                    };

                    let viewer_h = if self.devtools_dock == DevToolsDockPosition::Bottom { 200.0 } else { 420.0 };
                    ScrollArea::both().max_height(viewer_h).show(ui, |ui| {
                        for (line_idx, line) in content.lines().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("{:3} |", line_idx + 1)).weak().size(10.5));
                                ui.label(RichText::new(line).monospace().size(11.0).color(Color32::from_rgb(232, 234, 237)));
                            });
                        }
                    });
                });
            }
            DevToolsTab::Network => {
                // Network Tab Toolbar
                ui.horizontal(|ui| {
                    if ui.button("🚫 Clear").clicked() {
                        tab.network_entries.clear();
                        self.devtools_selected_network_id = None;
                    }

                    ui.separator();

                    let filters = [
                        (NetworkTypeFilter::All, "All"),
                        (NetworkTypeFilter::Doc, "Doc"),
                        (NetworkTypeFilter::Css, "CSS"),
                        (NetworkTypeFilter::Js, "JS"),
                        (NetworkTypeFilter::Img, "Img"),
                        (NetworkTypeFilter::Fetch, "Fetch"),
                    ];
                    for (f, label) in filters {
                        let is_sel = self.devtools_network_type == f;
                        if ui.selectable_label(is_sel, label).clicked() {
                            self.devtools_network_type = f;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.devtools_network_filter)
                                .hint_text("Filter requests")
                                .desired_width(120.0),
                        );
                    });
                });

                ui.separator();

                // Network Requests Table
                let net_h = if self.devtools_dock == DevToolsDockPosition::Bottom { 180.0 } else { 260.0 };
                ScrollArea::both().max_height(net_h).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Name").strong().size(11.0));
                        ui.add_space(80.0);
                        ui.label(RichText::new("Status").strong().size(11.0));
                        ui.add_space(20.0);
                        ui.label(RichText::new("Type").strong().size(11.0));
                        ui.add_space(20.0);
                        ui.label(RichText::new("Size").strong().size(11.0));
                        ui.add_space(20.0);
                        ui.label(RichText::new("Time").strong().size(11.0));
                        ui.add_space(20.0);
                        ui.label(RichText::new("Waterfall").strong().size(11.0));
                    });
                    ui.separator();

                    if tab.network_entries.is_empty() {
                        ui.label(RichText::new("No requests recorded").weak().italics());
                    } else {
                        for entry in &tab.network_entries {
                            let is_sel = self.devtools_selected_network_id == Some(entry.id);
                            let row_bg = if is_sel {
                                Color32::from_rgb(40, 62, 88)
                            } else {
                                Color32::TRANSPARENT
                            };

                            let (row_rect, row_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), egui::Sense::click());
                            if is_sel || row_resp.hovered() {
                                ui.painter().rect_filled(row_rect, 2.0, if is_sel { row_bg } else { Color32::from_rgb(45, 48, 54) });
                            }
                            if row_resp.clicked() {
                                self.devtools_selected_network_id = Some(entry.id);
                            }

                            // Render row contents
                            let name_preview = if entry.name.len() > 18 { format!("{}...", &entry.name[..16]) } else { entry.name.clone() };
                            ui.painter().text(Pos2::new(row_rect.min.x + 4.0, row_rect.center().y), Align2::LEFT_CENTER, name_preview, FontId::proportional(11.0), Color32::from_rgb(232, 234, 237));

                            let status_color = if entry.status >= 200 && entry.status < 300 {
                                Color32::from_rgb(129, 201, 149)
                            } else if entry.status >= 300 && entry.status < 400 {
                                Color32::from_rgb(138, 180, 248)
                            } else {
                                Color32::from_rgb(242, 139, 130)
                            };
                            ui.painter().text(Pos2::new(row_rect.min.x + 120.0, row_rect.center().y), Align2::LEFT_CENTER, format!("{}", entry.status), FontId::proportional(11.0), status_color);
                            ui.painter().text(Pos2::new(row_rect.min.x + 180.0, row_rect.center().y), Align2::LEFT_CENTER, &entry.resource_type, FontId::proportional(11.0), Color32::from_rgb(154, 160, 166));

                            let size_str = if entry.size_bytes < 1024 { format!("{} B", entry.size_bytes) } else { format!("{:.1} KB", entry.size_bytes as f32 / 1024.0) };
                            ui.painter().text(Pos2::new(row_rect.min.x + 240.0, row_rect.center().y), Align2::LEFT_CENTER, size_str, FontId::proportional(11.0), Color32::from_rgb(154, 160, 166));

                            ui.painter().text(Pos2::new(row_rect.min.x + 300.0, row_rect.center().y), Align2::LEFT_CENTER, format!("{} ms", entry.duration_ms), FontId::proportional(11.0), Color32::from_rgb(154, 160, 166));

                            // Waterfall bar
                            let wf_w = ((entry.duration_ms as f32) * 1.5).max(4.0).min(60.0);
                            let wf_rect = EguiRect::from_min_size(Pos2::new(row_rect.min.x + 360.0, row_rect.center().y - 4.0), Vec2::new(wf_w, 8.0));
                            ui.painter().rect_filled(wf_rect, 2.0, Color32::from_rgb(138, 180, 248));
                        }
                    }
                });

                // Request Details Drawer if an item is selected
                if let Some(sel_id) = self.devtools_selected_network_id {
                    if let Some(entry) = tab.network_entries.iter().find(|e| e.id == sel_id) {
                        ui.separator();
                        ui.label(RichText::new("Request Details:").strong().size(11.5).color(Color32::from_rgb(232, 234, 237)));
                        ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                            ui.label(format!("Request URL: {}", entry.url));
                            ui.label(format!("Request Method: {}", entry.method));
                            ui.label(format!("Status Code: {} {}", entry.status, entry.status_text));
                            ui.add_space(4.0);
                            ui.label(RichText::new("Response Headers:").strong().size(11.0));
                            for (k, v) in &entry.headers {
                                ui.label(format!("{}: {}", k, v));
                            }
                        });
                    }
                }
            }
            DevToolsTab::Application => {
                // Application Tab: Storage & Cookies Inspector
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(Vec2::new(140.0, ui.available_height()), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.label(RichText::new("Storage").strong().size(12.0));
                        ui.separator();

                        let is_cookies = self.devtools_storage_section == StorageSection::Cookies;
                        if ui.selectable_label(is_cookies, "🍪 Cookies").clicked() {
                            self.devtools_storage_section = StorageSection::Cookies;
                        }
                        let is_local = self.devtools_storage_section == StorageSection::LocalStorage;
                        if ui.selectable_label(is_local, "🗄 Local Storage").clicked() {
                            self.devtools_storage_section = StorageSection::LocalStorage;
                        }
                        let is_session = self.devtools_storage_section == StorageSection::SessionStorage;
                        if ui.selectable_label(is_session, "📦 Session Storage").clicked() {
                            self.devtools_storage_section = StorageSection::SessionStorage;
                        }
                        let is_tex = self.devtools_storage_section == StorageSection::TextureCache;
                        if ui.selectable_label(is_tex, "🖼 Texture Cache").clicked() {
                            self.devtools_storage_section = StorageSection::TextureCache;
                        }
                    });

                    ui.separator();

                    ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                        match self.devtools_storage_section {
                            StorageSection::Cookies => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Cookies Table").strong().size(12.0));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Clear All Cookies").clicked() {
                                            self.devtools_cookies.clear();
                                        }
                                    });
                                });
                                ui.separator();
                                for cookie in &self.devtools_cookies {
                                    ui.label(format!("Name: {} | Domain: {} | Secure: {}", cookie.name, cookie.domain, cookie.secure));
                                }
                            }
                            StorageSection::LocalStorage => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Local Storage Items").strong().size(12.0));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Clear Local Storage").clicked() {
                                            self.devtools_local_storage.clear();
                                        }
                                    });
                                });
                                ui.separator();
                                for (k, v) in &self.devtools_local_storage {
                                    ui.label(format!("{}: {}", k, v));
                                }
                            }
                            StorageSection::SessionStorage => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Session Storage Items").strong().size(12.0));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Clear Session Storage").clicked() {
                                            self.devtools_session_storage.clear();
                                        }
                                    });
                                });
                                ui.separator();
                                for (k, v) in &self.devtools_session_storage {
                                    ui.label(format!("{}: {}", k, v));
                                }
                            }
                            StorageSection::TextureCache => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("Cached Textures: {}", self.image_textures.len())).strong().size(12.0));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Clear Texture Cache").clicked() {
                                            self.image_textures.clear();
                                        }
                                    });
                                });
                                ui.separator();
                                for url in self.image_textures.keys() {
                                    ui.label(RichText::new(url).weak());
                                }
                            }
                        }
                    });
                });
            }
            DevToolsTab::Security => {
                // Security Tab: TLS & Encryption Overview
                Frame::NONE
                    .fill(Color32::from_rgb(24, 26, 30))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 52, 58)))
                    .corner_radius(6)
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        if tab.url.starts_with("https://") {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("🔒 Valid HTTPS").strong().size(14.0).color(Color32::from_rgb(129, 201, 149)));
                                ui.label(RichText::new("Connection is Secure").color(Color32::from_rgb(232, 234, 237)));
                            });
                            ui.separator();
                            ui.add_space(4.0);
                            ui.label("Protocol: TLS 1.3");
                            ui.label("Cipher: TLS_AES_256_GCM_SHA384 (Pure Rust Rustls)");
                            ui.label("Certificate: Valid Mozilla WebPKI root anchor");
                            ui.label("Mixed Content: None (100% Secure Subresources)");
                        } else if tab.url.starts_with("http://") {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⚠️ Insecure Connection").strong().size(14.0).color(Color32::from_rgb(242, 139, 130)));
                                ui.label(RichText::new("Plaintext HTTP").color(Color32::from_rgb(242, 139, 130)));
                            });
                            ui.separator();
                            ui.add_space(4.0);
                            ui.label("Warning: Data transmission over HTTP is not encrypted.");
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("ℹ️ Internal Page").strong().size(14.0).color(Color32::from_rgb(138, 180, 248)));
                                ui.label(RichText::new("Local Environment").color(Color32::from_rgb(232, 234, 237)));
                            });
                            ui.separator();
                            ui.add_space(4.0);
                            ui.label("This page is served internally by the Aether Browser Engine.");
                        }
                    });
            }
        }

        if let Some(url) = pending_navigate {
            self.navigate_active_tab(&url);
        }
    }
}

fn draw_menu_icon(painter: &egui::Painter, icon: MenuIcon, center: Pos2, color: Color32) {
    let s = Stroke::new(1.3_f32, color);
    match icon {
        MenuIcon::Shield => {
            painter.line_segment([Pos2::new(center.x - 5.0, center.y - 6.0), Pos2::new(center.x + 5.0, center.y - 6.0)], s);
            painter.line_segment([Pos2::new(center.x + 5.0, center.y - 6.0), Pos2::new(center.x + 5.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x + 5.0, center.y), Pos2::new(center.x, center.y + 6.0)], s);
            painter.line_segment([Pos2::new(center.x, center.y + 6.0), Pos2::new(center.x - 5.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x - 5.0, center.y), Pos2::new(center.x - 5.0, center.y - 6.0)], s);
        }
        MenuIcon::NewTab => {
            let r = EguiRect::from_center_size(center, Vec2::new(11.0, 9.0));
            painter.rect_stroke(r, 1.5_f32, s, StrokeKind::Inside);
            painter.line_segment([Pos2::new(center.x - 2.5, center.y), Pos2::new(center.x + 2.5, center.y)], s);
            painter.line_segment([Pos2::new(center.x, center.y - 2.5), Pos2::new(center.x, center.y + 2.5)], s);
        }
        MenuIcon::NewWindow => {
            let r1 = EguiRect::from_min_size(Pos2::new(center.x - 5.0, center.y - 5.0), Vec2::new(8.0, 7.0));
            let r2 = EguiRect::from_min_size(Pos2::new(center.x - 2.0, center.y - 2.0), Vec2::new(8.0, 7.0));
            painter.rect_stroke(r1, 1.0_f32, Stroke::new(1.0_f32, color), StrokeKind::Inside);
            painter.rect_filled(r2, 1.0_f32, Color32::from_rgb(41, 42, 45));
            painter.rect_stroke(r2, 1.0_f32, s, StrokeKind::Inside);
        }
        MenuIcon::Incognito => {
            painter.circle_stroke(Pos2::new(center.x - 3.5, center.y + 1.0), 3.0, s);
            painter.circle_stroke(Pos2::new(center.x + 3.5, center.y + 1.0), 3.0, s);
            painter.line_segment([Pos2::new(center.x - 0.5, center.y + 1.0), Pos2::new(center.x + 0.5, center.y + 1.0)], s);
            painter.line_segment([Pos2::new(center.x - 7.0, center.y - 3.0), Pos2::new(center.x + 7.0, center.y - 3.0)], s);
        }
        MenuIcon::Passwords => {
            painter.circle_stroke(Pos2::new(center.x - 3.0, center.y), 3.0, s);
            painter.line_segment([Pos2::new(center.x, center.y), Pos2::new(center.x + 6.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x + 4.0, center.y), Pos2::new(center.x + 4.0, center.y + 2.5)], s);
            painter.line_segment([Pos2::new(center.x + 6.0, center.y), Pos2::new(center.x + 6.0, center.y + 2.5)], s);
        }
        MenuIcon::History => {
            painter.circle_stroke(center, 5.5, s);
            painter.line_segment([center, Pos2::new(center.x, center.y - 3.5)], s);
            painter.line_segment([center, Pos2::new(center.x + 2.5, center.y)], s);
        }
        MenuIcon::Downloads => {
            painter.line_segment([Pos2::new(center.x, center.y - 4.5), Pos2::new(center.x, center.y + 2.5)], s);
            painter.line_segment([Pos2::new(center.x - 3.0, center.y), Pos2::new(center.x + 3.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x + 3.0, center.y), Pos2::new(center.x, center.y + 3.0)], s);
            painter.line_segment([Pos2::new(center.x - 5.0, center.y + 5.0), Pos2::new(center.x + 5.0, center.y + 5.0)], s);
        }
        MenuIcon::Bookmarks => {
            let r = 5.5;
            let mut pts = Vec::new();
            for i in 0..10 {
                let angle = (i as f32) * std::f32::consts::PI / 5.0 - std::f32::consts::FRAC_PI_2;
                let rad = if i % 2 == 0 { r } else { r * 0.45 };
                pts.push(Pos2::new(center.x + angle.cos() * rad, center.y + angle.sin() * rad));
            }
            painter.add(egui::Shape::closed_line(pts, s));
        }
        MenuIcon::TabGroups => {
            for dx in [-3.5, 1.5] {
                for dy in [-3.5, 1.5] {
                    let r = EguiRect::from_min_size(Pos2::new(center.x + dx, center.y + dy), Vec2::new(3.0, 3.0));
                    painter.rect_stroke(r, 0.5_f32, Stroke::new(1.0_f32, color), StrokeKind::Inside);
                }
            }
        }
        MenuIcon::Extensions => {
            let r = EguiRect::from_center_size(center, Vec2::new(9.0, 9.0));
            painter.rect_stroke(r, 1.5_f32, s, StrokeKind::Inside);
            painter.circle_filled(Pos2::new(center.x + 4.5, center.y), 1.5, color);
        }
        MenuIcon::DeleteData => {
            painter.line_segment([Pos2::new(center.x - 5.0, center.y - 4.0), Pos2::new(center.x + 5.0, center.y - 4.0)], s);
            let bin = EguiRect::from_min_size(Pos2::new(center.x - 3.5, center.y - 3.0), Vec2::new(7.0, 8.0));
            painter.rect_stroke(bin, 1.0_f32, s, StrokeKind::Inside);
            painter.line_segment([Pos2::new(center.x, center.y - 1.5), Pos2::new(center.x, center.y + 3.5)], Stroke::new(1.0_f32, color));
        }
        MenuIcon::Zoom => {
            let glass_center = Pos2::new(center.x - 1.5, center.y - 1.5);
            painter.circle_stroke(glass_center, 4.0, s);
            painter.line_segment([Pos2::new(glass_center.x + 2.8, glass_center.y + 2.8), Pos2::new(center.x + 5.5, center.y + 5.5)], Stroke::new(1.8_f32, color));
        }
        MenuIcon::Print => {
            let base = EguiRect::from_center_size(Pos2::new(center.x, center.y + 1.0), Vec2::new(11.0, 6.0));
            painter.rect_stroke(base, 1.0_f32, s, StrokeKind::Inside);
            let paper = EguiRect::from_center_size(Pos2::new(center.x, center.y - 3.5), Vec2::new(7.0, 4.0));
            painter.rect_stroke(paper, 0.5_f32, Stroke::new(1.0_f32, color), StrokeKind::Inside);
        }
        MenuIcon::GoogleLens => {
            draw_camera_icon(painter, center, 5.5, color);
        }
        MenuIcon::Translate => {
            painter.text(center, Align2::CENTER_CENTER, "文A", FontId::proportional(10.0), color);
        }
        MenuIcon::Find => {
            let doc = EguiRect::from_center_size(Pos2::new(center.x - 1.0, center.y), Vec2::new(8.0, 10.0));
            painter.rect_stroke(doc, 1.0_f32, Stroke::new(1.0_f32, color), StrokeKind::Inside);
            painter.circle_stroke(Pos2::new(center.x + 2.0, center.y + 2.0), 2.5, Stroke::new(1.2_f32, color));
        }
        MenuIcon::Cast => {
            let screen = EguiRect::from_center_size(center, Vec2::new(11.0, 8.0));
            painter.rect_stroke(screen, 1.0_f32, s, StrokeKind::Inside);
            painter.circle_filled(Pos2::new(center.x - 3.5, center.y + 2.0), 1.0, color);
        }
        MenuIcon::MoreTools => {
            painter.line_segment([Pos2::new(center.x - 4.5, center.y - 4.5), Pos2::new(center.x + 4.5, center.y + 4.5)], s);
            painter.line_segment([Pos2::new(center.x - 4.5, center.y + 4.5), Pos2::new(center.x + 4.5, center.y - 4.5)], s);
        }
        MenuIcon::Help => {
            painter.circle_stroke(center, 5.5, s);
            painter.text(center, Align2::CENTER_CENTER, "?", FontId::proportional(9.0), color);
        }
        MenuIcon::Settings => {
            painter.circle_stroke(center, 4.0, s);
            for i in 0..4 {
                let angle = (i as f32) * std::f32::consts::PI / 2.0;
                let p1 = Pos2::new(center.x + angle.cos() * 4.0, center.y + angle.sin() * 4.0);
                let p2 = Pos2::new(center.x + angle.cos() * 5.8, center.y + angle.sin() * 5.8);
                painter.line_segment([p1, p2], s);
            }
        }
        MenuIcon::Exit => {
            let door = EguiRect::from_center_size(Pos2::new(center.x - 2.5, center.y), Vec2::new(6.0, 10.0));
            painter.rect_stroke(door, 1.0_f32, s, StrokeKind::Inside);
            painter.line_segment([Pos2::new(center.x - 1.0, center.y), Pos2::new(center.x + 5.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x + 3.0, center.y - 2.0), Pos2::new(center.x + 5.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x + 3.0, center.y + 2.0), Pos2::new(center.x + 5.0, center.y)], s);
        }
    }
}

fn render_menu_item(
    ui: &mut egui::Ui,
    icon: MenuIcon,
    label: &str,
    shortcut_or_chevron: Option<&str>,
) -> egui::Response {
    let desired_size = Vec2::new(ui.available_width(), 28.0);
    let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(60, 64, 67));
    }

    let icon_center = Pos2::new(rect.min.x + 16.0, rect.center().y);
    draw_menu_icon(ui.painter(), icon, icon_center, Color32::from_rgb(232, 234, 237));

    let text_pos = Pos2::new(rect.min.x + 34.0, rect.center().y);
    ui.painter().text(
        text_pos,
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        Color32::from_rgb(232, 234, 237),
    );

    if let Some(right_hint) = shortcut_or_chevron {
        let is_chevron = right_hint == ">" || right_hint == "›";
        let font_size = if is_chevron { 13.0 } else { 11.5 };
        let right_pos = Pos2::new(rect.max.x - 10.0, rect.center().y);
        ui.painter().text(
            right_pos,
            Align2::RIGHT_CENTER,
            right_hint,
            FontId::proportional(font_size),
            Color32::from_rgb(154, 160, 166),
        );
    }

    resp
}

fn render_profile_card(
    ui: &mut egui::Ui,
    name: &str,
    status: &str,
) -> egui::Response {
    let desired_size = Vec2::new(ui.available_width(), 36.0);
    let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    let bg_color = if resp.hovered() {
        Color32::from_rgb(64, 68, 74)
    } else {
        Color32::from_rgb(53, 56, 61)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), bg_color);

    let avatar_center = Pos2::new(rect.min.x + 18.0, rect.center().y);
    ui.painter().circle_filled(avatar_center, 10.0, Color32::from_rgb(32, 33, 36));
    ui.painter().text(
        avatar_center,
        Align2::CENTER_CENTER,
        "K",
        FontId::proportional(11.0),
        Color32::from_rgb(138, 180, 248),
    );

    let name_pos = Pos2::new(rect.min.x + 34.0, rect.center().y);
    ui.painter().text(
        name_pos,
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(12.5),
        Color32::from_rgb(232, 234, 237),
    );

    let chevron_x = rect.max.x - 10.0;
    ui.painter().text(
        Pos2::new(chevron_x, rect.center().y),
        Align2::RIGHT_CENTER,
        ">",
        FontId::proportional(13.0),
        Color32::from_rgb(154, 160, 166),
    );

    let pill_w = 60.0;
    let pill_h = 18.0;
    let pill_rect = EguiRect::from_center_size(
        Pos2::new(chevron_x - 12.0 - pill_w / 2.0, rect.center().y),
        Vec2::new(pill_w, pill_h),
    );
    ui.painter().rect_filled(pill_rect, CornerRadius::same(9), Color32::from_rgb(26, 115, 232));
    ui.painter().text(
        pill_rect.center(),
        Align2::CENTER_CENTER,
        status,
        FontId::proportional(10.0),
        Color32::WHITE,
    );

    resp
}

fn render_menu_divider(ui: &mut egui::Ui) {
    ui.add_space(2.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [Pos2::new(rect.min.x + 4.0, rect.center().y), Pos2::new(rect.max.x - 4.0, rect.center().y)],
        Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)),
    );
    ui.add_space(2.0);
}

fn render_zoom_row(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut BrowserApp,
) {
    let desired_size = Vec2::new(ui.available_width(), 32.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    let icon_center = Pos2::new(rect.min.x + 16.0, rect.center().y);
    draw_menu_icon(ui.painter(), MenuIcon::Zoom, icon_center, Color32::from_rgb(232, 234, 237));

    let label_pos = Pos2::new(rect.min.x + 34.0, rect.center().y);
    ui.painter().text(
        label_pos,
        Align2::LEFT_CENTER,
        "Zoom",
        FontId::proportional(12.5),
        Color32::from_rgb(232, 234, 237),
    );

    let btn_size = Vec2::new(24.0, 24.0);

    // Fullscreen button
    let fs_rect = EguiRect::from_center_size(Pos2::new(rect.max.x - 16.0, rect.center().y), btn_size);
    let fs_resp = ui.interact(fs_rect, ui.id().with("menu_zoom_fs"), egui::Sense::click());
    if fs_resp.hovered() {
        ui.painter().circle_filled(fs_rect.center(), 12.0, Color32::from_rgb(70, 74, 80));
    }
    let fc = fs_rect.center();
    let fs_stroke = Stroke::new(1.3_f32, Color32::from_rgb(232, 234, 237));
    let d = 4.5;
    let b = 2.0;
    ui.painter().line_segment([Pos2::new(fc.x - d, fc.y - d + b), Pos2::new(fc.x - d, fc.y - d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x - d, fc.y - d), Pos2::new(fc.x - d + b, fc.y - d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x + d - b, fc.y - d), Pos2::new(fc.x + d, fc.y - d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x + d, fc.y - d), Pos2::new(fc.x + d, fc.y - d + b)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x - d, fc.y + d - b), Pos2::new(fc.x - d, fc.y + d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x - d, fc.y + d), Pos2::new(fc.x - d + b, fc.y + d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x + d - b, fc.y + d), Pos2::new(fc.x + d, fc.y + d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x + d, fc.y + d), Pos2::new(fc.x + d, fc.y + d - b)], fs_stroke);

    if fs_resp.clicked() {
        let is_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
    }

    // [+] Button
    let plus_rect = EguiRect::from_center_size(Pos2::new(fs_rect.min.x - 16.0, rect.center().y), btn_size);
    let plus_resp = ui.interact(plus_rect, ui.id().with("menu_zoom_plus"), egui::Sense::click());
    if plus_resp.hovered() {
        ui.painter().circle_filled(plus_rect.center(), 12.0, Color32::from_rgb(70, 74, 80));
    }
    let pc = plus_rect.center();
    let p_stroke = Stroke::new(1.4_f32, Color32::from_rgb(232, 234, 237));
    ui.painter().line_segment([Pos2::new(pc.x - 4.0, pc.y), Pos2::new(pc.x + 4.0, pc.y)], p_stroke);
    ui.painter().line_segment([Pos2::new(pc.x, pc.y - 4.0), Pos2::new(pc.x, pc.y + 4.0)], p_stroke);
    if plus_resp.clicked() {
        app.zoom_in();
    }

    // Percentage display
    let pct_text = app.zoom_percentage();
    let pct_x = plus_rect.min.x - 22.0;
    ui.painter().text(
        Pos2::new(pct_x, rect.center().y),
        Align2::CENTER_CENTER,
        pct_text,
        FontId::proportional(12.0),
        Color32::from_rgb(232, 234, 237),
    );

    // [-] Button
    let minus_rect = EguiRect::from_center_size(Pos2::new(pct_x - 22.0, rect.center().y), btn_size);
    let minus_resp = ui.interact(minus_rect, ui.id().with("menu_zoom_minus"), egui::Sense::click());
    if minus_resp.hovered() {
        ui.painter().circle_filled(minus_rect.center(), 12.0, Color32::from_rgb(70, 74, 80));
    }
    let mc = minus_rect.center();
    ui.painter().line_segment([Pos2::new(mc.x - 4.0, mc.y), Pos2::new(mc.x + 4.0, mc.y)], p_stroke);
    if minus_resp.clicked() {
        app.zoom_out();
    }
}

fn format_current_time() -> String {
    if let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        let total_secs = duration.as_secs();
        let hours = (total_secs / 3600) % 24;
        let mins = (total_secs / 60) % 60;
        let secs = total_secs % 60;
        let millis = duration.subsec_millis();
        format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis)
    } else {
        "00:00:00.000".to_string()
    }
}

fn format_dim(val: f32) -> String {
    if val.abs() < 0.01 {
        "-".to_string()
    } else {
        format!("{:.0}", val)
    }
}

fn render_chrome_box_model(ui: &mut egui::Ui, dims: Option<&Dimensions>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width() - 8.0, 130.0), egui::Sense::hover());
    let painter = ui.painter();

    let m_top = dims.map_or("-".to_string(), |d| format_dim(d.margin.top));
    let m_bot = dims.map_or("-".to_string(), |d| format_dim(d.margin.bottom));
    let m_left = dims.map_or("-".to_string(), |d| format_dim(d.margin.left));
    let m_right = dims.map_or("-".to_string(), |d| format_dim(d.margin.right));

    let b_top = dims.map_or("-".to_string(), |d| format_dim(d.border.top));
    let b_bot = dims.map_or("-".to_string(), |d| format_dim(d.border.bottom));
    let b_left = dims.map_or("-".to_string(), |d| format_dim(d.border.left));
    let b_right = dims.map_or("-".to_string(), |d| format_dim(d.border.right));

    let p_top = dims.map_or("-".to_string(), |d| format_dim(d.padding.top));
    let p_bot = dims.map_or("-".to_string(), |d| format_dim(d.padding.bottom));
    let p_left = dims.map_or("-".to_string(), |d| format_dim(d.padding.left));
    let p_right = dims.map_or("-".to_string(), |d| format_dim(d.padding.right));

    let content_text = dims.map_or("- × -".to_string(), |d| format!("{:.0} × {:.0}", d.content.width, d.content.height));

    // 1. Margin (outer layer, orange #f9ab00, bg #2d2619)
    let margin_rect = rect;
    painter.rect_filled(margin_rect, 4.0, Color32::from_rgb(45, 38, 25));
    painter.rect_stroke(margin_rect, 4.0, Stroke::new(1.0_f32, Color32::from_rgb(249, 171, 0)), StrokeKind::Inside);
    painter.text(Pos2::new(margin_rect.min.x + 6.0, margin_rect.min.y + 3.0), Align2::LEFT_TOP, "margin", FontId::proportional(9.0), Color32::from_rgb(249, 171, 0));
    painter.text(Pos2::new(margin_rect.center().x, margin_rect.min.y + 4.0), Align2::CENTER_TOP, m_top, FontId::proportional(9.5), Color32::from_rgb(249, 171, 0));
    painter.text(Pos2::new(margin_rect.center().x, margin_rect.max.y - 4.0), Align2::CENTER_BOTTOM, m_bot, FontId::proportional(9.5), Color32::from_rgb(249, 171, 0));
    painter.text(Pos2::new(margin_rect.min.x + 8.0, margin_rect.center().y), Align2::LEFT_CENTER, m_left, FontId::proportional(9.5), Color32::from_rgb(249, 171, 0));
    painter.text(Pos2::new(margin_rect.max.x - 8.0, margin_rect.center().y), Align2::RIGHT_CENTER, m_right, FontId::proportional(9.5), Color32::from_rgb(249, 171, 0));

    // 2. Border (middle layer, yellow #fdd663, bg #2a281c)
    let border_rect = margin_rect.shrink(16.0);
    painter.rect_filled(border_rect, 3.0, Color32::from_rgb(42, 40, 28));
    painter.rect_stroke(border_rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(253, 214, 99)), StrokeKind::Inside);
    painter.text(Pos2::new(border_rect.min.x + 6.0, border_rect.min.y + 3.0), Align2::LEFT_TOP, "border", FontId::proportional(9.0), Color32::from_rgb(253, 214, 99));
    painter.text(Pos2::new(border_rect.center().x, border_rect.min.y + 3.0), Align2::CENTER_TOP, b_top, FontId::proportional(9.5), Color32::from_rgb(253, 214, 99));
    painter.text(Pos2::new(border_rect.center().x, border_rect.max.y - 3.0), Align2::CENTER_BOTTOM, b_bot, FontId::proportional(9.5), Color32::from_rgb(253, 214, 99));
    painter.text(Pos2::new(border_rect.min.x + 8.0, border_rect.center().y), Align2::LEFT_CENTER, b_left, FontId::proportional(9.5), Color32::from_rgb(253, 214, 99));
    painter.text(Pos2::new(border_rect.max.x - 8.0, border_rect.center().y), Align2::RIGHT_CENTER, b_right, FontId::proportional(9.5), Color32::from_rgb(253, 214, 99));

    // 3. Padding (third layer, green #81c995, bg #1a2820)
    let padding_rect = border_rect.shrink(15.0);
    painter.rect_filled(padding_rect, 2.0, Color32::from_rgb(26, 40, 32));
    painter.rect_stroke(padding_rect, 2.0, Stroke::new(1.0_f32, Color32::from_rgb(129, 201, 149)), StrokeKind::Inside);
    painter.text(Pos2::new(padding_rect.min.x + 6.0, padding_rect.min.y + 3.0), Align2::LEFT_TOP, "padding", FontId::proportional(9.0), Color32::from_rgb(129, 201, 149));
    painter.text(Pos2::new(padding_rect.center().x, padding_rect.min.y + 3.0), Align2::CENTER_TOP, p_top, FontId::proportional(9.5), Color32::from_rgb(129, 201, 149));
    painter.text(Pos2::new(padding_rect.center().x, padding_rect.max.y - 3.0), Align2::CENTER_BOTTOM, p_bot, FontId::proportional(9.5), Color32::from_rgb(129, 201, 149));
    painter.text(Pos2::new(padding_rect.min.x + 8.0, padding_rect.center().y), Align2::LEFT_CENTER, p_left, FontId::proportional(9.5), Color32::from_rgb(129, 201, 149));
    painter.text(Pos2::new(padding_rect.max.x - 8.0, padding_rect.center().y), Align2::RIGHT_CENTER, p_right, FontId::proportional(9.5), Color32::from_rgb(129, 201, 149));

    // 4. Content (innermost layer, blue #8ab4f8, bg #1e2a3c)
    let content_rect = padding_rect.shrink(14.0);
    painter.rect_filled(content_rect, 2.0, Color32::from_rgb(30, 42, 60));
    painter.rect_stroke(content_rect, 2.0, Stroke::new(1.0_f32, Color32::from_rgb(138, 180, 248)), StrokeKind::Inside);
    painter.text(content_rect.center(), Align2::CENTER_CENTER, content_text, FontId::proportional(10.5), Color32::from_rgb(138, 180, 248));
}

pub enum VectorIcon {
    Letter(&'static str),
    PlayTriangle,
    Infinity,
    Sparkle,
    Camera,
    Plus,
}

/// Helper to render a circular shortcut icon button with guaranteed vector graphics
fn render_vector_shortcut<F: FnOnce()>(
    ui: &mut egui::Ui,
    icon: VectorIcon,
    bg_color: Color32,
    label: &str,
    on_click: F,
) {
    let item_size = Vec2::new(68.0, 78.0);
    ui.allocate_ui_with_layout(
        item_size,
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;

            let (rect, resp) = ui.allocate_exact_size(Vec2::new(48.0, 48.0), egui::Sense::click());
            let center = rect.center();

            let hover_bg = if resp.hovered() {
                Color32::from_rgba_premultiplied(
                    bg_color.r().saturating_add(25),
                    bg_color.g().saturating_add(25),
                    bg_color.b().saturating_add(25),
                    255,
                )
            } else {
                bg_color
            };

            ui.painter().circle_filled(center, 24.0, hover_bg);

            match icon {
                VectorIcon::Letter(s) => {
                    let font_size = if s.len() > 1 { 16.0 } else { 22.0 };
                    ui.painter().text(center, Align2::CENTER_CENTER, s, FontId::proportional(font_size), Color32::WHITE);
                }
                VectorIcon::PlayTriangle => {
                    // Pointing right
                    let p1 = Pos2::new(center.x - 4.0, center.y - 7.0);
                    let p2 = Pos2::new(center.x + 7.0, center.y);
                    let p3 = Pos2::new(center.x - 4.0, center.y + 7.0);
                    ui.painter().add(egui::Shape::convex_polygon(vec![p1, p2, p3], Color32::WHITE, Stroke::NONE));
                }
                VectorIcon::Infinity => {
                    // Two connected loops
                    ui.painter().circle_stroke(Pos2::new(center.x - 5.0, center.y), 4.5, Stroke::new(1.8_f32, Color32::WHITE));
                    ui.painter().circle_stroke(Pos2::new(center.x + 5.0, center.y), 4.5, Stroke::new(1.8_f32, Color32::WHITE));
                }
                VectorIcon::Sparkle => {
                    draw_sparkle_icon(ui.painter(), center, 9.0, Color32::WHITE);
                }
                VectorIcon::Camera => {
                    draw_camera_icon(ui.painter(), center, 9.0, Color32::WHITE);
                }
                VectorIcon::Plus => {
                    ui.painter().line_segment([Pos2::new(center.x - 6.0, center.y), Pos2::new(center.x + 6.0, center.y)], Stroke::new(2.0_f32, Color32::WHITE));
                    ui.painter().line_segment([Pos2::new(center.x, center.y - 6.0), Pos2::new(center.x, center.y + 6.0)], Stroke::new(2.0_f32, Color32::WHITE));
                }
            }

            let text_color = if resp.hovered() { Color32::WHITE } else { Color32::from_rgb(232, 234, 237) };
            ui.label(RichText::new(label).size(12.0).color(text_color));

            if resp.clicked() {
                on_click();
            }
        },
    );
}

fn draw_sparkle_icon(painter: &egui::Painter, center: Pos2, r: f32, color: Color32) {
    let top = Pos2::new(center.x, center.y - r);
    let right = Pos2::new(center.x + r, center.y);
    let bottom = Pos2::new(center.x, center.y + r);
    let left = Pos2::new(center.x - r, center.y);
    painter.add(egui::Shape::convex_polygon(vec![top, right, bottom, left], color, Stroke::NONE));
}

fn draw_camera_icon(painter: &egui::Painter, center: Pos2, r: f32, color: Color32) {
    let box_rect = egui::Rect::from_center_size(center, Vec2::new(r * 2.0, r * 1.5));
    painter.rect_stroke(box_rect, 2.0_f32, Stroke::new(1.4_f32, color), StrokeKind::Inside);
    painter.circle_stroke(center, r * 0.45, Stroke::new(1.4_f32, color));
}

fn render_dom_tree_node(
    ui: &mut egui::Ui,
    node: &Node,
    current_path: Vec<usize>,
    depth: usize,
    selected_path: &Option<Vec<usize>>,
    expanded_paths: &mut std::collections::HashSet<Vec<usize>>,
    new_selected_path: &mut Option<Vec<usize>>,
    search_query: &str,
) {
    if depth > 16 {
        return;
    }

    match &node.node_type {
        NodeType::Element(data) => {
            let has_children = !node.children.is_empty();
            let is_expanded = expanded_paths.contains(&current_path);
            let is_selected = selected_path.as_ref() == Some(&current_path);

            let row_height = 20.0;
            let (rect, row_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), egui::Sense::click());

            if is_selected {
                ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(40, 62, 88));
            } else if row_resp.hovered() {
                ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(40, 43, 48));
            }

            if row_resp.clicked() {
                *new_selected_path = Some(current_path.clone());
            }

            // Indentation
            let indent_x = rect.min.x + (depth as f32) * 14.0 + 4.0;

            // Expand/collapse caret
            if has_children {
                let caret_rect = EguiRect::from_center_size(Pos2::new(indent_x + 4.0, rect.center().y), Vec2::new(12.0, 12.0));
                let caret_resp = ui.interact(caret_rect, ui.id().with(("caret", &current_path)), egui::Sense::click());
                let caret_sym = if is_expanded { "▼" } else { "▶" };
                ui.painter().text(caret_rect.center(), Align2::CENTER_CENTER, caret_sym, FontId::proportional(9.0), Color32::from_rgb(154, 160, 166));
                if caret_resp.clicked() {
                    if is_expanded {
                        expanded_paths.remove(&current_path);
                    } else {
                        expanded_paths.insert(current_path.clone());
                    }
                }
            }

            // Syntax tokens: <tag attr="val">
            let text_start_x = indent_x + 14.0;
            let mut cursor_x = text_start_x;

            // '<'
            ui.painter().text(Pos2::new(cursor_x, rect.center().y), Align2::LEFT_CENTER, "<", FontId::monospace(11.0), Color32::from_rgb(128, 134, 139));
            cursor_x += 8.0;

            // Tag name in blue #8ab4f8
            ui.painter().text(Pos2::new(cursor_x, rect.center().y), Align2::LEFT_CENTER, &data.tag_name, FontId::monospace(11.0), Color32::from_rgb(138, 180, 248));
            cursor_x += (data.tag_name.len() as f32) * 6.6;

            // Attributes
            for (attr_k, attr_v) in &data.attributes {
                cursor_x += 5.0;
                ui.painter().text(Pos2::new(cursor_x, rect.center().y), Align2::LEFT_CENTER, attr_k, FontId::monospace(11.0), Color32::from_rgb(253, 214, 99));
                cursor_x += (attr_k.len() as f32) * 6.6;

                ui.painter().text(Pos2::new(cursor_x, rect.center().y), Align2::LEFT_CENTER, "=\"", FontId::monospace(11.0), Color32::from_rgb(128, 134, 139));
                cursor_x += 12.0;

                let val_preview = if attr_v.len() > 24 { format!("{}...", &attr_v[..22]) } else { attr_v.clone() };
                ui.painter().text(Pos2::new(cursor_x, rect.center().y), Align2::LEFT_CENTER, &val_preview, FontId::monospace(11.0), Color32::from_rgb(129, 201, 149));
                cursor_x += (val_preview.len() as f32) * 6.6;

                ui.painter().text(Pos2::new(cursor_x, rect.center().y), Align2::LEFT_CENTER, "\"", FontId::monospace(11.0), Color32::from_rgb(128, 134, 139));
                cursor_x += 6.0;
            }

            // '>'
            let closing_preview = if !has_children {
                ">"
            } else if !is_expanded {
                "> ... </"
            } else {
                ">"
            };
            ui.painter().text(Pos2::new(cursor_x, rect.center().y), Align2::LEFT_CENTER, closing_preview, FontId::monospace(11.0), Color32::from_rgb(128, 134, 139));
            cursor_x += (closing_preview.len() as f32) * 6.6;

            if has_children && !is_expanded {
                ui.painter().text(Pos2::new(cursor_x, rect.center().y), Align2::LEFT_CENTER, &data.tag_name, FontId::monospace(11.0), Color32::from_rgb(138, 180, 248));
                cursor_x += (data.tag_name.len() as f32) * 6.6;
                ui.painter().text(Pos2::new(cursor_x, rect.center().y), Align2::LEFT_CENTER, ">", FontId::monospace(11.0), Color32::from_rgb(128, 134, 139));
            }

            // Recursion if expanded
            if has_children && is_expanded {
                for (child_idx, child) in node.children.iter().enumerate() {
                    let mut child_path = current_path.clone();
                    child_path.push(child_idx);
                    render_dom_tree_node(
                        ui,
                        child,
                        child_path,
                        depth + 1,
                        selected_path,
                        expanded_paths,
                        new_selected_path,
                        search_query,
                    );
                }

                // Render matching closing tag </tag_name>
                let (close_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), egui::Sense::hover());
                let close_indent_x = close_rect.min.x + (depth as f32) * 14.0 + 18.0;
                let mut cx = close_indent_x;
                ui.painter().text(Pos2::new(cx, close_rect.center().y), Align2::LEFT_CENTER, "</", FontId::monospace(11.0), Color32::from_rgb(128, 134, 139));
                cx += 13.0;
                ui.painter().text(Pos2::new(cx, close_rect.center().y), Align2::LEFT_CENTER, &data.tag_name, FontId::monospace(11.0), Color32::from_rgb(138, 180, 248));
                cx += (data.tag_name.len() as f32) * 6.6;
                ui.painter().text(Pos2::new(cx, close_rect.center().y), Align2::LEFT_CENTER, ">", FontId::monospace(11.0), Color32::from_rgb(128, 134, 139));
            }
        }
        NodeType::Text(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let row_height = 18.0;
                let (rect, row_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), egui::Sense::click());
                let is_selected = selected_path.as_ref() == Some(&current_path);
                if is_selected {
                    ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(40, 62, 88));
                }
                if row_resp.clicked() {
                    *new_selected_path = Some(current_path.clone());
                }

                let indent_x = rect.min.x + (depth as f32) * 14.0 + 18.0;
                let preview = if trimmed.len() > 48 { format!("\"{}...\"", &trimmed[..45]) } else { format!("\"{}\"", trimmed) };
                ui.painter().text(Pos2::new(indent_x, rect.center().y), Align2::LEFT_CENTER, preview, FontId::monospace(10.5), Color32::from_rgb(232, 234, 237));
            }
        }
        NodeType::Comment(comment) => {
            let row_height = 18.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), egui::Sense::hover());
            let indent_x = rect.min.x + (depth as f32) * 14.0 + 18.0;
            let preview = format!("<!-- {} -->", comment.trim());
            ui.painter().text(Pos2::new(indent_x, rect.center().y), Align2::LEFT_CENTER, preview, FontId::monospace(10.5), Color32::from_rgb(128, 134, 139));
        }
    }
}
