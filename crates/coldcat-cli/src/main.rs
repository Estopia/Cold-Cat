use anyhow::Result;
use clap::{Parser, Subcommand};
use coldcat_core::client::send_command;
use coldcat_core::duration::parse_duration_secs;
use coldcat_core::protocol::{CommandRequest, CommandResponse, ServiceState};
use coldcat_core::daemon_addr;
use coldcat_platform::{
    discover_installed_apps, expand_app_rules, repair_installation, PlatformPaths,
};

#[derive(Debug, Parser)]
#[command(name = "coldcat")]
#[command(about = "Cold Cat website and application blocker")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start an enforced block session.
    Start {
        /// Duration such as 30m, 2h, 1h30m, or raw seconds.
        #[arg(long, value_parser = parse_duration)]
        duration: u64,
        /// Comma-separated domains to block.
        #[arg(long, value_delimiter = ',')]
        sites: Vec<String>,
        /// Comma-separated process names or executable paths to block.
        #[arg(long, value_delimiter = ',')]
        apps: Vec<String>,
    },
    /// Show daemon status.
    Status,
    /// Run post-expiry cleanup. Refused while a session is active.
    Recover,
    /// List installed applications discovered for app-block rules.
    Apps,
    /// Repair Cold Cat's local privileged directories and file modes.
    Repair,
    /// Check local paths and daemon connectivity.
    Doctor,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Start {
            duration,
            sites,
            apps,
        } => {
            let response = send_command(CommandRequest::Start {
                name: "CLI Block".to_string(),
                duration_secs: duration,
                sites,
                apps: expand_app_rules(&apps),
            })?;
            print_response(&response);
            exit_for_response(response);
        }
        Command::Status => {
            let response = send_command(CommandRequest::Status)?;
            print_response(&response);
            exit_for_response(response);
        }
        Command::Recover => {
            let response = send_command(CommandRequest::Recover)?;
            print_response(&response);
            exit_for_response(response);
        }
        Command::Apps => list_apps(),
        Command::Repair => repair(),
        Command::Doctor => doctor(),
    }
}

fn parse_duration(input: &str) -> std::result::Result<u64, String> {
    parse_duration_secs(input).map_err(|err| err.to_string())
}

fn print_response(response: &CommandResponse) {
    if response.ok {
        println!("{}", response.message);
    } else {
        eprintln!("{}", response.message);
    }

    if let Some(status) = &response.status {
        println!("state: {:?}", status.state);
        println!("now: {}", status.now);
        if !status.active_sessions.is_empty() {
            println!("Active blocks:");
            for session in &status.active_sessions {
                let total = session.end_time.saturating_sub(session.started_at);
                let remaining = session.end_time.saturating_sub(status.now);
                println!(
                    "  - '{}' ends in {}s (total {}s)",
                    session.name, remaining, total
                );
                if !session.sites.is_empty() {
                    println!("    Sites: {}", session.sites.join(", "));
                }
                if !session.apps.is_empty() {
                    println!("    Apps:  {}", session.apps.join(", "));
                }
            }
        } else {
            println!("No active sessions.");
        }
        if matches!(status.state, ServiceState::Tampered) || status.tampered_state {
            println!("tampered_state: true");
        }
    }
}

fn exit_for_response(response: CommandResponse) -> ! {
    std::process::exit(if response.ok { 0 } else { 1 });
}

fn doctor() -> Result<()> {
    let paths = PlatformPaths::detect();
    println!("daemon_addr: {}", daemon_addr());
    println!("data_dir: {}", paths.data_dir.display());
    println!("config_dir: {}", paths.config_dir.display());
    println!("runtime_dir: {}", paths.runtime_dir.display());
    println!("state_file: {}", paths.state_file.display());
    println!("state_backup_file: {}", paths.state_backup_file.display());
    println!("key_file: {}", paths.key_file.display());
    println!("hosts_file: {}", paths.hosts_file.display());
    println!("installed_apps: {}", discover_installed_apps().len());

    match send_command(CommandRequest::Status) {
        Ok(response) => {
            println!("daemon: reachable");
            print_response(&response);
        }
        Err(err) => {
            println!("daemon: unreachable ({err:#})");
        }
    }

    Ok(())
}

fn list_apps() -> Result<()> {
    let apps = discover_installed_apps();
    if apps.is_empty() {
        println!("No installed applications discovered.");
        return Ok(());
    }

    for app in apps {
        println!("{}\t{}\t{}", app.name, app.rule, app.detail);
    }
    Ok(())
}

fn repair() -> Result<()> {
    repair_installation()?;
    println!("Cold Cat local directories and file modes repaired.");
    println!(
        "Run `sudo packaging/linux/install.sh --repair` from the repository to repair the Linux systemd unit and desktop launcher."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_start_command() {
        let args = Args::try_parse_from([
            "coldcat",
            "start",
            "--duration",
            "2h",
            "--sites",
            "reddit.com,old.reddit.com",
            "--apps",
            "discord,steam",
        ])
        .unwrap();

        match args.command {
            Command::Start {
                duration,
                sites,
                apps,
            } => {
                assert_eq!(duration, 7200);
                assert_eq!(sites, vec!["reddit.com", "old.reddit.com"]);
                assert_eq!(apps, vec!["discord", "steam"]);
            }
            _ => panic!("expected start command"),
        }
    }

    #[test]
    fn rejects_bad_duration() {
        assert!(Args::try_parse_from(["coldcat", "start", "--duration", "soon"]).is_err());
    }

    #[test]
    fn parses_linux_support_commands() {
        assert!(matches!(
            Args::try_parse_from(["coldcat", "apps"]).unwrap().command,
            Command::Apps
        ));
        assert!(matches!(
            Args::try_parse_from(["coldcat", "repair"]).unwrap().command,
            Command::Repair
        ));
    }
}
