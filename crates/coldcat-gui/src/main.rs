use coldcat_core::client::send_command;
use coldcat_core::duration::parse_duration_secs;
use coldcat_core::protocol::{CommandRequest, CommandResponse, ServiceState, ServiceStatus};
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("Cold Cat"),
        ..Default::default()
    };

    eframe::run_native(
        "Cold Cat",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(ColdCatApp::default()))
        }),
    )
}

#[derive(Debug)]
struct ColdCatApp {
    duration: String,
    sites: String,
    apps: String,
    status: Option<ServiceStatus>,
    message: String,
    error: Option<String>,
    last_action: String,
}

impl Default for ColdCatApp {
    fn default() -> Self {
        Self {
            duration: "2h".to_string(),
            sites: "reddit.com\nold.reddit.com".to_string(),
            apps: "discord\nsteam".to_string(),
            status: None,
            message: "Connect to the Cold Cat daemon to view status or start a block.".to_string(),
            error: None,
            last_action: "Ready".to_string(),
        }
    }
}

impl eframe::App for ColdCatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Cold Cat");
                ui.separator();
                ui.label("Focus sessions with enforced website and app blocking");
            });
        });

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Last action: {}", self.last_action));
                if let Some(status) = &self.status {
                    ui.separator();
                    ui.label(format!("Daemon state: {}", state_label(&status.state)));
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            egui::SidePanel::left("setup")
                .resizable(true)
                .default_width(420.0)
                .show_inside(ui, |ui| self.setup_panel(ui));

            egui::CentralPanel::default().show_inside(ui, |ui| self.status_panel(ui));
        });
    }
}

impl ColdCatApp {
    fn setup_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("New enforced block");
        ui.label("Pick a duration and add one domain or app per line. Commas also work.");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Duration");
            ui.text_edit_singleline(&mut self.duration);
        });

        ui.horizontal_wrapped(|ui| {
            for quick in ["30m", "1h", "2h", "4h", "8h"] {
                if ui.button(quick).clicked() {
                    self.duration = quick.to_string();
                }
            }
        });

        ui.add_space(8.0);
        ui.columns(2, |columns| {
            columns[0].label("Websites");
            columns[0].add(
                egui::TextEdit::multiline(&mut self.sites)
                    .hint_text("reddit.com\nyoutube.com")
                    .desired_rows(12)
                    .lock_focus(true),
            );

            columns[1].label("Applications");
            columns[1].add(
                egui::TextEdit::multiline(&mut self.apps)
                    .hint_text("discord\nsteam.exe")
                    .desired_rows(12)
                    .lock_focus(true),
            );
        });

        ui.add_space(10.0);
        let start = egui::Button::new("Start enforced block")
            .min_size(egui::vec2(ui.available_width(), 42.0));
        if ui.add(start).clicked() {
            self.start_block();
        }

        ui.horizontal(|ui| {
            if ui.button("Refresh status").clicked() {
                self.refresh_status();
            }
            if ui.button("Recover after expiry").clicked() {
                self.recover();
            }
        });

        ui.add_space(8.0);
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(255, 130, 130), error);
        } else {
            ui.colored_label(egui::Color32::from_rgb(160, 220, 160), &self.message);
        }
    }

    fn status_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Current session");
        ui.label(
            "Cold Cat never exposes an early stop button while an enforced session is active.",
        );
        ui.add_space(8.0);

        match &self.status {
            Some(status) => {
                ui.group(|ui| {
                    ui.heading(state_label(&status.state));
                    ui.label(format!("Current Unix time: {}", status.now));
                    if status.tampered_state {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 130, 130),
                            "Sealed state validation failed. Administrator recovery is required.",
                        );
                    }
                });

                ui.add_space(10.0);
                if let Some(session) = &status.active_session {
                    ui.group(|ui| {
                        ui.label(format!("Session ID: {}", session.id));
                        ui.label(format!("Started: {}", session.started_at));
                        ui.label(format!("Ends: {}", session.end_time));
                        ui.label(format!(
                            "Remaining: {}",
                            remaining_label(status.now, session.end_time)
                        ));
                    });

                    ui.add_space(10.0);
                    ui.columns(2, |columns| {
                        columns[0].heading("Blocked sites");
                        if session.sites.is_empty() {
                            columns[0].label("None");
                        } else {
                            for site in &session.sites {
                                columns[0].label(format!("- {site}"));
                            }
                        }

                        columns[1].heading("Blocked apps");
                        if session.apps.is_empty() {
                            columns[1].label("None");
                        } else {
                            for app in &session.apps {
                                columns[1].label(format!("- {app}"));
                            }
                        }
                    });
                } else {
                    ui.group(|ui| {
                        ui.heading("No active session");
                        ui.label("Create a block from the left panel when you are ready.");
                    });
                }
            }
            None => {
                ui.group(|ui| {
                    ui.heading("Daemon status unknown");
                    ui.label("Click Refresh status. If it cannot connect, install and start the Cold Cat daemon service.");
                });
            }
        }
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

        let sites = parse_rule_text(&self.sites);
        let apps = parse_rule_text(&self.apps);
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
}

fn parse_rule_text(input: &str) -> Vec<String> {
    input
        .split(['\n', '\r', ',', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
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
            parse_rule_text("reddit.com, old.reddit.com\n\nsteam; discord"),
            vec!["reddit.com", "old.reddit.com", "steam", "discord"]
        );
    }

    #[test]
    fn formats_remaining_time() {
        assert_eq!(remaining_label(100, 100), "expired");
        assert_eq!(remaining_label(100, 145), "45s");
        assert_eq!(remaining_label(100, 3_800), "1h 1m");
    }
}
