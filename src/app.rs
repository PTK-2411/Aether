use crate::css::{CssParser, Stylesheet};
use crate::default_style::DEFAULT_USER_AGENT_CSS;
use crate::dom::{Node, NodeType};
use crate::html_parser::HtmlParser;
use crate::layout::{build_layout_tree, Dimensions, EdgeSizes, Rect};
use crate::network::{normalize_url, resolve_relative_url, NetworkManager, NetworkMessage};
use crate::render::{build_display_list, paint_display_list, DisplayList};
use crate::style::style_tree;
use eframe::egui::{self, Color32, CursorIcon, Key, RichText, ScrollArea, Vec2};

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
}

impl Tab {
    pub fn new(id: usize, url: String) -> Self {
        Tab {
            id,
            title: "New Tab".to_string(),
            url: url.clone(),
            history: vec![url],
            history_index: 0,
            is_loading: false,
            scroll_y: 0.0,
            raw_html: String::new(),
            dom_root: None,
            stylesheet: Stylesheet { rules: Vec::new() },
            display_list: None,
            page_height: 0.0,
            last_layout_width: 0.0,
        }
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

pub struct BrowserApp {
    tabs: Vec<Tab>,
    active_tab_idx: usize,
    next_tab_id: usize,
    network: NetworkManager,
    address_input: String,
    hovered_link: Option<String>,
    show_devtools: bool,
    devtools_tab: DevToolsTab,
    selected_dom_node: Option<String>,
    zoom_level: f32,
}

#[derive(PartialEq, Eq)]
enum DevToolsTab {
    DomTree,
    Styles,
    Source,
    Network,
}

impl BrowserApp {
    pub fn new() -> Self {
        let network = NetworkManager::new();
        let mut app = BrowserApp {
            tabs: Vec::new(),
            active_tab_idx: 0,
            next_tab_id: 1,
            network,
            address_input: "about:welcome".to_string(),
            hovered_link: None,
            show_devtools: false,
            devtools_tab: DevToolsTab::DomTree,
            selected_dom_node: None,
            zoom_level: 1.0,
        };

        // Open initial tab
        app.open_new_tab("about:welcome".to_string());
        app
    }

    pub fn open_new_tab(&mut self, url: String) {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let mut tab = Tab::new(tab_id, url.clone());
        tab.is_loading = true;
        self.tabs.push(tab);
        self.active_tab_idx = self.tabs.len() - 1;
        self.address_input = url.clone();
        self.network.fetch(tab_id, &url);
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            return; // keep at least 1 tab
        }
        self.tabs.remove(idx);
        if self.active_tab_idx >= self.tabs.len() {
            self.active_tab_idx = self.tabs.len() - 1;
        }
        if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            self.address_input = tab.url.clone();
        }
    }

    pub fn navigate_active_tab(&mut self, target_url: &str) {
        let normalized = normalize_url(target_url);
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            tab.is_loading = true;
            tab.url = normalized.clone();
            tab.scroll_y = 0.0;

            // Update history
            if tab.history_index + 1 < tab.history.len() {
                tab.history.truncate(tab.history_index + 1);
            }
            tab.history.push(normalized.clone());
            tab.history_index = tab.history.len() - 1;

            self.address_input = normalized.clone();
            let tab_id = tab.id;
            self.network.fetch(tab_id, &normalized);
        }
    }

    pub fn go_back(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if tab.can_go_back() {
                tab.history_index -= 1;
                let url = tab.history[tab.history_index].clone();
                tab.url = url.clone();
                tab.is_loading = true;
                tab.scroll_y = 0.0;
                self.address_input = url.clone();
                let tab_id = tab.id;
                self.network.fetch(tab_id, &url);
            }
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if tab.can_go_forward() {
                tab.history_index += 1;
                let url = tab.history[tab.history_index].clone();
                tab.url = url.clone();
                tab.is_loading = true;
                tab.scroll_y = 0.0;
                self.address_input = url.clone();
                let tab_id = tab.id;
                self.network.fetch(tab_id, &url);
            }
        }
    }

    pub fn reload(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            tab.is_loading = true;
            let url = tab.url.clone();
            let tab_id = tab.id;
            self.network.fetch(tab_id, &url);
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
                                tab.url = res.url.clone();
                                tab.title = if res.title.len() > 24 {
                                    format!("{}...", &res.title[..24])
                                } else {
                                    res.title
                                };
                                tab.raw_html = res.html;

                                // Parse HTML to DOM
                                let dom = HtmlParser::parse(&tab.raw_html);

                                // Collect internal styles
                                let mut css_source = DEFAULT_USER_AGENT_CSS.to_string();
                                collect_style_tags(&dom, &mut css_source);

                                tab.stylesheet = CssParser::parse(&css_source);
                                tab.dom_root = Some(dom);

                                // Trigger layout
                                let layout_w = if tab.last_layout_width > 0.0 {
                                    tab.last_layout_width
                                } else {
                                    850.0
                                };
                                tab.recompute_layout(layout_w);
                            }
                            Err(err) => {
                                tab.title = "Error loading page".to_string();
                                tab.raw_html = format!(
                                    "<html><body style='padding: 24px; font-family: sans-serif;'><h1 style='color: red;'>Load Error</h1><p>{}</p></body></html>",
                                    err
                                );
                                let dom = HtmlParser::parse(&tab.raw_html);
                                tab.stylesheet = CssParser::parse(DEFAULT_USER_AGENT_CSS);
                                tab.dom_root = Some(dom);
                                tab.recompute_layout(850.0);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn collect_style_tags(node: &Node, out: &mut String) {
    if node.tag_name() == Some("style") {
        out.push('\n');
        out.push_str(&node.inner_text());
    }
    for child in &node.children {
        collect_style_tags(child, out);
    }
}

impl eframe::App for BrowserApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_network_responses();

        // Top Control Panel (Tabs, Navigation Bar)
        egui::TopBottomPanel::top("browser_top_bar").show(ctx, |ui| {
            // 1. Tab Bar
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let mut close_target = None;

                for (idx, tab) in self.tabs.iter().enumerate() {
                    let is_active = idx == self.active_tab_idx;
                    let title = if tab.is_loading {
                        format!("⏳ {}", tab.title)
                    } else {
                        format!("🌐 {}", tab.title)
                    };

                    let btn = egui::Button::new(
                        RichText::new(title)
                            .strong()
                            .color(if is_active { Color32::WHITE } else { Color32::from_rgb(180, 185, 195) }),
                    )
                    .fill(if is_active {
                        Color32::from_rgb(45, 52, 70)
                    } else {
                        Color32::from_rgb(28, 32, 42)
                    });

                    if ui.add(btn).clicked() {
                        self.active_tab_idx = idx;
                        self.address_input = self.tabs[idx].url.clone();
                    }

                    if self.tabs.len() > 1 {
                        if ui.small_button("×").clicked() {
                            close_target = Some(idx);
                        }
                    }
                    ui.separator();
                }

                if let Some(target) = close_target {
                    self.close_tab(target);
                }

                if ui.button("+").clicked() {
                    self.open_new_tab("about:welcome".to_string());
                }
            });

            ui.add_space(2.0);

            // 2. Navigation toolbar
            ui.horizontal(|ui| {
                let can_back = self.tabs.get(self.active_tab_idx).map_or(false, |t| t.can_go_back());
                let can_fwd = self.tabs.get(self.active_tab_idx).map_or(false, |t| t.can_go_forward());

                if ui.add_enabled(can_back, egui::Button::new("⬅")).clicked() {
                    self.go_back();
                }
                if ui.add_enabled(can_fwd, egui::Button::new("➡")).clicked() {
                    self.go_forward();
                }
                if ui.button("🔄").clicked() {
                    self.reload();
                }
                if ui.button("🏠").clicked() {
                    self.navigate_active_tab("about:welcome");
                }

                // Security badge indicator
                let is_secure = self.address_input.starts_with("https://");
                let is_internal = self.address_input.starts_with("about:");
                let (badge_text, badge_color) = if is_secure {
                    ("🔒 HTTPS", Color32::from_rgb(46, 204, 113))
                } else if is_internal {
                    ("📄 Internal", Color32::from_rgb(52, 152, 219))
                } else {
                    ("🔓 HTTP", Color32::from_rgb(230, 126, 34))
                };
                ui.label(RichText::new(badge_text).color(badge_color).small());

                // Address Omnibox
                let address_response = ui.add_sized(
                    Vec2::new(ui.available_width() - 220.0, 24.0),
                    egui::TextEdit::singleline(&mut self.address_input)
                        .hint_text("Nhập URL hoặc từ khóa tìm kiếm..."),
                );

                if address_response.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                    let target = self.address_input.clone();
                    self.navigate_active_tab(&target);
                }

                if ui.button("➔ Đi").clicked() {
                    let target = self.address_input.clone();
                    self.navigate_active_tab(&target);
                }

                // Zoom controls
                if ui.small_button("-").clicked() {
                    self.zoom_level = (self.zoom_level - 0.1).max(0.5);
                }
                ui.label(format!("{:.0}%", self.zoom_level * 100.0));
                if ui.small_button("+").clicked() {
                    self.zoom_level = (self.zoom_level + 0.1).min(2.0);
                }

                // DevTools toggle
                let devtools_btn = egui::Button::new("🛠 F12")
                    .fill(if self.show_devtools {
                        Color32::from_rgb(37, 99, 235)
                    } else {
                        Color32::from_rgb(60, 64, 75)
                    });
                if ui.add(devtools_btn).clicked() {
                    self.show_devtools = !self.show_devtools;
                }
            });
        });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("browser_status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(ref link) = self.hovered_link {
                    ui.label(RichText::new(format!("🔗 {}", link)).color(Color32::from_rgb(100, 180, 255)));
                } else if let Some(tab) = self.tabs.get(self.active_tab_idx) {
                    if tab.is_loading {
                        ui.label("⏳ Đang tải nội dung trang...");
                    } else {
                        ui.label(RichText::new("Sẵn sàng (Pure Rust Engine)").weak());
                    }
                }
            });
        });

        // DevTools side panel if open
        if self.show_devtools {
            egui::SidePanel::right("devtools_panel")
                .default_width(380.0)
                .min_width(280.0)
                .show(ctx, |ui| {
                    self.render_devtools(ui);
                });
        }

        // Main Web Page Viewport
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_page_viewport(ui, ctx);
        });
    }
}

impl BrowserApp {
    fn render_page_viewport(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let available_size = ui.available_size();
        let current_tab = match self.tabs.get_mut(self.active_tab_idx) {
            Some(t) => t,
            None => return,
        };

        // If viewport size changed significantly, recompute layout
        let target_width = (available_size.x / self.zoom_level).max(200.0);
        if (target_width - current_tab.last_layout_width).abs() > 10.0 {
            current_tab.recompute_layout(target_width);
        }

        let page_height = current_tab.page_height * self.zoom_level + 80.0;
        let mut clicked_href = None;
        let mut new_hovered_link = None;

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (response, painter) = ui.allocate_painter(
                    Vec2::new(available_size.x, page_height.max(available_size.y)),
                    egui::Sense::click_and_drag(),
                );

                // Background of the web page viewport
                painter.rect_filled(response.rect, 0.0, Color32::WHITE);

                let origin = response.rect.min;
                let offset = Vec2::new(origin.x, origin.y);

                if let Some(ref display_list) = current_tab.display_list {
                    paint_display_list(&painter, display_list, offset, self.zoom_level);

                    // Check hover on links
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

    fn render_devtools(&mut self, ui: &mut egui::Ui) {
        ui.heading("🛠 IronSurf DevTools");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.devtools_tab, DevToolsTab::DomTree, "DOM Tree");
            ui.selectable_value(&mut self.devtools_tab, DevToolsTab::Styles, "Styles");
            ui.selectable_value(&mut self.devtools_tab, DevToolsTab::Source, "HTML Source");
            ui.selectable_value(&mut self.devtools_tab, DevToolsTab::Network, "Network");
        });

        ui.separator();

        let tab = match self.tabs.get(self.active_tab_idx) {
            Some(t) => t,
            None => return,
        };

        match self.devtools_tab {
            DevToolsTab::DomTree => {
                ui.label(RichText::new("Cây phân cấp tài liệu DOM (Document Object Model):").small());
                ScrollArea::both().show(ui, |ui| {
                    if let Some(ref dom) = tab.dom_root {
                        render_dom_node_ui(ui, dom, 0, &mut self.selected_dom_node);
                    } else {
                        ui.label("Chưa có DOM");
                    }
                });
            }
            DevToolsTab::Styles => {
                ui.label(RichText::new("Các quy tắc CSS đang áp dụng:").small());
                ScrollArea::vertical().show(ui, |ui| {
                    for (i, rule) in tab.stylesheet.rules.iter().take(40).enumerate() {
                        let selectors_str = rule
                            .selectors
                            .iter()
                            .map(|s| format!("{:?}", s))
                            .collect::<Vec<_>>()
                            .join(", ");
                        ui.label(RichText::new(format!("[{}] {}", i + 1, selectors_str)).strong().color(Color32::from_rgb(90, 160, 240)));
                        for decl in &rule.declarations {
                            ui.label(format!("   {}: {:?}", decl.name, decl.value));
                        }
                        ui.add_space(4.0);
                    }
                });
            }
            DevToolsTab::Source => {
                ui.label(RichText::new("Mã HTML gốc của trang:").small());
                ScrollArea::both().show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut tab.raw_html.clone())
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY),
                    );
                });
            }
            DevToolsTab::Network => {
                ui.label(RichText::new("Thông tin mạng & Tài nguyên:").small());
                ui.add_space(6.0);
                ui.label(format!("URL hiện tại: {}", tab.url));
                ui.label(format!("Dung lượng HTML: {} bytes", tab.raw_html.len()));
                ui.label(format!("Tổng số quy tắc CSS: {}", tab.stylesheet.rules.len()));
                ui.label(format!("Chiều cao trang tính toán: {:.1} px", tab.page_height));
                ui.label("Giao thức TLS: Rustls 100% Pure Rust");
                ui.label("Engine: IronSurf 1.0 (Zero Chromium/Blink/WebKit)");
            }
        }
    }
}

fn render_dom_node_ui(
    ui: &mut egui::Ui,
    node: &Node,
    depth: usize,
    selected: &mut Option<String>,
) {
    if depth > 12 {
        return;
    }
    match &node.node_type {
        NodeType::Element(data) => {
            let mut label = format!("<{}", data.tag_name);
            if let Some(id) = data.attributes.get("id") {
                label.push_str(&format!(" id=\"{}\"", id));
            }
            if let Some(class) = data.attributes.get("class") {
                label.push_str(&format!(" class=\"{}\"", class));
            }
            label.push('>');

            let is_sel = selected.as_ref() == Some(&label);
            if node.children.is_empty() {
                if ui.selectable_label(is_sel, format!("{} (empty)", label)).clicked() {
                    *selected = Some(label);
                }
            } else {
                egui::CollapsingHeader::new(RichText::new(label.clone()).color(Color32::from_rgb(130, 190, 255)))
                    .default_open(depth < 2)
                    .show(ui, |ui| {
                        for child in &node.children {
                            render_dom_node_ui(ui, child, depth + 1, selected);
                        }
                    });
            }
        }
        NodeType::Text(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let preview = if trimmed.len() > 30 {
                    format!("\"{}...\"", &trimmed[..30])
                } else {
                    format!("\"{}\"", trimmed)
                };
                ui.label(RichText::new(format!("  {}", preview)).weak().italics());
            }
        }
        NodeType::Comment(comment) => {
            ui.label(RichText::new(format!("<!-- {} -->", comment.trim())).color(Color32::GRAY).italics());
        }
    }
}
