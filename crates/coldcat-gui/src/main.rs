use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

use coldcat_core::client::send_command;
use coldcat_core::duration::parse_duration_secs;
use coldcat_core::protocol::{CommandRequest, CommandResponse, ServiceState, ServiceStatus};
use eframe::egui::{self, Color32, RichText, Stroke};
use sysinfo::{ProcessExt, System, SystemExt};

const ACCENT: Color32 = Color32::from_rgb(87, 166, 255);
const ACCENT_2: Color32 = Color32::from_rgb(94, 234, 212);
const BG: Color32 = Color32::from_rgb(10, 14, 24);
const PANEL: Color32 = Color32::from_rgb(17, 24, 39);
const CARD: Color32 = Color32::from_rgb(24, 34, 52);
const CARD_ALT: Color32 = Color32::from_rgb(31, 41, 61);
const BORDER: Color32 = Color32::from_rgb(55, 68, 92);
const MUTED: Color32 = Color32::from_rgb(156, 171, 194);
const ERROR: Color32 = Color32::from_rgb(255, 120, 120);
const OK: Color32 = Color32::from_rgb(132, 255, 180);

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

#[derive(Debug)]
struct ColdCatApp {
    page: Page,
    duration: String,
    site_rules: Vec<String>,
    app_rules: Vec<String>,
    manual_site: String,
    manual_app: String,
    app_filter: String,
    app_candidates: Vec<AppCandidate>,
    last_app_scan: Option<Instant>,
    status: Option<ServiceStatus>,
    status_fetched_at: Option<Instant>,
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
            app_filter: String::new(),
            app_candidates: Vec::new(),
            last_app_scan: None,
            status: None,
            status_fetched_at: None,
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
        ui.vertical_centered(|ui| {
            ui.heading(RichText::new("Cold Cat").size(30.0).color(ACCENT_2));
            ui.label(RichText::new("Enforced focus").color(MUTED));
        });
        ui.add_space(28.0);

        self.nav_button(ui, Page::Dashboard, "Dashboard", "Current session");
        self.nav_button(ui, Page::Builder, "Block Builder", "Start a session");
        self.nav_button(ui, Page::Apps, "Apps", "Pick processes");
        self.nav_button(ui, Page::Sites, "Websites", "Categories and lists");
        self.nav_button(ui, Page::Settings, "Settings", "Daemon and recovery");

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

    fn nav_button(&mut self, ui: &mut egui::Ui, page: Page, title: &str, subtitle: &str) {
        let selected = self.page == page;
        let width = ui.available_width().max(1.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, 62.0), egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

        if response.clicked() {
            self.page = page;
        }

        let fill = if selected {
            CARD_ALT
        } else if response.hovered() {
            Color32::from_rgb(28, 39, 59)
        } else {
            PANEL
        };
        let stroke = if selected {
            Stroke::new(1.5, ACCENT)
        } else {
            Stroke::new(1.0, Color32::TRANSPARENT)
        };

        ui.painter().rect(rect, 12.0, fill, stroke);
        ui.painter().text(
            rect.left_top() + egui::vec2(14.0, 12.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(17.0),
            if selected { ACCENT } else { Color32::WHITE },
        );
        ui.painter().text(
            rect.left_top() + egui::vec2(14.0, 36.0),
            egui::Align2::LEFT_TOP,
            subtitle,
            egui::FontId::proportional(13.0),
            MUTED,
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
                if ui.button("Refresh daemon").clicked() {
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
                        ui.colored_label(ERROR, "Tampered state");
                    }
                });

                if let Some(session) = &status.active_session {
                    let total = session.end_time.saturating_sub(session.started_at).max(1);
                    let elapsed = now.saturating_sub(session.started_at).min(total);
                    let progress = elapsed as f32 / total as f32;
                    ui.add_space(8.0);
                    ui.add(
                        egui::ProgressBar::new(progress)
                            .desired_width(ui.available_width())
                            .text(remaining_label(now, session.end_time)),
                    );
                    ui.add_space(8.0);
                    ui.label(format!("Started: {}", session.started_at));
                    ui.label(format!("Ends: {}", session.end_time));
                    ui.label(format!("Blocked sites: {}", session.sites.len()));
                    ui.label(format!("Blocked apps: {}", session.apps.len()));
                } else {
                    ui.add_space(8.0);
                    ui.label("No active session.");
                }
            }
            None => {
                ui.label("Daemon status has not been loaded.");
                ui.add_space(8.0);
                if ui.button("Check daemon now").clicked() {
                    self.refresh_status();
                }
            }
        }
    }

    fn quick_actions_card(&mut self, ui: &mut egui::Ui) {
        ui.heading("Quick actions");
        ui.label(RichText::new("Build, start, or recover a session.").color(MUTED));
        ui.add_space(12.0);

        if ui
            .add(
                egui::Button::new("Open block builder")
                    .min_size(egui::vec2(ui.available_width(), 38.0)),
            )
            .clicked()
        {
            self.page = Page::Builder;
        }
        if ui
            .add(
                egui::Button::new("Start current draft")
                    .min_size(egui::vec2(ui.available_width(), 38.0)),
            )
            .clicked()
        {
            self.start_block();
        }
        if ui
            .add(
                egui::Button::new("Recover after expiry")
                    .min_size(egui::vec2(ui.available_width(), 38.0)),
            )
            .clicked()
        {
            self.recover();
        }

        ui.add_space(10.0);
        self.message_strip(ui);
    }

    fn builder(&mut self, ui: &mut egui::Ui) {
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
                if ui.button(quick).clicked() {
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
        if ui
            .add(
                egui::Button::new("Start enforced block")
                    .min_size(egui::vec2(ui.available_width(), 46.0)),
            )
            .clicked()
        {
            self.start_block();
        }
        ui.add_space(8.0);
        self.message_strip(ui);
    }

    fn apps_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("App picker");
                if ui.button("Scan running apps").clicked() {
                    self.scan_apps();
                }
                if ui.button("Scan installed apps").clicked() {
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
        if ui.button("Refresh status").clicked() {
            self.refresh_status();
        }
        if ui.button("Recover after expiry").clicked() {
            self.recover();
        }
        ui.add_space(8.0);
        self.message_strip(ui);
    }

    fn site_editor(&mut self, ui: &mut egui::Ui) {
        ui.heading("Selected websites");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.manual_site);
            if ui.button("Add site").clicked() {
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
            if ui.button("Add app").clicked() {
                self.add_manual_apps();
            }
        });
        ui.add_space(8.0);
        removable_rules(ui, &mut self.app_rules);
    }

    fn message_strip(&self, ui: &mut egui::Ui) {
        if let Some(error) = &self.error {
            ui.colored_label(ERROR, error);
        } else {
            ui.colored_label(OK, &self.message);
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
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(Color32::from_rgb(238, 244, 255));
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.widgets.active.bg_fill = ACCENT;
    style.visuals.widgets.hovered.bg_fill = CARD_ALT;
    style.visuals.widgets.inactive.bg_fill = CARD;
    style.visuals.selection.bg_fill = ACCENT;
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    ctx.set_style(style);
}

fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::default()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(16.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, add_contents)
        .inner
}

fn metric_card<T: std::fmt::Display>(ui: &mut egui::Ui, title: &str, value: T, subtitle: &str) {
    card(ui, |ui| {
        ui.label(RichText::new(title).color(MUTED));
        ui.heading(RichText::new(value.to_string()).size(30.0).color(ACCENT_2));
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
                                    if ui.small_button("Remove").clicked() {
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
                    } else if ui.button("Add").clicked() {
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
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(16.0)
        .inner_margin(egui::Margin::same(16.0))
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
            if ui.button("Add category").clicked() {
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
        Page::Apps => "Apps",
        Page::Sites => "Websites",
        Page::Settings => "Settings",
    }
}

fn page_subtitle(page: Page) -> &'static str {
    match page {
        Page::Dashboard => "See the active block and your next draft at a glance.",
        Page::Builder => "Set duration, review selected rules, and start an enforced session.",
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
        Some(ServiceState::Active) => ACCENT_2,
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
}
