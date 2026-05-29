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
const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
const ACCENT_DARK: Color32 = Color32::from_rgb(29, 78, 216);
const BG: Color32 = Color32::from_rgb(246, 247, 249);
const PANEL: Color32 = Color32::from_rgb(255, 255, 255);
const CARD: Color32 = Color32::from_rgb(255, 255, 255);
const CARD_ALT: Color32 = Color32::from_rgb(239, 246, 255);
const BORDER: Color32 = Color32::from_rgb(229, 231, 235);
const MUTED: Color32 = Color32::from_rgb(100, 116, 139);
const ERROR: Color32 = Color32::from_rgb(180, 35, 24);
const ERROR_BG: Color32 = Color32::from_rgb(254, 243, 242);
const OK: Color32 = Color32::from_rgb(21, 128, 61);
const OK_BG: Color32 = Color32::from_rgb(236, 253, 245);

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
    Dashboard,
    Builder,
    Profiles,
    Apps,
    Sites,
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

#[derive(Debug)]
struct ColdCatApp {
    page: Page,
    duration: String,
    site_rules: Vec<String>,
    app_rules: Vec<String>,
    manual_site: String,
    manual_app: String,
    draft_name: String,
    profiles: Vec<BlockProfile>,
    selected_profile: Option<usize>,
    app_filter: String,
    app_candidates: Vec<AppCandidate>,
    last_app_scan: Option<Instant>,
    status: Option<ServiceStatus>,
    status_fetched_at: Option<Instant>,
    auto_refresh: bool,
    last_auto_refresh: Instant,
    message: String,
    error: Option<String>,
    last_action: String,
}

impl Default for ColdCatApp {
    fn default() -> Self {
        let mut app = Self {
            page: Page::Dashboard,
            duration: "2h".to_string(),
            site_rules: vec!["reddit.com".to_string(), "old.reddit.com".to_string()],
            app_rules: Vec::new(),
            manual_site: String::new(),
            manual_app: String::new(),
            draft_name: "New block".to_string(),
            profiles: load_profiles(),
            selected_profile: None,
            app_filter: String::new(),
            app_candidates: Vec::new(),
            last_app_scan: None,
            status: None,
            status_fetched_at: None,
            auto_refresh: false,
            last_auto_refresh: Instant::now(),
            message: "Ready. Install and start the daemon, then build a block.".to_string(),
            error: None,
            last_action: "Ready".to_string(),
        };
        app.scan_apps();
        app
    }
}

impl eframe::App for ColdCatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.maybe_auto_refresh();

        egui::SidePanel::left("coldcat_sidebar")
            .exact_width(280.0)
            .frame(
                egui::Frame::default()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::same(18.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.sidebar(ui));
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(BG)
                    .inner_margin(egui::Margin::same(18.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        self.header(ui);
                        ui.add_space(16.0);
                        match self.page {
                            Page::Dashboard => self.dashboard(ui),
                            Page::Builder => self.builder(ui),
                            Page::Profiles => self.profiles_page(ui),
                            Page::Apps => self.apps_page(ui),
                            Page::Sites => self.sites_page(ui),
                            Page::Settings => self.settings(ui),
                        }
                    });
            });
    }
}

impl ColdCatApp {
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
            draw_logo(ui.painter(), rect, ACCENT);
            ui.heading(RichText::new("Cold Cat").size(26.0).color(TEXT));
        });
        ui.label(RichText::new("Enforced focus").color(MUTED));
        ui.add_space(30.0);

        self.nav_button(ui, Page::Dashboard, NavIcon::Dashboard, "Dashboard");
        self.nav_button(ui, Page::Builder, NavIcon::Builder, "Block Builder");
        self.nav_button(ui, Page::Profiles, NavIcon::Profiles, "Profiles");
        self.nav_button(ui, Page::Apps, NavIcon::Apps, "Apps");
        self.nav_button(ui, Page::Sites, NavIcon::Sites, "Websites");
        self.nav_button(ui, Page::Settings, NavIcon::Settings, "Settings");

        ui.add_space(28.0);
        card(ui, |ui| {
            ui.label(RichText::new("Selected").strong());
            ui.label(format!("{} sites queued", self.site_rules.len()));
            ui.label(format!("{} apps queued", self.app_rules.len()));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Use Block Builder to review before starting.")
                    .small()
                    .color(MUTED),
            );
        });
    }

    fn nav_button(&mut self, ui: &mut egui::Ui, page: Page, icon: NavIcon, title: &str) {
        let selected = self.page == page;
        let width = ui.available_width().max(1.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, 48.0), egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

        if response.clicked() {
            self.page = page;
        }

        let fill = if selected {
            CARD_ALT
        } else if response.hovered() {
            Color32::from_rgb(248, 250, 252)
        } else {
            PANEL
        };

        ui.painter().rect_filled(rect, 9.0, fill);
        if selected {
            let bar = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top() + 9.0),
                egui::vec2(4.0, rect.height() - 18.0),
            );
            ui.painter().rect_filled(bar, 2.0, ACCENT);
        }
        let icon_rect = egui::Rect::from_center_size(
            rect.left_center() + egui::vec2(27.0, 0.0),
            egui::vec2(20.0, 20.0),
        );
        draw_nav_icon(
            ui.painter(),
            icon_rect,
            icon,
            if selected { ACCENT } else { MUTED },
        );
        ui.painter().text(
            rect.left_top() + egui::vec2(50.0, 14.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(16.0),
            if selected { ACCENT_DARK } else { TEXT },
        );
        ui.add_space(8.0);
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.vertical(|ui| {
                    ui.heading(RichText::new(page_title(self.page)).size(28.0));
                    ui.add(
                        egui::Label::new(RichText::new(page_subtitle(self.page)).color(MUTED))
                            .wrap(),
                    );
                });
                ui.separator();
                if secondary_button(ui, "Refresh daemon", 140.0, 34.0).clicked() {
                    self.refresh_status();
                }
                let state = self
                    .status
                    .as_ref()
                    .map(|status| state_label(&status.state))
                    .unwrap_or("Unknown");
                ui.label(
                    RichText::new(state)
                        .strong()
                        .color(status_color(self.status.as_ref())),
                );
            });
        });
    }

    fn dashboard(&mut self, ui: &mut egui::Ui) {
        if ui.available_width() >= 760.0 {
            ui.columns(2, |columns| {
                card(&mut columns[0], |ui| self.session_card(ui));
                card(&mut columns[1], |ui| self.quick_actions_card(ui));
            });
        } else {
            card(ui, |ui| self.session_card(ui));
            ui.add_space(12.0);
            card(ui, |ui| self.quick_actions_card(ui));
        }

        ui.add_space(12.0);
        if ui.available_width() >= 780.0 {
            ui.columns(3, |columns| {
                metric_card(
                    &mut columns[0],
                    "Websites queued",
                    self.site_rules.len(),
                    "Use categories or manual entries",
                );
                metric_card(
                    &mut columns[1],
                    "Apps queued",
                    self.app_rules.len(),
                    "Use the process picker",
                );
                metric_card(
                    &mut columns[2],
                    "Duration",
                    self.duration.as_str(),
                    "Applies to the next block",
                );
            });
        } else {
            metric_card(
                ui,
                "Websites queued",
                self.site_rules.len(),
                "Use categories or manual entries",
            );
            ui.add_space(12.0);
            metric_card(
                ui,
                "Apps queued",
                self.app_rules.len(),
                "Use the process picker",
            );
            ui.add_space(12.0);
            metric_card(
                ui,
                "Duration",
                self.duration.as_str(),
                "Applies to the next block",
            );
        }

        ui.add_space(12.0);
        card(ui, |ui| {
            ui.heading("Current draft");
            ui.add_space(8.0);
            if ui.available_width() >= 620.0 {
                ui.columns(2, |columns| {
                    rule_list(&mut columns[0], "Sites", &self.site_rules);
                    rule_list(&mut columns[1], "Apps", &self.app_rules);
                });
            } else {
                rule_list(ui, "Sites", &self.site_rules);
                ui.add_space(12.0);
                rule_list(ui, "Apps", &self.app_rules);
            }
        });
    }

    fn session_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("Session status");
        ui.label(
            RichText::new("No early stop is exposed during an active enforced block.").color(MUTED),
        );
        ui.add_space(12.0);

        match self.status.clone() {
            Some(status) => {
                let now = self.status_now(&status);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(state_label(&status.state))
                            .size(22.0)
                            .strong(),
                    );
                    if status.tampered_state {
                        alert_box(
                            ui,
                            "!",
                            "Tampered state",
                            "Sealed state validation failed. Administrator recovery is required.",
                            ERROR_BG,
                            ERROR,
                        );
                    }
                });

                if status.active_sessions.is_empty() {
                    ui.add_space(8.0);
                    ui.label("No active session.");
                } else {
                    ui.add_space(8.0);
                    for session in &status.active_sessions {
                        let total = session.end_time.saturating_sub(session.started_at).max(1);
                        let elapsed = now.saturating_sub(session.started_at).min(total);
                        let progress = elapsed as f32 / total as f32;
                        ui.label(RichText::new(&session.name).strong());
                        ui.add(
                            egui::ProgressBar::new(progress)
                                .desired_width(ui.available_width())
                                .text(remaining_label(now, session.end_time)),
                        );
                        ui.add_space(4.0);
                        ui.label(format!("Started: {}", session.started_at));
                        ui.label(format!("Ends: {}", session.end_time));
                        ui.label(format!("Blocked sites: {}", session.sites.len()));
                        ui.label(format!("Blocked apps: {}", session.apps.len()));
                        ui.add_space(10.0);
                    }
                }
            }
            None => {
                ui.label("Daemon status has not been loaded.");
                ui.add_space(8.0);
                if secondary_button(ui, "Check daemon now", ui.available_width(), 36.0).clicked() {
                    self.refresh_status();
                }
            }
        }
    }

    fn quick_actions_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("Quick actions");
        ui.label(RichText::new("Build, start, or recover a session.").color(MUTED));
        ui.add_space(12.0);

        if secondary_button(ui, "Open block builder", ui.available_width(), 38.0).clicked() {
            self.page = Page::Builder;
        }
        if secondary_button(ui, "Open profiles", ui.available_width(), 38.0).clicked() {
            self.page = Page::Profiles;
        }
        if secondary_button(ui, "Use active as draft", ui.available_width(), 38.0).clicked() {
            self.use_active_as_draft();
        }
        if primary_button(ui, "Start current draft", ui.available_width(), 40.0).clicked() {
            self.start_block();
        }
        if danger_button(ui, "Recover after expiry", ui.available_width(), 38.0).clicked() {
            self.recover();
        }

        ui.add_space(10.0);
        self.message_strip(ui);
    }

    fn builder(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| self.draft_tools_card(ui));
        ui.add_space(12.0);

        if ui.available_width() >= 760.0 {
            ui.columns(2, |columns| {
                card(&mut columns[0], |ui| {
                    self.duration_card(ui);
                });

                card(&mut columns[1], |ui| {
                    self.start_card(ui);
                });
            });
        } else {
            card(ui, |ui| self.duration_card(ui));
            ui.add_space(12.0);
            card(ui, |ui| self.start_card(ui));
        }

        ui.add_space(12.0);
        if ui.available_width() >= 760.0 {
            ui.columns(2, |columns| {
                card(&mut columns[0], |ui| self.site_editor(ui));
                card(&mut columns[1], |ui| self.app_editor(ui));
            });
        } else {
            card(ui, |ui| self.site_editor(ui));
            ui.add_space(12.0);
            card(ui, |ui| self.app_editor(ui));
        }
    }

    fn draft_tools_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("Draft");
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut self.draft_name).desired_width(220.0));
            if primary_button(ui, "Save profile", 122.0, 34.0).clicked() {
                self.save_current_profile();
            }
            if secondary_button(ui, "Clear draft", 104.0, 34.0).clicked() {
                self.clear_draft();
            }
            if secondary_button(ui, "Use active session", 146.0, 34.0).clicked() {
                self.use_active_as_draft();
            }
        });
    }

    fn duration_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("1. Duration");
        ui.add(
            egui::Label::new(
                RichText::new("Choose how long the block should be enforced.").color(MUTED),
            )
            .wrap(),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Duration");
            ui.text_edit_singleline(&mut self.duration);
        });
        ui.horizontal_wrapped(|ui| {
            for quick in ["25m", "30m", "1h", "2h", "4h", "8h"] {
                if secondary_button(ui, quick, 58.0, 30.0).clicked() {
                    self.duration = quick.to_string();
                }
            }
        });
    }

    fn start_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("2. Start");
        ui.add(
            egui::Label::new(
                RichText::new("The daemon will reject early clear/overwrite commands.")
                    .color(MUTED),
            )
            .wrap(),
        );
        ui.add_space(12.0);
        if primary_button(ui, "Start enforced block", ui.available_width(), 46.0).clicked() {
            self.start_block();
        }
        ui.add_space(8.0);
        self.message_strip(ui);
    }

    fn profiles_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Block profiles");
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Use built-in profiles or save your own reusable block lists and durations.",
                    )
                    .color(MUTED),
                )
                .wrap(),
            );
        });

        ui.add_space(12.0);
        if ui.available_width() >= 900.0 {
            ui.columns(2, |columns| {
                card(&mut columns[0], |ui| self.built_in_profiles_card(ui));
                card(&mut columns[1], |ui| self.saved_profiles_card(ui));
            });
        } else {
            card(ui, |ui| self.built_in_profiles_card(ui));
            ui.add_space(12.0);
            card(ui, |ui| self.saved_profiles_card(ui));
        }
    }

    fn built_in_profiles_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("Built-in presets");
        ui.add_space(8.0);
        for profile in built_in_profiles() {
            profile_row(ui, &profile, false, false, |action| match action {
                ProfileAction::Load => self.apply_profile(&profile),
                ProfileAction::Start => {
                    self.apply_profile(&profile);
                    self.start_block();
                }
                ProfileAction::Delete => {}
            });
        }
    }

    fn saved_profiles_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("Saved profiles");
        ui.horizontal_wrapped(|ui| {
            if primary_button(ui, "Save current draft", 156.0, 34.0).clicked() {
                self.save_current_profile();
            }
            if secondary_button(ui, "Reload", 86.0, 34.0).clicked() {
                self.profiles = load_profiles();
                self.message = "Profiles reloaded.".to_string();
                self.error = None;
                self.last_action = "Reloaded profiles".to_string();
            }
        });
        ui.add_space(8.0);

        if self.profiles.is_empty() {
            ui.label(RichText::new("No saved profiles yet.").color(MUTED));
            return;
        }

        let mut delete = None;
        for index in 0..self.profiles.len() {
            let profile = self.profiles[index].clone();
            let selected = self.selected_profile == Some(index);
            profile_row(ui, &profile, selected, true, |action| match action {
                ProfileAction::Load => {
                    self.selected_profile = Some(index);
                    self.apply_profile(&profile);
                }
                ProfileAction::Start => {
                    self.selected_profile = Some(index);
                    self.apply_profile(&profile);
                    self.start_block();
                }
                ProfileAction::Delete => {
                    delete = Some(index);
                }
            });
        }

        if let Some(index) = delete {
            let removed = self.profiles.remove(index);
            self.selected_profile = None;
            match save_profiles(&self.profiles) {
                Ok(()) => {
                    self.message = format!("Deleted profile '{}'.", removed.name);
                    self.error = None;
                    self.last_action = "Deleted profile".to_string();
                }
                Err(err) => {
                    self.error = Some(format!("Failed to save profiles: {err}"));
                    self.last_action = "Delete failed".to_string();
                }
            }
        }
    }

    fn apps_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("App picker");
                if secondary_button(ui, "Scan running apps", 150.0, 34.0).clicked() {
                    self.scan_apps();
                }
                if secondary_button(ui, "Scan installed apps", 150.0, 34.0).clicked() {
                    self.scan_installed_apps();
                }
                if let Some(scan) = self.last_app_scan {
                    ui.label(
                        RichText::new(format!("Scanned {}s ago", scan.elapsed().as_secs()))
                            .color(MUTED),
                    );
                }
            });
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Pick apps from currently running processes, scan installed apps, or add an executable name/path manually.",
                    )
                    .color(MUTED),
                )
                .wrap(),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Search");
                ui.add(
                    egui::TextEdit::singleline(&mut self.app_filter).desired_width(f32::INFINITY),
                );
            });
        });

        ui.add_space(12.0);
        if ui.available_width() >= 900.0 {
            ui.columns(2, |columns| {
                card(&mut columns[0], |ui| self.running_apps_card(ui));
                card(&mut columns[1], |ui| self.app_editor(ui));
            });
        } else {
            card(ui, |ui| self.running_apps_card(ui));
            ui.add_space(12.0);
            card(ui, |ui| self.app_editor(ui));
        }
    }

    fn running_apps_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("Discovered apps");
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .max_height(430.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for candidate in self.filtered_candidates() {
                    let already_selected = self
                        .app_rules
                        .iter()
                        .any(|rule| rule.eq_ignore_ascii_case(&candidate.rule));
                    app_candidate_row(ui, &candidate, already_selected, |rule| {
                        add_unique_rule(&mut self.app_rules, rule);
                    });
                }
            });
    }

    fn sites_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Website presets");
            ui.add(
                egui::Label::new(
                    RichText::new("Add whole categories, then remove anything you still need.")
                        .color(MUTED),
                )
                .wrap(),
            );
        });
        ui.add_space(12.0);

        if ui.available_width() >= 900.0 {
            for row in SITE_CATEGORIES.chunks(2) {
                ui.columns(2, |columns| {
                    for (index, category) in row.iter().enumerate() {
                        if category_card(&mut columns[index], category) {
                            for site in category.sites {
                                add_unique_rule(&mut self.site_rules, (*site).to_string());
                            }
                        }
                    }
                });
                ui.add_space(12.0);
            }
        } else {
            for category in SITE_CATEGORIES {
                if category_card(ui, category) {
                    for site in category.sites {
                        add_unique_rule(&mut self.site_rules, (*site).to_string());
                    }
                }
                ui.add_space(12.0);
            }
        }

        card(ui, |ui| self.site_editor(ui));
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        if ui.available_width() >= 760.0 {
            ui.columns(2, |columns| {
                card(&mut columns[0], |ui| self.daemon_settings_card(ui));
                card(&mut columns[1], security_model_card);
            });
        } else {
            card(ui, |ui| self.daemon_settings_card(ui));
            ui.add_space(12.0);
            card(ui, security_model_card);
        }
    }

    fn daemon_settings_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("Daemon");
        ui.add(
            egui::Label::new(
                RichText::new(
                    "The GUI is a front-end. Enforcement stays in the privileged daemon.",
                )
                .color(MUTED),
            )
            .wrap(),
        );
        ui.add_space(8.0);
        ui.checkbox(
            &mut self.auto_refresh,
            "Auto-refresh status every 15 seconds",
        );
        if secondary_button(ui, "Refresh status", ui.available_width(), 36.0).clicked() {
            self.refresh_status();
        }
        if danger_button(ui, "Recover after expiry", ui.available_width(), 36.0).clicked() {
            self.recover();
        }
        ui.add_space(8.0);
        self.message_strip(ui);
    }

    fn site_editor(&mut self, ui: &mut egui::Ui) {
        ui.heading("Selected websites");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.manual_site);
            if secondary_button(ui, "Add site", 96.0, 30.0).clicked() {
                self.add_manual_sites();
            }
        });
        ui.add_space(8.0);
        removable_rules(ui, &mut self.site_rules);
    }

    fn app_editor(&mut self, ui: &mut egui::Ui) {
        ui.heading("Selected apps");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.manual_app);
            if secondary_button(ui, "Add app", 96.0, 30.0).clicked() {
                self.add_manual_apps();
            }
        });
        ui.add_space(8.0);
        removable_rules(ui, &mut self.app_rules);
    }

    fn message_strip(&self, ui: &mut egui::Ui) {
        if let Some(error) = &self.error {
            alert_box(ui, "!", "Action failed", error, ERROR_BG, ERROR);
        } else {
            alert_box(ui, "i", "Status", &self.message, OK_BG, OK);
        }
        ui.label(
            RichText::new(format!("Last action: {}", self.last_action))
                .small()
                .color(MUTED),
        );
    }

    fn start_block(&mut self) {
        self.error = None;
        let duration_secs = match parse_duration_secs(&self.duration) {
            Ok(value) => value,
            Err(err) => {
                self.error = Some(err.to_string());
                self.last_action = "Start failed".to_string();
                return;
            }
        };

        let mut sites = self.site_rules.clone();
        let mut apps = self.app_rules.clone();
        for site in parse_rule_text(&self.manual_site) {
            add_unique_rule(&mut sites, site);
        }
        for app in parse_rule_text(&self.manual_app) {
            add_unique_rule(&mut apps, app);
        }

        if sites.is_empty() && apps.is_empty() {
            self.error = Some("Add at least one website or application to block.".to_string());
            self.last_action = "Start failed".to_string();
            return;
        }

        self.apply_response(
            send_command(CommandRequest::Start {
                name: self.block_name(),
                duration_secs,
                sites,
                apps,
            }),
            "Started block",
            "Start failed",
        );
    }

    fn refresh_status(&mut self) {
        self.error = None;
        self.apply_response(
            send_command(CommandRequest::Status),
            "Status refreshed",
            "Status failed",
        );
    }

    fn recover(&mut self) {
        self.error = None;
        self.apply_response(
            send_command(CommandRequest::Recover),
            "Recovery requested",
            "Recovery failed",
        );
    }

    fn apply_response(
        &mut self,
        response: coldcat_core::Result<CommandResponse>,
        ok_action: &str,
        error_action: &str,
    ) {
        match response {
            Ok(response) => {
                self.status = response.status;
                self.status_fetched_at = Some(Instant::now());
                if response.ok {
                    self.message = response.message;
                    self.error = None;
                    self.last_action = ok_action.to_string();
                } else {
                    self.error = Some(response.message);
                    self.last_action = error_action.to_string();
                }
            }
            Err(err) => {
                self.error = Some(err.to_string());
                self.last_action = error_action.to_string();
            }
        }
    }

    fn add_manual_sites(&mut self) {
        for site in parse_rule_text(&self.manual_site) {
            add_unique_rule(&mut self.site_rules, site);
        }
        self.manual_site.clear();
    }

    fn add_manual_apps(&mut self) {
        for app in parse_rule_text(&self.manual_app) {
            add_unique_rule(&mut self.app_rules, app);
        }
        self.manual_app.clear();
    }

    fn apply_profile(&mut self, profile: &BlockProfile) {
        self.draft_name = profile.name.clone();
        self.duration = profile.duration.clone();
        self.site_rules = profile.sites.clone();
        self.app_rules = profile.apps.clone();
        self.message = format!("Loaded profile '{}'.", profile.name);
        self.error = None;
        self.last_action = "Loaded profile".to_string();
        self.page = Page::Builder;
    }

    fn save_current_profile(&mut self) {
        let profile = self.profile_from_current_draft();
        if profile.sites.is_empty() && profile.apps.is_empty() {
            self.error =
                Some("Add at least one website or app before saving a profile.".to_string());
            self.last_action = "Save failed".to_string();
            return;
        }

        if parse_duration_secs(&profile.duration).is_err() {
            self.error = Some("Fix the duration before saving this profile.".to_string());
            self.last_action = "Save failed".to_string();
            return;
        }

        if let Some(existing) = self
            .profiles
            .iter_mut()
            .find(|existing| existing.name.eq_ignore_ascii_case(&profile.name))
        {
            *existing = profile.clone();
        } else {
            self.profiles.push(profile.clone());
        }
        self.profiles
            .sort_by_key(|profile| profile.name.to_ascii_lowercase());

        match save_profiles(&self.profiles) {
            Ok(()) => {
                self.message = format!("Saved profile '{}'.", profile.name);
                self.error = None;
                self.last_action = "Saved profile".to_string();
            }
            Err(err) => {
                self.error = Some(format!("Failed to save profiles: {err}"));
                self.last_action = "Save failed".to_string();
            }
        }
    }

    fn profile_from_current_draft(&self) -> BlockProfile {
        BlockProfile {
            name: self.block_name(),
            duration: self.duration.trim().to_string(),
            sites: self.site_rules.clone(),
            apps: self.app_rules.clone(),
        }
    }

    fn block_name(&self) -> String {
        let name = self.draft_name.trim();
        if name.is_empty() {
            "New block".to_string()
        } else {
            name.to_string()
        }
    }

    fn clear_draft(&mut self) {
        self.draft_name = "New block".to_string();
        self.duration = "2h".to_string();
        self.site_rules.clear();
        self.app_rules.clear();
        self.manual_site.clear();
        self.manual_app.clear();
        self.selected_profile = None;
        self.message = "Draft cleared.".to_string();
        self.error = None;
        self.last_action = "Cleared draft".to_string();
    }

    fn use_active_as_draft(&mut self) {
        let Some(status) = &self.status else {
            self.error =
                Some("Refresh daemon status first; no active session is loaded.".to_string());
            self.last_action = "Load active failed".to_string();
            return;
        };
        let Some(session) = status.active_sessions.first() else {
            self.error = Some("There is no active session to copy.".to_string());
            self.last_action = "Load active failed".to_string();
            return;
        };

        let now = self.status_now(status);
        self.draft_name = format!("Copy of {}", session.name);
        self.duration = remaining_duration_for_draft(now, session.end_time);
        self.site_rules = session.sites.clone();
        self.app_rules = session.apps.clone();
        self.selected_profile = None;
        self.message = "Copied active session into the draft.".to_string();
        self.error = None;
        self.last_action = "Copied active session".to_string();
        self.page = Page::Builder;
    }

    fn scan_apps(&mut self) {
        self.app_candidates = scan_running_apps();
        self.last_app_scan = Some(Instant::now());
    }

    fn scan_installed_apps(&mut self) {
        merge_candidates(&mut self.app_candidates, scan_installed_apps());
        self.last_app_scan = Some(Instant::now());
    }

    fn filtered_candidates(&self) -> Vec<AppCandidate> {
        let filter = self.app_filter.trim().to_ascii_lowercase();
        self.app_candidates
            .iter()
            .filter(|candidate| {
                filter.is_empty()
                    || candidate.name.to_ascii_lowercase().contains(&filter)
                    || candidate.rule.to_ascii_lowercase().contains(&filter)
                    || candidate.detail.to_ascii_lowercase().contains(&filter)
            })
            .take(250)
            .cloned()
            .collect()
    }

    fn status_now(&self, status: &ServiceStatus) -> u64 {
        status.now
            + self
                .status_fetched_at
                .map(|instant| instant.elapsed().as_secs())
                .unwrap_or(0)
    }

    fn maybe_auto_refresh(&mut self) {
        if !self.auto_refresh || self.last_auto_refresh.elapsed() < Duration::from_secs(15) {
            return;
        }
        self.last_auto_refresh = Instant::now();
        self.apply_response(
            send_command(CommandRequest::Status),
            "Auto-refreshed status",
            "Auto-refresh failed",
        );
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = CARD;
    style.visuals.widgets.active.bg_fill = ACCENT;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(239, 246, 255);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(248, 250, 252);
    style.visuals.widgets.noninteractive.bg_fill = CARD;
    style.visuals.selection.bg_fill = ACCENT;
    style.spacing.item_spacing = egui::vec2(12.0, 12.0);
    style.spacing.button_padding = egui::vec2(14.0, 9.0);
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(15.5));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(24.0));
    ctx.set_style(style);
}

fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::default()
        .fill(CARD)
        .stroke(Stroke::new(1.0, Color32::from_rgb(238, 242, 247)))
        .shadow(Shadow {
            offset: egui::vec2(0.0, 2.0),
            blur: 10.0,
            spread: 0.0,
            color: Color32::from_rgba_premultiplied(15, 23, 42, 18),
        })
        .rounding(12.0)
        .inner_margin(egui::Margin::same(22.0))
        .show(ui, add_contents)
        .inner
}

fn draw_logo(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.36;
    painter.circle_filled(center, radius, color);
    let ear_y = center.y - radius * 0.65;
    painter.line_segment(
        [
            egui::pos2(center.x - radius * 0.65, ear_y),
            egui::pos2(center.x - radius * 0.25, center.y - radius * 1.25),
        ],
        Stroke::new(2.0, color),
    );
    painter.line_segment(
        [
            egui::pos2(center.x + radius * 0.65, ear_y),
            egui::pos2(center.x + radius * 0.25, center.y - radius * 1.25),
        ],
        Stroke::new(2.0, color),
    );
}

fn draw_nav_icon(painter: &egui::Painter, rect: egui::Rect, icon: NavIcon, color: Color32) {
    let stroke = Stroke::new(1.8, color);
    let c = rect.center();
    let w = rect.width();
    let h = rect.height();
    let left = rect.left() + w * 0.18;
    let right = rect.right() - w * 0.18;
    let top = rect.top() + h * 0.18;
    let bottom = rect.bottom() - h * 0.18;

    match icon {
        NavIcon::Dashboard => {
            painter.line_segment([egui::pos2(left, c.y), egui::pos2(c.x, top)], stroke);
            painter.line_segment([egui::pos2(c.x, top), egui::pos2(right, c.y)], stroke);
            painter.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(left + 2.0, c.y),
                    egui::pos2(right - 2.0, bottom),
                ),
                2.0,
                stroke,
            );
        }
        NavIcon::Builder => {
            painter.rect_stroke(rect.shrink(3.0), 3.0, stroke);
            painter.line_segment(
                [
                    egui::pos2(left + 2.0, c.y),
                    egui::pos2(c.x - 1.0, bottom - 2.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x - 1.0, bottom - 2.0),
                    egui::pos2(right - 1.0, top + 3.0),
                ],
                stroke,
            );
        }
        NavIcon::Profiles => {
            painter.rect_stroke(
                egui::Rect::from_min_max(egui::pos2(left + 2.0, top), egui::pos2(right, bottom)),
                2.0,
                stroke,
            );
            painter.line_segment(
                [egui::pos2(left, top + 3.0), egui::pos2(left, bottom + 2.0)],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(left, bottom + 2.0),
                    egui::pos2(right - 3.0, bottom + 2.0),
                ],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(left + 6.0, c.y), egui::pos2(right - 4.0, c.y)],
                stroke,
            );
        }
        NavIcon::Apps => {
            painter.rect_stroke(rect.shrink(4.0), 4.0, stroke);
            painter.line_segment(
                [egui::pos2(c.x, top - 1.0), egui::pos2(c.x, top - 5.0)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x, bottom + 1.0), egui::pos2(c.x, bottom + 5.0)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(left - 5.0, c.y), egui::pos2(left - 1.0, c.y)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(right + 1.0, c.y), egui::pos2(right + 5.0, c.y)],
                stroke,
            );
            painter.circle_filled(c, 2.4, color);
        }
        NavIcon::Sites => {
            painter.circle_stroke(c, w.min(h) * 0.34, stroke);
            painter.line_segment([egui::pos2(left, c.y), egui::pos2(right, c.y)], stroke);
            painter.line_segment([egui::pos2(c.x, top), egui::pos2(c.x, bottom)], stroke);
            painter.circle_stroke(c, w.min(h) * 0.18, stroke);
        }
        NavIcon::Settings => {
            painter.circle_stroke(c, w.min(h) * 0.30, stroke);
            painter.circle_stroke(c, w.min(h) * 0.11, stroke);
            for angle in [
                0.0_f32,
                std::f32::consts::FRAC_PI_2,
                std::f32::consts::PI,
                std::f32::consts::PI + std::f32::consts::FRAC_PI_2,
            ] {
                let outer = egui::pos2(c.x + angle.cos() * w * 0.43, c.y + angle.sin() * h * 0.43);
                let inner = egui::pos2(c.x + angle.cos() * w * 0.32, c.y + angle.sin() * h * 0.32);
                painter.line_segment([inner, outer], stroke);
            }
        }
    }
}

fn primary_button(ui: &mut egui::Ui, label: &str, width: f32, height: f32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
            .fill(ACCENT)
            .stroke(Stroke::new(1.0, ACCENT_DARK))
            .rounding(8.0)
            .min_size(egui::vec2(width, height)),
    )
}

fn secondary_button(ui: &mut egui::Ui, label: &str, width: f32, height: f32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).strong().color(TEXT))
            .fill(Color32::from_rgb(248, 250, 252))
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(8.0)
            .min_size(egui::vec2(width, height)),
    )
}

fn danger_button(ui: &mut egui::Ui, label: &str, width: f32, height: f32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
            .fill(ERROR)
            .stroke(Stroke::new(1.0, Color32::from_rgb(145, 32, 24)))
            .rounding(8.0)
            .min_size(egui::vec2(width, height)),
    )
}

fn alert_box(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    body: &str,
    fill: Color32,
    color: Color32,
) {
    egui::Frame::default()
        .fill(fill)
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.35)))
        .rounding(10.0)
        .inner_margin(egui::Margin::symmetric(14.0, 11.0))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(icon).strong().color(color));
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).strong().color(color));
                    ui.add(egui::Label::new(RichText::new(body).color(TEXT)).wrap());
                });
            });
        });
}

fn metric_card<T: std::fmt::Display>(ui: &mut egui::Ui, title: &str, value: T, subtitle: &str) {
    card(ui, |ui| {
        ui.label(RichText::new(title).color(MUTED));
        ui.heading(RichText::new(value.to_string()).size(30.0).color(ACCENT));
        ui.label(RichText::new(subtitle).small().color(MUTED));
    });
}

fn rule_list(ui: &mut egui::Ui, title: &str, rules: &[String]) {
    ui.heading(title);
    if rules.is_empty() {
        ui.label(RichText::new("None selected").color(MUTED));
    } else {
        for rule in rules.iter().take(10) {
            ui.label(format!("- {rule}"));
        }
        if rules.len() > 10 {
            ui.label(RichText::new(format!("and {} more", rules.len() - 10)).color(MUTED));
        }
    }
}

fn removable_rules(ui: &mut egui::Ui, rules: &mut Vec<String>) {
    if rules.is_empty() {
        ui.label(RichText::new("Nothing selected yet.").color(MUTED));
        return;
    }

    let mut remove = None;
    egui::ScrollArea::vertical()
        .max_height(260.0)
        .show(ui, |ui| {
            for (index, rule) in rules.iter().enumerate() {
                egui::Frame::default()
                    .fill(CARD_ALT)
                    .rounding(10.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 7.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(rule);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("Remove").color(ERROR).strong(),
                                            )
                                            .fill(Color32::from_rgb(255, 251, 250))
                                            .stroke(Stroke::new(
                                                1.0,
                                                Color32::from_rgb(253, 162, 155),
                                            ))
                                            .rounding(7.0)
                                            .min_size(egui::vec2(74.0, 26.0)),
                                        )
                                        .clicked()
                                    {
                                        remove = Some(index);
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(4.0);
            }
        });

    if let Some(index) = remove {
        rules.remove(index);
    }
}

fn app_candidate_row(
    ui: &mut egui::Ui,
    candidate: &AppCandidate,
    already_selected: bool,
    mut add: impl FnMut(String),
) {
    egui::Frame::default()
        .fill(if already_selected { PANEL } else { CARD_ALT })
        .rounding(10.0)
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&candidate.name).strong());
                    ui.add(
                        egui::Label::new(RichText::new(&candidate.detail).small().color(MUTED))
                            .wrap(),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if already_selected {
                        ui.label(RichText::new("Added").color(OK));
                    } else if secondary_button(ui, "Add", 62.0, 30.0).clicked() {
                        add(candidate.rule.clone());
                    }
                });
            });
        });
    ui.add_space(6.0);
}

fn category_card(ui: &mut egui::Ui, category: &SiteCategory) -> bool {
    let mut clicked = false;
    egui::Frame::default()
        .fill(CARD)
        .stroke(Stroke::new(1.0, Color32::from_rgb(238, 242, 247)))
        .rounding(12.0)
        .inner_margin(egui::Margin::same(20.0))
        .show(ui, |ui| {
            ui.heading(category.name);
            ui.add(egui::Label::new(RichText::new(category.subtitle).color(MUTED)).wrap());
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    RichText::new(category.sites.join(", "))
                        .small()
                        .color(MUTED),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            if secondary_button(ui, "Add category", 128.0, 32.0).clicked() {
                clicked = true;
            }
        });
    clicked
}

fn security_model_card(ui: &mut egui::Ui) {
    ui.heading("Security model");
    ui.add(egui::Label::new("Active sessions cannot be cleared by this GUI before expiry.").wrap());
    ui.add(
        egui::Label::new("State is signed by the daemon and stored in privileged locations.")
            .wrap(),
    );
    ui.add(
        egui::Label::new(
            "Administrator/root recovery is intentionally documented for owner control.",
        )
        .wrap(),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileAction {
    Load,
    Start,
    Delete,
}

fn profile_row(
    ui: &mut egui::Ui,
    profile: &BlockProfile,
    selected: bool,
    can_delete: bool,
    mut action: impl FnMut(ProfileAction),
) {
    egui::Frame::default()
        .fill(if selected {
            CARD_ALT
        } else {
            Color32::from_rgb(248, 250, 252)
        })
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&profile.name).strong().color(TEXT));
                    ui.label(
                        RichText::new(format!(
                            "{} • {} sites • {} apps",
                            profile.duration,
                            profile.sites.len(),
                            profile.apps.len()
                        ))
                        .small()
                        .color(MUTED),
                    );
                });
                if secondary_button(ui, "Load", 70.0, 30.0).clicked() {
                    action(ProfileAction::Load);
                }
                if primary_button(ui, "Start", 70.0, 30.0).clicked() {
                    action(ProfileAction::Start);
                }
                if can_delete && danger_button(ui, "Delete", 78.0, 30.0).clicked() {
                    action(ProfileAction::Delete);
                }
            });
            if !profile.sites.is_empty() || !profile.apps.is_empty() {
                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(RichText::new(profile_preview(profile)).small().color(MUTED))
                        .wrap(),
                );
            }
        });
    ui.add_space(8.0);
}

fn profile_preview(profile: &BlockProfile) -> String {
    let mut parts = Vec::new();
    if !profile.sites.is_empty() {
        parts.push(format!(
            "Sites: {}",
            profile
                .sites
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !profile.apps.is_empty() {
        parts.push(format!(
            "Apps: {}",
            profile
                .apps
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    parts.join(" | ")
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

fn page_title(page: Page) -> &'static str {
    match page {
        Page::Dashboard => "Dashboard",
        Page::Builder => "Block Builder",
        Page::Profiles => "Profiles",
        Page::Apps => "Apps",
        Page::Sites => "Websites",
        Page::Settings => "Settings",
    }
}

fn page_subtitle(page: Page) -> &'static str {
    match page {
        Page::Dashboard => "See the active block and your next draft at a glance.",
        Page::Builder => "Set duration, review selected rules, and start an enforced session.",
        Page::Profiles => "Load built-in presets or save reusable block profiles.",
        Page::Apps => "Select running apps instead of typing process names by hand.",
        Page::Sites => "Use presets or build a custom website block list.",
        Page::Settings => "Daemon status, recovery, and security notes.",
    }
}

fn state_label(state: &ServiceState) -> &'static str {
    match state {
        ServiceState::Idle => "Idle",
        ServiceState::Active => "Active",
        ServiceState::ExpiredNeedsRecovery => "Expired - recovery available",
        ServiceState::Tampered => "Tampered state",
    }
}

fn status_color(status: Option<&ServiceStatus>) -> Color32 {
    match status.map(|status| &status.state) {
        Some(ServiceState::Active) => ACCENT,
        Some(ServiceState::Idle) => OK,
        Some(ServiceState::ExpiredNeedsRecovery) => Color32::YELLOW,
        Some(ServiceState::Tampered) => ERROR,
        None => MUTED,
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

fn remaining_duration_for_draft(now: u64, end_time: u64) -> String {
    let remaining = end_time.saturating_sub(now).max(60);
    if remaining.is_multiple_of(3600) {
        format!("{}h", remaining / 3600)
    } else if remaining.is_multiple_of(60) {
        format!("{}m", remaining / 60)
    } else {
        format!("{remaining}s")
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
