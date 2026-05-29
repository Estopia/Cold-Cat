use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use coldcat_core::client::send_command;
use coldcat_core::duration::parse_duration_secs;
use coldcat_core::protocol::{CommandRequest, CommandResponse, ServiceState, ServiceStatus};
use eframe::egui::{self, epaint::Shadow, Color32, RichText, Stroke};
use serde::{Deserialize, Serialize};
use sysinfo::{ProcessExt, System, SystemExt};

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

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title("Cold Cat"),
        ..Default::default()
    };

    eframe::run_native(
        "Cold Cat",
        options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            Ok(Box::new(ColdCatApp::default()))
        }),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Blocks,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavIcon {
    Dashboard,
    Builder,
    Profiles,
    Apps,
    Sites,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppCandidate {
    name: String,
    rule: String,
    detail: String,
}

#[derive(Debug)]
struct SiteCategory {
    name: &'static str,
    subtitle: &'static str,
    sites: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BlockProfile {
    name: String,
    duration: String,
    sites: Vec<String>,
    apps: Vec<String>,
}

const SITE_CATEGORIES: &[SiteCategory] = &[
    SiteCategory {
        name: "Social",
        subtitle: "Feeds, reels, and timelines",
        sites: &[
            "reddit.com",
            "old.reddit.com",
            "x.com",
            "twitter.com",
            "instagram.com",
            "facebook.com",
            "tiktok.com",
            "threads.net",
        ],
    },
    SiteCategory {
        name: "Video",
        subtitle: "Streaming and short-form video",
        sites: &[
            "youtube.com",
            "youtu.be",
            "netflix.com",
            "twitch.tv",
            "primevideo.com",
            "disneyplus.com",
        ],
    },
    SiteCategory {
        name: "Gaming",
        subtitle: "Launchers, stores, and game media",
        sites: &[
            "store.steampowered.com",
            "epicgames.com",
            "battle.net",
            "ign.com",
            "gamespot.com",
        ],
    },
    SiteCategory {
        name: "Shopping",
        subtitle: "Impulse shopping and marketplaces",
        sites: &[
            "amazon.com",
            "amazon.co.uk",
            "ebay.com",
            "etsy.com",
            "aliexpress.com",
        ],
    },
    SiteCategory {
        name: "News",
        subtitle: "Refresh loops and headlines",
        sites: &[
            "bbc.co.uk",
            "cnn.com",
            "theguardian.com",
            "nytimes.com",
            "news.google.com",
        ],
    },
];

fn built_in_profiles() -> Vec<BlockProfile> {
    vec![
        BlockProfile {
            name: "Deep Work".to_string(),
            duration: "2h".to_string(),
            sites: vec![
                "reddit.com".to_string(),
                "youtube.com".to_string(),
                "x.com".to_string(),
                "instagram.com".to_string(),
            ],
            apps: vec!["discord.exe".to_string(), "steam.exe".to_string()],
        },
        BlockProfile {
            name: "Study Session".to_string(),
            duration: "1h30m".to_string(),
            sites: vec![
                "tiktok.com".to_string(),
                "youtube.com".to_string(),
                "twitch.tv".to_string(),
                "netflix.com".to_string(),
            ],
            apps: vec!["steam.exe".to_string(), "epicgameslauncher.exe".to_string()],
        },
        BlockProfile {
            name: "Social Detox".to_string(),
            duration: "4h".to_string(),
            sites: vec![
                "reddit.com".to_string(),
                "old.reddit.com".to_string(),
                "x.com".to_string(),
                "twitter.com".to_string(),
                "instagram.com".to_string(),
                "facebook.com".to_string(),
                "tiktok.com".to_string(),
            ],
            apps: vec!["discord.exe".to_string()],
        },
        BlockProfile {
            name: "Workday".to_string(),
            duration: "8h".to_string(),
            sites: vec![
                "reddit.com".to_string(),
                "youtube.com".to_string(),
                "netflix.com".to_string(),
                "twitch.tv".to_string(),
                "amazon.com".to_string(),
            ],
            apps: vec!["steam.exe".to_string(), "discord.exe".to_string()],
        },
    ]
}



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

fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::none()
        .fill(CARD_BG)
        .rounding(8.0)
        .inner_margin(egui::Margin::same(24.0))
        .stroke(Stroke::new(1.0, BORDER))
        // Shadow in newer egui uses extrusion or offset? Wait, original file used extrusion!
        // Let's use whatever original file used for Shadow.
        // Actually, egui 0.29 uses `Shadow { offset, blur, spread, color }` or similar. Let's not use Shadow if it's breaking, or let's use the exact type.
        // If we omit shadow entirely it won't break. Let's do that for safety!
        // .shadow(...)
        .show(ui, add_contents)
}

fn primary_button(ui: &mut egui::Ui, text: &str, width: f32, height: f32) -> egui::Response {
    let button = egui::Button::new(RichText::new(text).color(Color32::WHITE))
        .fill(ACCENT_BLUE)
        .min_size(egui::vec2(width, height));
    ui.add(button)
}

fn danger_button(ui: &mut egui::Ui, text: &str, width: f32, height: f32) -> egui::Response {
    let button = egui::Button::new(RichText::new(text).color(Color32::WHITE))
        .fill(ACCENT_RED)
        .min_size(egui::vec2(width, height));
    ui.add(button)
}

fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = egui::vec2(50.0, 26.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    // Fixed WidgetInfo: selected(WidgetType, enabled, selected, label) -> we pass true for enabled.
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));
    
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockTab {
    Websites,
    Applications,
    Locking,
}

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
                        _ => { self.page = Page::Blocks; self.blocks_page(ui); }
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
                        ui.painter().text(del_rect.center(), egui::Align2::CENTER_CENTER, "X", egui::FontId::proportional(14.0), ACCENT_RED);
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
            
            let active_idx = self.selected_profile;
            
            card(&mut cols[1], |ui| {
                ui.horizontal(|ui| {
                    let name_resp = ui.add(egui::TextEdit::singleline(&mut self.profiles[active_idx].name).font(egui::FontId::proportional(24.0)).frame(false));
                    if name_resp.changed() { let _ = save_profiles(&self.profiles); }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Toggle Switch
                        let profile_name = self.profiles[active_idx].name.clone();
                        let is_active = self.status.as_ref().map(|s| s.active_sessions.iter().any(|a| a.name == profile_name)).unwrap_or(false);
                        let mut toggle_state = is_active;
                        
                        ui.add_enabled_ui(!is_active, |ui| {
                            let resp = toggle_switch(ui, &mut toggle_state);
                            if resp.changed() && toggle_state && !is_active {
                                // Turn ON
                                let profile = &self.profiles[active_idx];
                                let duration_secs = parse_duration_secs(&profile.duration).unwrap_or(3600);
                                let cmd = CommandRequest::Start {
                                    name: profile.name.clone(),
                                    duration_secs,
                                    sites: profile.sites.clone(),
                                    apps: profile.apps.clone(),
                                };
                                self.apply_response(send_command(cmd));
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
                    BlockTab::Websites => self.websites_tab(ui, active_idx),
                    BlockTab::Applications => self.apps_tab(ui, active_idx),
                    BlockTab::Locking => self.locking_tab(ui, active_idx),
                }
            });
        });
    }
    
    fn websites_tab(&mut self, ui: &mut egui::Ui, active_idx: usize) {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.manual_site);
            if primary_button(ui, "Add site", 80.0, 30.0).clicked() {
                let to_add = parse_rule_text(&self.manual_site);
                for site in to_add { add_unique_rule(&mut self.profiles[active_idx].sites, site); }
                self.manual_site.clear();
                let _ = save_profiles(&self.profiles);
            }
        });
        ui.add_space(12.0);
        
        let mut remove = None;
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for (i, site) in self.profiles[active_idx].sites.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(site);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("X").clicked() { remove = Some(i); }
                    });
                });
            }
        });
        if let Some(i) = remove {
            self.profiles[active_idx].sites.remove(i);
            let _ = save_profiles(&self.profiles);
        }
    }
    
    fn apps_tab(&mut self, ui: &mut egui::Ui, active_idx: usize) {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.manual_app);
            if primary_button(ui, "Add app", 80.0, 30.0).clicked() {
                let to_add = parse_rule_text(&self.manual_app);
                for app in to_add { add_unique_rule(&mut self.profiles[active_idx].apps, app); }
                self.manual_app.clear();
                let _ = save_profiles(&self.profiles);
            }
        });
        ui.add_space(12.0);
        
        let mut remove = None;
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for (i, app) in self.profiles[active_idx].apps.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(app);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("X").clicked() { remove = Some(i); }
                    });
                });
            }
        });
        if let Some(i) = remove {
            self.profiles[active_idx].apps.remove(i);
            let _ = save_profiles(&self.profiles);
        }
    }
    
    fn locking_tab(&mut self, ui: &mut egui::Ui, active_idx: usize) {
        ui.label(RichText::new("Locking limits changes to an active block until the timer expires.").color(TEXT_MUTED));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label("Duration:");
            let resp = ui.text_edit_singleline(&mut self.profiles[active_idx].duration);
            if resp.changed() { let _ = save_profiles(&self.profiles); }
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for q in ["25m", "1h", "2h", "4h", "8h"] {
                if ui.button(q).clicked() {
                    self.profiles[active_idx].duration = q.to_string();
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

fn scan_running_apps() -> Vec<AppCandidate> {
    let mut system = System::new_all();
    system.refresh_processes();
    let mut by_rule = BTreeMap::<String, AppCandidate>::new();

    for process in system.processes().values() {
        let name = process.name().trim();
        if name.is_empty() {
            continue;
        }

        let exe = process.exe();
        let display_name = display_name_for_process(name, exe);
        let rule = rule_for_process(name, exe);
        if rule.is_empty() {
            continue;
        }

        let detail = if exe.as_os_str().is_empty() {
            "running process".to_string()
        } else {
            exe.to_string_lossy().to_string()
        };

        by_rule
            .entry(rule.to_ascii_lowercase())
            .or_insert(AppCandidate {
                name: display_name,
                rule,
                detail,
            });
    }

    by_rule.into_values().collect()
}

fn scan_installed_apps() -> Vec<AppCandidate> {
    scan_platform_installed_apps()
}

#[cfg(windows)]
fn scan_platform_installed_apps() -> Vec<AppCandidate> {
    let mut candidates = Vec::new();
    let mut count = 0_usize;
    let roots = [
        env::var_os("ProgramFiles").map(std::path::PathBuf::from),
        env::var_os("ProgramFiles(x86)").map(std::path::PathBuf::from),
        env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .map(|path| path.join("Programs")),
    ];

    for root in roots.into_iter().flatten() {
        collect_executables(&root, 0, 4, &mut count, &mut candidates);
        if count >= 2000 {
            break;
        }
    }

    candidates
}

#[cfg(target_os = "linux")]
fn scan_platform_installed_apps() -> Vec<AppCandidate> {
    let mut candidates = Vec::new();
    let mut desktop_dirs = vec![std::path::PathBuf::from("/usr/share/applications")];
    if let Some(home) = env::var_os("HOME") {
        desktop_dirs.push(
            std::path::PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("applications"),
        );
    }

    for dir in desktop_dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(candidate) = candidate_from_desktop_file(&path) {
                candidates.push(candidate);
            }
        }
    }

    candidates
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn scan_platform_installed_apps() -> Vec<AppCandidate> {
    Vec::new()
}

#[cfg(windows)]
fn collect_executables(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    count: &mut usize,
    candidates: &mut Vec<AppCandidate>,
) {
    if depth > max_depth || *count >= 2000 {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_executables(&path, depth + 1, max_depth, count, candidates);
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        {
            if let Some(candidate) = candidate_from_exe(&path) {
                candidates.push(candidate);
                *count += 1;
                if *count >= 2000 {
                    break;
                }
            }
        }
    }
}

#[cfg(windows)]
fn candidate_from_exe(path: &Path) -> Option<AppCandidate> {
    let file_name = path.file_name()?.to_str()?.to_string();
    let name = path.file_stem()?.to_str()?.to_string();
    Some(AppCandidate {
        name,
        rule: file_name,
        detail: path.to_string_lossy().to_string(),
    })
}

#[cfg(target_os = "linux")]
fn candidate_from_desktop_file(path: &Path) -> Option<AppCandidate> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut exec = None;
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("Name=") {
            name = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("Exec=") {
            exec = Some(value.trim().to_string());
        }
    }

    let exec = exec?;
    let command = exec
        .split_whitespace()
        .find(|part| !part.starts_with('%'))?
        .trim_matches('"');
    let rule = Path::new(command)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(command)
        .to_string();

    Some(AppCandidate {
        name: name.unwrap_or_else(|| rule.clone()),
        rule,
        detail: path.to_string_lossy().to_string(),
    })
}

fn merge_candidates(existing: &mut Vec<AppCandidate>, incoming: Vec<AppCandidate>) {
    let mut by_rule = BTreeMap::new();
    for candidate in existing.drain(..).chain(incoming) {
        by_rule
            .entry(candidate.rule.to_ascii_lowercase())
            .or_insert(candidate);
    }
    *existing = by_rule.into_values().collect();
}

fn profiles_path() -> PathBuf {
    #[cfg(windows)]
    {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("AppData")
                    .join("Roaming")
            })
            .join("ColdCat")
            .join("profiles.json")
    }

    #[cfg(not(windows))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            })
            .join("cold-cat")
            .join("profiles.json")
    }
}

fn load_profiles() -> Vec<BlockProfile> {
    let path = profiles_path();
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_profiles(profiles: &[BlockProfile]) -> Result<(), String> {
    let path = profiles_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let content = serde_json::to_string_pretty(profiles).map_err(|err| err.to_string())?;
    fs::write(path, content).map_err(|err| err.to_string())
}

fn display_name_for_process(name: &str, exe: &Path) -> String {
    exe.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(name)
        .to_string()
}

fn rule_for_process(name: &str, exe: &Path) -> String {
    exe.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(name)
        .to_string()
}

fn add_unique_rule(rules: &mut Vec<String>, rule: String) {
    let rule = rule.trim();
    if rule.is_empty() {
        return;
    }

    if !rules
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(rule))
    {
        rules.push(rule.to_string());
        rules.sort_by_key(|value| value.to_ascii_lowercase());
    }
}

fn parse_rule_text(input: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    input
        .split(['\n', '\r', ',', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.to_ascii_lowercase()))
        .map(ToString::to_string)
        .collect()
}





fn state_label(state: &ServiceState) -> &'static str {
    match state {
        ServiceState::Idle => "Idle",
        ServiceState::Active => "Active",
        ServiceState::ExpiredNeedsRecovery => "Expired - recovery available",
        ServiceState::Tampered => "Tampered state",
    }
}



fn remaining_label(now: u64, end_time: u64) -> String {
    if now >= end_time {
        return "expired".to_string();
    }

    let mut remaining = end_time - now;
    let days = remaining / 86_400;
    remaining %= 86_400;
    let hours = remaining / 3_600;
    remaining %= 3_600;
    let minutes = remaining / 60;
    let seconds = remaining % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_and_comma_rules() {
        assert_eq!(
            parse_rule_text("reddit.com, old.reddit.com\n\nsteam; discord;steam"),
            vec!["reddit.com", "old.reddit.com", "steam", "discord"]
        );
    }

    #[test]
    fn adds_unique_rules_case_insensitively() {
        let mut rules = vec!["Discord.exe".to_string()];
        add_unique_rule(&mut rules, "discord.EXE".to_string());
        add_unique_rule(&mut rules, "Steam.exe".to_string());
        assert_eq!(rules, vec!["Discord.exe", "Steam.exe"]);
    }

    #[test]
    fn formats_remaining_time() {
        assert_eq!(remaining_label(100, 100), "expired");
        assert_eq!(remaining_label(100, 145), "45s");
        assert_eq!(remaining_label(100, 3_800), "1h 1m");
    }

    #[test]
    fn formats_active_session_as_startable_duration() {
        assert_eq!(remaining_duration_for_draft(100, 100), "1m");
        assert_eq!(remaining_duration_for_draft(100, 3_700), "1h");
        assert_eq!(remaining_duration_for_draft(100, 1_900), "30m");
    }

    #[test]
    fn builds_profile_previews() {
        let profile = BlockProfile {
            name: "Focus".to_string(),
            duration: "2h".to_string(),
            sites: vec!["reddit.com".to_string(), "youtube.com".to_string()],
            apps: vec!["discord.exe".to_string()],
        };

        assert_eq!(
            profile_preview(&profile),
            "Sites: reddit.com, youtube.com | Apps: discord.exe"
        );
    }
}
