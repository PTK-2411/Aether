use crate::css::{CssParser, Stylesheet};
use crate::default_style::DEFAULT_USER_AGENT_CSS;
use crate::dom::{Node, NodeType};
use crate::html_parser::HtmlParser;
use crate::layout::{build_layout_tree, Dimensions, EdgeSizes, Rect};
use crate::network::{normalize_url, resolve_relative_url, NetworkManager, NetworkMessage};
use crate::render::{build_display_list, paint_display_list, DisplayList};
use crate::style::style_tree;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Key, Pos2, RichText,
    ScrollArea, Stroke, StrokeKind, Vec2,
};

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

pub struct BrowserApp {
    tabs: Vec<Tab>,
    active_tab_idx: usize,
    next_tab_id: usize,
    network: NetworkManager,
    address_input: String,
    center_search_input: String,
    hovered_link: Option<String>,
    show_devtools: bool,
    devtools_tab: DevToolsTab,
    selected_dom_node: Option<String>,
    zoom_level: f32,
    show_menu_popup: bool,
    show_more_shortcuts: bool,
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
            address_input: "".to_string(),
            center_search_input: "".to_string(),
            hovered_link: None,
            show_devtools: false,
            devtools_tab: DevToolsTab::DomTree,
            selected_dom_node: None,
            zoom_level: 1.0,
            show_menu_popup: false,
            show_more_shortcuts: false,
        };

        // Open initial New Tab
        app.open_new_tab("about:newtab".to_string());
        app
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

                                let layout_w = if tab.last_layout_width > 0.0 {
                                    tab.last_layout_width
                                } else {
                                    1000.0
                                };
                                tab.recompute_layout(layout_w);
                            }
                            Err(err) => {
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_network_responses();

        // Apply dark mode visuals matching Google Chrome dark theme
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(Color32::from_rgb(232, 234, 237));
        visuals.panel_fill = Color32::from_rgb(32, 33, 36); // #202124
        visuals.window_fill = Color32::from_rgb(40, 42, 45);
        ctx.set_visuals(visuals);

        // Global hotkey F12 toggle DevTools
        if ctx.input(|i| i.key_pressed(Key::F12)) {
            self.show_devtools = !self.show_devtools;
        }

        // Top Chrome Panel (Tabs + Navigation Bar)
        egui::TopBottomPanel::top("chrome_top_panel")
            .frame(Frame::NONE.fill(Color32::from_rgb(32, 33, 36)))
            .show(ctx, |ui| {
                self.render_tab_strip(ui);
                self.render_navigation_bar(ui, ctx);
            });

        // DevTools side panel if open
        if self.show_devtools {
            egui::SidePanel::right("devtools_panel")
                .default_width(380.0)
                .min_width(280.0)
                .frame(Frame::NONE.fill(Color32::from_rgb(28, 30, 33)))
                .show(ctx, |ui| {
                    self.render_devtools(ui);
                });
        }

        // Main Web Viewport or Google New Tab Start Page
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::from_rgb(53, 54, 58))) // #35363a
            .show(ctx, |ui| {
                let is_new_tab = self.tabs.get(self.active_tab_idx).map_or(true, |t| t.is_new_tab());
                if is_new_tab {
                    self.render_google_new_tab_page(ui, ctx);
                } else {
                    self.render_page_viewport(ui, ctx);
                }
            });
    }
}

impl BrowserApp {
    /// Renders the Chrome Tab Strip (Tabs, + Button, Window Controls)
    fn render_tab_strip(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let mut close_tab_idx = None;

            for (idx, tab) in self.tabs.iter().enumerate() {
                let is_active = idx == self.active_tab_idx;

                let tab_bg = if is_active {
                    Color32::from_rgb(50, 54, 57) // #323639
                } else {
                    Color32::TRANSPARENT
                };

                let tab_text_color = if is_active {
                    Color32::from_rgb(241, 243, 244)
                } else {
                    Color32::from_rgb(154, 160, 166)
                };

                let frame = Frame::NONE
                    .fill(tab_bg)
                    .corner_radius(CornerRadius {
                        nw: 8,
                        ne: 8,
                        sw: 0,
                        se: 0,
                    })
                    .inner_margin(egui::Margin::symmetric(10, 6));

                let resp = frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // Google "G" icon or globe
                        if tab.is_loading {
                            ui.label(RichText::new("●").size(10.0).color(Color32::from_rgb(251, 188, 4)));
                        } else if tab.is_new_tab() {
                            // Mini Google G circle
                            let (g_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
                            ui.painter().circle_filled(g_rect.center(), 7.0, Color32::from_rgb(66, 133, 244));
                            ui.painter().text(g_rect.center(), Align2::CENTER_CENTER, "G", FontId::proportional(9.0), Color32::WHITE);
                        } else {
                            let (w_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
                            ui.painter().circle_stroke(w_rect.center(), 6.0, Stroke::new(1.0_f32, Color32::from_rgb(154, 160, 166)));
                            ui.painter().line_segment([Pos2::new(w_rect.min.x, w_rect.center().y), Pos2::new(w_rect.max.x, w_rect.center().y)], Stroke::new(1.0_f32, Color32::from_rgb(154, 160, 166)));
                        }

                        // Tab title
                        let display_title = if tab.title.is_empty() { "New Tab" } else { &tab.title };
                        let label = ui.add(
                            egui::Label::new(
                                RichText::new(display_title)
                                    .size(12.0)
                                    .color(tab_text_color),
                            )
                            .truncate(),
                        );
                        if label.clicked() {
                            self.active_tab_idx = idx;
                            self.address_input = if tab.is_new_tab() { String::new() } else { tab.url.clone() };
                        }

                        // Vector Close button 'x' (Guaranteed 100% vector drawn, NEVER a box)
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
                    });
                });

                if resp.response.clicked() {
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

            // Right-side window controls (Minimize, Maximize, Close - vector drawn)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // Window close
                let (w_close_rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::click());
                let wc = w_close_rect.center();
                ui.painter().line_segment([wc - Vec2::splat(3.5), wc + Vec2::splat(3.5)], Stroke::new(1.2_f32, Color32::from_rgb(154, 160, 166)));
                ui.painter().line_segment([Pos2::new(wc.x - 3.5, wc.y + 3.5), Pos2::new(wc.x + 3.5, wc.y - 3.5)], Stroke::new(1.2_f32, Color32::from_rgb(154, 160, 166)));

                ui.add_space(10.0);

                // Window maximize
                let (w_max_rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::click());
                let wm_box = egui::Rect::from_center_size(w_max_rect.center(), Vec2::new(8.0, 8.0));
                ui.painter().rect_stroke(wm_box, 1.0_f32, Stroke::new(1.2_f32, Color32::from_rgb(154, 160, 166)), StrokeKind::Inside);

                ui.add_space(10.0);

                // Window minimize
                let (w_min_rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::click());
                let wmc = w_min_rect.center();
                ui.painter().line_segment([Pos2::new(wmc.x - 4.0, wmc.y), Pos2::new(wmc.x + 4.0, wmc.y)], Stroke::new(1.2_f32, Color32::from_rgb(154, 160, 166)));
            });
        });
    }

    /// Renders the Chrome Navigation Toolbar (Back, Forward, Reload, Omnibox, AI Mode, Profile)
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
            let omnibox_width = ui.available_width() - 175.0;
            let omnibox_h = 34.0;

            let (response, painter) = ui.allocate_painter(Vec2::new(omnibox_width.max(200.0), omnibox_h), egui::Sense::hover());

            // Omnibox pill background
            let omnibox_bg = Color32::from_rgb(40, 41, 44); // #28292c
            let border_color = Color32::from_rgb(68, 71, 70); // #444746
            painter.rect_filled(response.rect, 17.0, omnibox_bg);
            painter.rect_stroke(response.rect, 17.0, Stroke::new(1.0_f32, border_color), StrokeKind::Inside);

            // Inside Omnibox layout
            let child_rect = response.rect.shrink2(Vec2::new(10.0, 4.0));
            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(child_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));

            // Google "G" icon on left
            child_ui.label(RichText::new("G").size(14.0).strong().color(Color32::from_rgb(66, 133, 244)));
            child_ui.add_space(6.0);

            // URL TextEdit
            let available_edit_w = child_ui.available_width() - 85.0;
            let text_edit = child_ui.add_sized(
                Vec2::new(available_edit_w.max(50.0), 24.0),
                egui::TextEdit::singleline(&mut self.address_input)
                    .hint_text("Search Google or type a URL")
                    .frame(false)
                    .text_color(Color32::from_rgb(232, 234, 237)),
            );

            if text_edit.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                let target = self.address_input.clone();
                self.navigate_active_tab(&target);
            }

            // AI Mode pill inside Omnibox right side
            child_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |child_ui| {
                let ai_pill = child_ui.add(
                    egui::Button::new(
                        RichText::new("AI Mode")
                            .size(11.0)
                            .color(Color32::from_rgb(168, 199, 250)),
                    )
                    .fill(Color32::from_rgb(48, 50, 54))
                    .corner_radius(12)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(95, 99, 104))),
                );
                if ai_pill.clicked() {
                    self.navigate_active_tab("https://gemini.google.com");
                }
            });

            // Right side of Toolbar: User profile avatar ("Hi, Khoa") and Menu ⋮
            ui.add_space(4.0);

            // User profile pill: "Hi, Khoa" with clean vector avatar (NO emoji square box!)
            let (p_rect, p_resp) = ui.allocate_exact_size(Vec2::new(82.0, 28.0), egui::Sense::click());
            if p_resp.hovered() {
                ui.painter().rect_filled(p_rect, 14.0, Color32::from_rgb(52, 54, 58));
            } else {
                ui.painter().rect_filled(p_rect, 14.0, Color32::from_rgb(40, 41, 44));
            }
            ui.painter().rect_stroke(p_rect, 14.0, Stroke::new(1.0_f32, Color32::from_rgb(68, 71, 70)), StrokeKind::Inside);
            let avatar_center = Pos2::new(p_rect.min.x + 14.0, p_rect.center().y);
            ui.painter().circle_filled(avatar_center, 8.0, Color32::from_rgb(26, 115, 232));
            ui.painter().text(avatar_center, Align2::CENTER_CENTER, "K", FontId::proportional(10.0), Color32::WHITE);
            ui.painter().text(Pos2::new(p_rect.min.x + 27.0, p_rect.center().y), Align2::LEFT_CENTER, "Hi, Khoa", FontId::proportional(11.0), Color32::from_rgb(232, 234, 237));
            if p_resp.clicked() {
                self.show_devtools = !self.show_devtools;
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
                egui::Window::new("Menu")
                    .anchor(Align2::RIGHT_TOP, Vec2::new(-10.0, 75.0))
                    .collapsible(false)
                    .resizable(false)
                    .frame(Frame::NONE.fill(Color32::from_rgb(43, 45, 48)).corner_radius(8).inner_margin(8))
                    .show(ctx, |ui| {
                        if ui.button("New Tab").clicked() {
                            self.open_new_tab("about:newtab".to_string());
                            self.show_menu_popup = false;
                        }
                        if ui.button("DevTools Inspector (F12)").clicked() {
                            self.show_devtools = !self.show_devtools;
                            self.show_menu_popup = false;
                        }
                        if ui.button("Reload").clicked() {
                            self.reload();
                            self.show_menu_popup = false;
                        }
                        if ui.button("CSS Test Suite (about:test)").clicked() {
                            self.navigate_active_tab("about:test");
                            self.show_menu_popup = false;
                        }
                        if ui.button("Engine Architecture (about:credits)").clicked() {
                            self.navigate_active_tab("about:credits");
                            self.show_menu_popup = false;
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("-").clicked() {
                                self.zoom_level = (self.zoom_level - 0.1).max(0.5);
                            }
                            ui.label(format!("{:.0}%", self.zoom_level * 100.0));
                            if ui.button("+").clicked() {
                                self.zoom_level = (self.zoom_level + 0.1).min(2.0);
                            }
                        });
                    });
            }
        });
        ui.add_space(4.0);
    }

    /// Renders the exact Google Dark Mode Start Page from Image 3
    fn render_google_new_tab_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ScrollArea::vertical().show(ui, |ui| {
            // Top Right Bar: Gmail, Images, 9-dot App Grid, Profile Picture
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(16.0);

                    // Profile Circle with landscape / avatar and blue border
                    let (profile_rect, profile_resp) = ui.allocate_exact_size(Vec2::new(32.0, 32.0), egui::Sense::click());
                    let p_center = profile_rect.center();
                    ui.painter().circle_filled(p_center, 15.0, Color32::from_rgb(66, 133, 244));
                    ui.painter().circle_filled(p_center, 13.0, Color32::from_rgb(45, 120, 110));
                    ui.painter().text(p_center, Align2::CENTER_CENTER, "K", FontId::proportional(14.0), Color32::WHITE);
                    if profile_resp.clicked() {
                        self.show_devtools = !self.show_devtools;
                    }

                    ui.add_space(8.0);

                    // 9-dot Grid Icon (Google Apps)
                    let (grid_rect, grid_resp) = ui.allocate_exact_size(Vec2::new(28.0, 28.0), egui::Sense::click());
                    let g_center = grid_rect.center();
                    for r in -1..=1 {
                        for c in -1..=1 {
                            let dot_pos = Pos2::new(g_center.x + (c as f32) * 5.0, g_center.y + (r as f32) * 5.0);
                            ui.painter().circle_filled(dot_pos, 1.5, Color32::from_rgb(232, 234, 237));
                        }
                    }
                    if grid_resp.clicked() {
                        self.navigate_active_tab("https://about.google/products/");
                    }

                    ui.add_space(8.0);

                    // Images link
                    if ui.add(egui::Link::new(RichText::new("Images").size(13.0).color(Color32::from_rgb(232, 234, 237)))).clicked() {
                        self.navigate_active_tab("https://images.google.com");
                    }

                    ui.add_space(8.0);

                    // Gmail link
                    if ui.add(egui::Link::new(RichText::new("Gmail").size(13.0).color(Color32::from_rgb(232, 234, 237)))).clicked() {
                        self.navigate_active_tab("https://mail.google.com");
                    }
                });
            });

            ui.add_space(50.0);

            // Center Content Container
            ui.vertical_centered(|ui| {
                // 1. "Google" Logo: Big prominent clean white text matching Image 3
                ui.label(
                    RichText::new("Google")
                        .size(80.0)
                        .strong()
                        .color(Color32::WHITE),
                );

                ui.add_space(26.0);

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

                // Text Edit: "Ask Google"
                let input_width = inner_ui.available_width() - 175.0;
                let center_edit = inner_ui.add_sized(
                    Vec2::new(input_width.max(100.0), 30.0),
                    egui::TextEdit::singleline(&mut self.center_search_input)
                        .hint_text("Ask Google")
                        .frame(false)
                        .text_color(Color32::WHITE)
                        .font(FontId::proportional(16.0)),
                );

                if center_edit.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                    let query = self.center_search_input.clone();
                    if !query.trim().is_empty() {
                        self.navigate_active_tab(&query);
                    }
                }

                // Right icons inside Search Bar: Mic, Lens/Camera, AI Mode (All Vector drawn)
                inner_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |inner_ui| {
                    let ai_btn = inner_ui.add(
                        egui::Button::new(
                            RichText::new("AI Mode")
                                .size(12.0)
                                .color(Color32::from_rgb(232, 234, 237)),
                        )
                        .fill(Color32::from_rgb(55, 57, 60))
                        .corner_radius(14)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(95, 99, 104))),
                    );
                    if ai_btn.clicked() {
                        self.navigate_active_tab("https://gemini.google.com");
                    }

                    inner_ui.add_space(8.0);

                    // Camera / Lens icon (Vector drawn)
                    let (cam_rect, cam_resp) = inner_ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::click());
                    let cc = cam_rect.center();
                    let cam_box = egui::Rect::from_center_size(cc, Vec2::new(14.0, 10.0));
                    inner_ui.painter().rect_stroke(cam_box, 2.0_f32, Stroke::new(1.2_f32, Color32::from_rgb(180, 185, 190)), StrokeKind::Inside);
                    inner_ui.painter().circle_stroke(cc, 2.5, Stroke::new(1.2_f32, Color32::from_rgb(180, 185, 190)));
                    if cam_resp.clicked() {
                        self.navigate_active_tab("https://lens.google.com");
                    }

                    inner_ui.add_space(8.0);

                    // Microphone icon (Vector drawn)
                    let (mic_rect, _mic_resp) = inner_ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::click());
                    let mc = mic_rect.center();
                    let mic_body = egui::Rect::from_center_size(Pos2::new(mc.x, mc.y - 2.0), Vec2::new(6.0, 9.0));
                    inner_ui.painter().rect_filled(mic_body, 3.0, Color32::from_rgb(180, 185, 190));
                    inner_ui.painter().line_segment([Pos2::new(mc.x, mc.y + 4.0), Pos2::new(mc.x, mc.y + 7.0)], Stroke::new(1.2_f32, Color32::from_rgb(180, 185, 190)));
                });

                ui.add_space(18.0);

                // 3. Two Suggestion Action Chips (Cards underneath search bar)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    // Calculate centering offset
                    let total_chips_w = 440.0;
                    let left_pad = ((ui.available_width() - total_chips_w) / 2.0).max(0.0);
                    ui.add_space(left_pad);

                    // Chip 1: "Do more with AI Mode - Create, summarize, plan"
                    let chip1 = Frame::NONE
                        .fill(Color32::from_rgb(48, 50, 54))
                        .corner_radius(16)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (star_rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
                                draw_sparkle_icon(ui.painter(), star_rect.center(), 7.0, Color32::from_rgb(168, 199, 250));
                                ui.add_space(4.0);
                                ui.vertical(|ui| {
                                    ui.label(RichText::new("Do more with AI Mode").size(12.0).strong().color(Color32::WHITE));
                                    ui.label(RichText::new("Create, summarize, plan").size(10.0).color(Color32::from_rgb(154, 160, 166)));
                                });
                            });
                        });

                    if chip1.response.interact(egui::Sense::click()).clicked() {
                        self.navigate_active_tab("https://gemini.google.com");
                    }

                    // Chip 2: "Create images - Reimagine, illustrate, edit"
                    let chip2 = Frame::NONE
                        .fill(Color32::from_rgb(48, 50, 54))
                        .corner_radius(16)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67)))
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (cam_chip, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
                                draw_camera_icon(ui.painter(), cam_chip.center(), 6.0, Color32::from_rgb(241, 180, 70));
                                ui.add_space(4.0);
                                ui.vertical(|ui| {
                                    ui.label(RichText::new("Create images").size(12.0).strong().color(Color32::WHITE));
                                    ui.label(RichText::new("Reimagine, illustrate, edit").size(10.0).color(Color32::from_rgb(154, 160, 166)));
                                });
                            });
                        });

                    if chip2.response.interact(egui::Sense::click()).clicked() {
                        self.navigate_active_tab("https://images.google.com");
                    }
                });

                ui.add_space(28.0);

                // 4. Seven Quick Shortcut Icons (Row of circles with vector graphics)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 24.0;

                    let total_shortcuts_w = 7.0 * 68.0;
                    let pad_shortcuts = ((ui.available_width() - total_shortcuts_w) / 2.0).max(0.0);
                    ui.add_space(pad_shortcuts);

                    // 1. Facebook
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Letter("f"),
                        Color32::from_rgb(24, 119, 242), // FB blue
                        "Facebook",
                        || self.navigate_active_tab("https://facebook.com"),
                    );

                    // 2. YouTube
                    render_vector_shortcut(
                        ui,
                        VectorIcon::PlayTriangle,
                        Color32::from_rgb(255, 0, 0), // YouTube red
                        "YouTube",
                        || self.navigate_active_tab("https://youtube.com"),
                    );

                    // 3. Trang chủ
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Letter("C"),
                        Color32::from_rgb(0, 128, 128), // Teal
                        "Trang chủ",
                        || self.navigate_active_tab("about:test"),
                    );

                    // 4. Google Colab
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Infinity,
                        Color32::from_rgb(227, 116, 0), // Amber
                        "Google Colab",
                        || self.navigate_active_tab("https://colab.research.google.com"),
                    );

                    // 5. Google Gemini
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Sparkle,
                        Color32::from_rgb(66, 133, 244), // Gemini blue
                        "Google Gemini",
                        || self.navigate_active_tab("https://gemini.google.com"),
                    );

                    // 6. Instagram
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Camera,
                        Color32::from_rgb(214, 41, 118), // Instagram magenta
                        "Instagram",
                        || self.navigate_active_tab("https://instagram.com"),
                    );

                    // 7. Show more
                    render_vector_shortcut(
                        ui,
                        VectorIcon::Plus,
                        Color32::from_rgb(60, 64, 67), // Dark grey
                        "Show more",
                        || {
                            self.show_more_shortcuts = !self.show_more_shortcuts;
                        },
                    );
                });

                if self.show_more_shortcuts {
                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 24.0;
                        let extra_w = 4.0 * 68.0;
                        let pad_extra = ((ui.available_width() - extra_w) / 2.0).max(0.0);
                        ui.add_space(pad_extra);

                        render_vector_shortcut(
                            ui,
                            VectorIcon::Letter("G"),
                            Color32::from_rgb(66, 133, 244),
                            "Google",
                            || self.navigate_active_tab("https://google.com"),
                        );

                        render_vector_shortcut(
                            ui,
                            VectorIcon::Letter("R"),
                            Color32::from_rgb(230, 80, 40),
                            "Rust Lang",
                            || self.navigate_active_tab("https://rust-lang.org"),
                        );

                        render_vector_shortcut(
                            ui,
                            VectorIcon::Letter("T"),
                            Color32::from_rgb(52, 168, 83),
                            "CSS Test",
                            || self.navigate_active_tab("about:test"),
                        );

                        render_vector_shortcut(
                            ui,
                            VectorIcon::Letter("i"),
                            Color32::from_rgb(142, 68, 173),
                            "Credits",
                            || self.navigate_active_tab("about:credits"),
                        );
                    });
                }
            });

            // 5. Bottom Right: "Customize Chrome" Button
            ui.add_space(60.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(20.0);
                    let customize_btn = ui.add(
                        egui::Button::new(
                            RichText::new("Customize Chrome")
                                .size(12.0)
                                .color(Color32::from_rgb(232, 234, 237)),
                        )
                        .fill(Color32::from_rgb(48, 50, 54))
                        .corner_radius(16)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(72, 75, 80))),
                    );
                    if customize_btn.clicked() {
                        self.show_devtools = !self.show_devtools;
                    }
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
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (response, painter) = ui.allocate_painter(
                    Vec2::new(available_size.x, page_height.max(available_size.y)),
                    egui::Sense::click_and_drag(),
                );

                // Webpage canvas background
                painter.rect_filled(response.rect, 0.0, Color32::WHITE);

                let origin = response.rect.min;
                let offset = Vec2::new(origin.x, origin.y);

                if let Some(ref display_list) = current_tab.display_list {
                    paint_display_list(&painter, display_list, offset, self.zoom_level);

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

    /// Renders the DevTools Inspector
    fn render_devtools(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("DevTools").size(16.0).color(Color32::WHITE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✕").clicked() {
                    self.show_devtools = false;
                }
            });
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.devtools_tab, DevToolsTab::DomTree, "DOM");
            ui.selectable_value(&mut self.devtools_tab, DevToolsTab::Styles, "CSS");
            ui.selectable_value(&mut self.devtools_tab, DevToolsTab::Source, "Source");
            ui.selectable_value(&mut self.devtools_tab, DevToolsTab::Network, "Network");
        });

        ui.separator();

        let tab = match self.tabs.get(self.active_tab_idx) {
            Some(t) => t,
            None => return,
        };

        match self.devtools_tab {
            DevToolsTab::DomTree => {
                ScrollArea::both().show(ui, |ui| {
                    if let Some(ref dom) = tab.dom_root {
                        render_dom_node_ui(ui, dom, 0, &mut self.selected_dom_node);
                    } else {
                        ui.label(RichText::new("No DOM available on New Tab page").weak());
                    }
                });
            }
            DevToolsTab::Styles => {
                ScrollArea::vertical().show(ui, |ui| {
                    for (i, rule) in tab.stylesheet.rules.iter().take(40).enumerate() {
                        let selectors_str = rule
                            .selectors
                            .iter()
                            .map(|s| format!("{:?}", s))
                            .collect::<Vec<_>>()
                            .join(", ");
                        ui.label(RichText::new(format!("[{}] {}", i + 1, selectors_str)).strong().color(Color32::from_rgb(138, 180, 248)));
                        for decl in &rule.declarations {
                            ui.label(format!("   {}: {:?}", decl.name, decl.value));
                        }
                        ui.add_space(4.0);
                    }
                });
            }
            DevToolsTab::Source => {
                ScrollArea::both().show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut tab.raw_html.clone())
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY),
                    );
                });
            }
            DevToolsTab::Network => {
                ui.label(format!("Active URL: {}", tab.url));
                ui.label(format!("Document Size: {} bytes", tab.raw_html.len()));
                ui.label(format!("CSS Rules Loaded: {}", tab.stylesheet.rules.len()));
                ui.label(format!("Render Height: {:.1} px", tab.page_height));
                ui.label("TLS Implementation: Rustls (Pure Rust)");
                ui.label("Engine: IronSurf 100% Pure Rust (No Chromium/Blink)");
            }
        }
    }
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
    ui.vertical_centered(|ui| {
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
                ui.painter().text(center, Align2::CENTER_CENTER, s, FontId::proportional(22.0), Color32::WHITE);
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

        ui.add_space(6.0);
        ui.label(RichText::new(label).size(12.0).color(Color32::from_rgb(232, 234, 237)));

        if resp.clicked() {
            on_click();
        }
    });
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
                egui::CollapsingHeader::new(RichText::new(label.clone()).color(Color32::from_rgb(138, 180, 248)))
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
