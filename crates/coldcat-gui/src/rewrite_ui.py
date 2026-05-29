import re
import os

filepath = r"e:\code shit\Cold Cat\crates\coldcat-gui\src\main.rs"
with open(filepath, "r", encoding="utf-8") as f:
    content = f.read()

# 1. Update Colors
new_colors = """
const TEXT: Color32 = Color32::from_rgb(17, 24, 39);
const TEXT_MUTED: Color32 = Color32::from_rgb(100, 116, 139);
const SIDEBAR_BG: Color32 = Color32::from_rgb(30, 41, 59); // Dark navy
const SIDEBAR_TEXT: Color32 = Color32::from_rgb(241, 245, 249);
const SIDEBAR_TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184);
const SIDEBAR_HOVER: Color32 = Color32::from_rgb(51, 65, 85);
const SIDEBAR_ACTIVE: Color32 = Color32::from_rgb(15, 23, 42);

const MAIN_BG: Color32 = Color32::from_rgb(248, 250, 252);
const CARD_BG: Color32 = Color32::from_rgb(255, 255, 255);
const BORDER: Color32 = Color32::from_rgb(226, 232, 240);

const ACCENT_GREEN: Color32 = Color32::from_rgb(16, 185, 129); // ON / Start
const ACCENT_RED: Color32 = Color32::from_rgb(239, 68, 68);    // Danger
const ACCENT_BLUE: Color32 = Color32::from_rgb(59, 130, 246);  // Links / Primary
const ACCENT_GRAY: Color32 = Color32::from_rgb(203, 213, 225); // Disabled Toggle
"""

# Replace the color constants block (lines 14 to 26 approximately)
color_pattern = re.compile(r'const TEXT: Color32.*?(?=\nfn main\(\))', re.DOTALL)
content = color_pattern.sub(new_colors.strip() + "\n", content)

# 2. Update apply_theme
new_theme = """
fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.panel_fill = MAIN_BG;
    style.visuals.window_fill = CARD_BG;
    style.visuals.widgets.active.bg_fill = ACCENT_BLUE;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(239, 246, 255);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(248, 250, 252);
    style.visuals.widgets.noninteractive.bg_fill = CARD_BG;
    style.visuals.selection.bg_fill = ACCENT_BLUE;
    style.spacing.item_spacing = egui::vec2(12.0, 12.0);
    style.spacing.button_padding = egui::vec2(14.0, 9.0);
    style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(15.5));
    style.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
    style.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
    ctx.set_style(style);
}

fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = egui::vec2(50.0, 26.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *on, ""));
    
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, *on);
        let bg_fill = if *on { ACCENT_GREEN } else { ACCENT_GRAY };
        let rounding = rect.height() / 2.0;
        ui.painter().rect_filled(rect, rounding, bg_fill);
        let circle_x = if *on { rect.right() - rounding } else { rect.left() + rounding };
        let circle_center = egui::pos2(circle_x, rect.center().y);
        ui.painter().circle_filled(circle_center, rounding - 2.0, Color32::WHITE);
    }
    response
}
"""
theme_pattern = re.compile(r'fn apply_theme\(ctx: &egui::Context\) \{.*?(?=\nfn card<R>)', re.DOTALL)
content = theme_pattern.sub(new_theme.strip() + "\n", content)

# 3. Modify Page and NavIcon
new_page = """
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Blocks,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockTab {
    Websites,
    Applications,
    Locking,
}
"""
page_pattern = re.compile(r'#\[derive\(Debug, Clone, Copy, PartialEq, Eq\)\]\nenum Page \{.*?\n\}', re.DOTALL)
content = page_pattern.sub(new_page.strip(), content)

nav_pattern = re.compile(r'#\[derive\(Debug, Clone, Copy, PartialEq, Eq\)\]\nenum NavIcon \{.*?\n\}', re.DOTALL)
content = nav_pattern.sub("", content) # Remove NavIcon entirely

# 4. We will replace the entire ColdCatApp impl to completely overhaul the UI
app_start = content.find("struct ColdCatApp {")
app_end = content.find("fn apply_theme(ctx: &egui::Context)")

new_app_code = """
#[derive(Debug)]
struct ColdCatApp {
    page: Page,
    profiles: Vec<BlockProfile>,
    selected_profile: usize,
    active_tab: BlockTab,
    manual_site: String,
    manual_app: String,
    app_filter: String,
    app_candidates: Vec<AppCandidate>,
    last_app_scan: Option<Instant>,
    status: Option<ServiceStatus>,
    status_fetched_at: Option<Instant>,
    auto_refresh: bool,
    last_auto_refresh: Instant,
    message: String,
    error: Option<String>,
}

impl Default for ColdCatApp {
    fn default() -> Self {
        let mut profiles = load_profiles();
        if profiles.is_empty() {
            profiles = built_in_profiles();
            let _ = save_profiles(&profiles);
        }
        let mut app = Self {
            page: Page::Blocks,
            profiles,
            selected_profile: 0,
            active_tab: BlockTab::Websites,
            manual_site: String::new(),
            manual_app: String::new(),
            app_filter: String::new(),
            app_candidates: Vec::new(),
            last_app_scan: None,
            status: None,
            status_fetched_at: None,
            auto_refresh: true,
            last_auto_refresh: Instant::now(),
            message: "Ready.".to_string(),
            error: None,
        };
        app.scan_apps();
        app
    }
}

impl eframe::App for ColdCatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.maybe_auto_refresh();

        egui::SidePanel::left("coldcat_sidebar")
            .exact_width(260.0)
            .frame(egui::Frame::default().fill(SIDEBAR_BG))
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(SIDEBAR_TEXT);
                egui::ScrollArea::vertical().show(ui, |ui| self.sidebar(ui));
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(MAIN_BG).inner_margin(egui::Margin::same(24.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    match self.page {
                        Page::Blocks => self.blocks_page(ui),
                        Page::Settings => self.settings_page(ui),
                    }
                });
            });
    }
}

impl ColdCatApp {
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.heading(RichText::new("Cold Cat").size(24.0).strong());
        });
        ui.add_space(30.0);

        self.nav_button(ui, Page::Blocks, "Blocks");
        self.nav_button(ui, Page::Settings, "Settings");

        ui.add_space(ui.available_height() - 60.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let state = self.status.as_ref().map(|s| state_label(&s.state)).unwrap_or("Unknown");
            ui.label(RichText::new(format!("Daemon: {}", state)).color(SIDEBAR_TEXT_MUTED));
        });
    }

    fn nav_button(&mut self, ui: &mut egui::Ui, page: Page, title: &str) {
        let selected = self.page == page;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        if response.clicked() { self.page = page; }
        
        let fill = if selected { SIDEBAR_ACTIVE } else if response.hovered() { SIDEBAR_HOVER } else { Color32::TRANSPARENT };
        ui.painter().rect_filled(rect, 0.0, fill);
        
        if selected {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top()), egui::vec2(4.0, rect.height())),
                0.0, ACCENT_GREEN
            );
        }
        ui.painter().text(
            rect.left_top() + egui::vec2(20.0, 14.0),
            egui::Align2::LEFT_TOP, title, egui::FontId::proportional(16.0),
            if selected { SIDEBAR_TEXT } else { SIDEBAR_TEXT_MUTED }
        );
    }

    fn blocks_page(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            cols[0].set_width(240.0);
            
            // Block List
            card(&mut cols[0], |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Blocks");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+").clicked() {
                            self.profiles.push(BlockProfile {
                                name: "New Block".to_string(),
                                duration: "1h".to_string(),
                                sites: vec![],
                                apps: vec![],
                            });
                            self.selected_profile = self.profiles.len() - 1;
                            let _ = save_profiles(&self.profiles);
                        }
                    });
                });
                ui.add_space(8.0);
                
                let mut to_delete = None;
                for (i, profile) in self.profiles.iter().enumerate() {
                    let selected = self.selected_profile == i;
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click());
                    if response.clicked() { self.selected_profile = i; }
                    
                    let bg = if selected { Color32::from_rgb(239, 246, 255) } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(rect, 6.0, bg);
                    ui.painter().text(
                        rect.left_top() + egui::vec2(12.0, 12.0), egui::Align2::LEFT_TOP,
                        &profile.name, egui::FontId::proportional(15.0), if selected { ACCENT_BLUE } else { TEXT }
                    );
                    
                    if selected && self.profiles.len() > 1 {
                        let del_rect = egui::Rect::from_min_size(rect.right_top() - egui::vec2(30.0, -10.0), egui::vec2(20.0, 20.0));
                        let del_resp = ui.interact(del_rect, ui.id().with(i), egui::Sense::click());
                        ui.painter().text(del_rect.center(), egui::Align2::CENTER_CENTER, "🗑", egui::FontId::proportional(14.0), ACCENT_RED);
                        if del_resp.clicked() { to_delete = Some(i); }
                    }
                }
                
                if let Some(i) = to_delete {
                    self.profiles.remove(i);
                    self.selected_profile = self.selected_profile.min(self.profiles.len().saturating_sub(1));
                    let _ = save_profiles(&self.profiles);
                }
            });
            
            // Block Editor
            if self.profiles.is_empty() { return; }
            if self.selected_profile >= self.profiles.len() { self.selected_profile = 0; }
            
            let profile = &mut self.profiles[self.selected_profile];
            card(&mut cols[1], |ui| {
                ui.horizontal(|ui| {
                    let name_resp = ui.add(egui::TextEdit::singleline(&mut profile.name).font(egui::FontId::proportional(24.0)).frame(false));
                    if name_resp.changed() { let _ = save_profiles(&self.profiles); }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Toggle Switch
                        let is_active = self.status.as_ref().map(|s| s.active_sessions.iter().any(|a| a.name == profile.name)).unwrap_or(false);
                        let mut toggle_state = is_active;
                        
                        ui.add_enabled_ui(!is_active, |ui| {
                            let resp = toggle_switch(ui, &mut toggle_state);
                            if resp.changed() && toggle_state && !is_active {
                                // Turn ON
                                let duration_secs = parse_duration_secs(&profile.duration).unwrap_or(3600);
                                self.apply_response(
                                    send_command(CommandRequest::Start {
                                        name: profile.name.clone(),
                                        duration_secs,
                                        sites: profile.sites.clone(),
                                        apps: profile.apps.clone(),
                                    })
                                );
                            }
                        });
                    });
                });
                
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.active_tab == BlockTab::Websites, "Websites").clicked() { self.active_tab = BlockTab::Websites; }
                    if ui.selectable_label(self.active_tab == BlockTab::Applications, "Applications").clicked() { self.active_tab = BlockTab::Applications; }
                    if ui.selectable_label(self.active_tab == BlockTab::Locking, "Locking").clicked() { self.active_tab = BlockTab::Locking; }
                });
                ui.separator();
                ui.add_space(8.0);
                
                match self.active_tab {
                    BlockTab::Websites => self.websites_tab(ui, profile),
                    BlockTab::Applications => self.apps_tab(ui, profile),
                    BlockTab::Locking => self.locking_tab(ui, profile),
                }
            });
        });
    }
    
    fn websites_tab(&mut self, ui: &mut egui::Ui, profile: &mut BlockProfile) {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.manual_site);
            if primary_button(ui, "Add site", 80.0, 30.0).clicked() {
                for site in parse_rule_text(&self.manual_site) { add_unique_rule(&mut profile.sites, site); }
                self.manual_site.clear();
                let _ = save_profiles(&self.profiles);
            }
        });
        ui.add_space(12.0);
        
        let mut remove = None;
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for (i, site) in profile.sites.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(site);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("X").clicked() { remove = Some(i); }
                    });
                });
            }
        });
        if let Some(i) = remove {
            profile.sites.remove(i);
            let _ = save_profiles(&self.profiles);
        }
    }
    
    fn apps_tab(&mut self, ui: &mut egui::Ui, profile: &mut BlockProfile) {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.manual_app);
            if primary_button(ui, "Add app", 80.0, 30.0).clicked() {
                for app in parse_rule_text(&self.manual_app) { add_unique_rule(&mut profile.apps, app); }
                self.manual_app.clear();
                let _ = save_profiles(&self.profiles);
            }
        });
        ui.add_space(12.0);
        
        let mut remove = None;
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for (i, app) in profile.apps.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(app);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("X").clicked() { remove = Some(i); }
                    });
                });
            }
        });
        if let Some(i) = remove {
            profile.apps.remove(i);
            let _ = save_profiles(&self.profiles);
        }
    }
    
    fn locking_tab(&mut self, ui: &mut egui::Ui, profile: &mut BlockProfile) {
        ui.label(RichText::new("Locking limits changes to an active block until the timer expires.").color(TEXT_MUTED));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label("Duration:");
            let resp = ui.text_edit_singleline(&mut profile.duration);
            if resp.changed() { let _ = save_profiles(&self.profiles); }
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for q in ["25m", "1h", "2h", "4h", "8h"] {
                if ui.button(q).clicked() {
                    profile.duration = q.to_string();
                    let _ = save_profiles(&self.profiles);
                }
            }
        });
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Daemon Status");
            if let Some(status) = &self.status {
                ui.label(format!("State: {}", state_label(&status.state)));
                if !status.active_sessions.is_empty() {
                    ui.label("Active Blocks:");
                    for s in &status.active_sessions {
                        let rem = s.end_time.saturating_sub(status.now);
                        ui.label(format!(" - {} ({}s remaining)", s.name, rem));
                    }
                }
            }
            ui.add_space(12.0);
            if primary_button(ui, "Refresh Status", 120.0, 36.0).clicked() {
                self.apply_response(send_command(CommandRequest::Status));
            }
            ui.add_space(8.0);
            if danger_button(ui, "Recover / Clear Blocks", 180.0, 36.0).clicked() {
                self.apply_response(send_command(CommandRequest::Recover));
            }
            if let Some(err) = &self.error {
                ui.add_space(12.0);
                ui.label(RichText::new(err).color(ACCENT_RED));
            }
        });
    }

    fn apply_response(&mut self, response: coldcat_core::Result<CommandResponse>) {
        match response {
            Ok(resp) => {
                self.status = resp.status;
                self.status_fetched_at = Some(Instant::now());
                if resp.ok {
                    self.message = resp.message;
                    self.error = None;
                } else {
                    self.error = Some(resp.message);
                }
            }
            Err(err) => {
                self.error = Some(err.to_string());
            }
        }
    }

    fn scan_apps(&mut self) {
        self.app_candidates = scan_running_apps();
        self.last_app_scan = Some(Instant::now());
    }

    fn maybe_auto_refresh(&mut self) {
        if self.auto_refresh && self.last_auto_refresh.elapsed() > Duration::from_secs(15) {
            self.last_auto_refresh = Instant::now();
            self.apply_response(send_command(CommandRequest::Status));
        }
    }
}

"""

content = content[:app_start] + new_app_code + content[app_end:]

# 5. Remove unused helper functions (nav_button, dashboard, builder, profiles_page, etc)
# Since we replaced the entire impl block up to apply_theme, we already removed them!
# We just need to make sure we don't have dangling draw_nav_icon functions which are no longer needed, 
# but they won't cause compile errors if they are just unused or we can strip them.

content = re.sub(r'fn draw_nav_icon.*?^}$', '', content, flags=re.MULTILINE | re.DOTALL)
content = re.sub(r'fn draw_logo.*?^}$', '', content, flags=re.MULTILINE | re.DOTALL)

with open(filepath, "w", encoding="utf-8") as f:
    f.write(content)
