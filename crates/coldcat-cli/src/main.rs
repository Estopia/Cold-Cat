use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use coldcat_core::duration::parse_duration_secs;
use coldcat_core::protocol::{CommandRequest, CommandResponse, ServiceState};
use coldcat_core::DEFAULT_DAEMON_ADDR;
use coldcat_platform::PlatformPaths;

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
            let response = send(CommandRequest::Start {
                duration_secs: duration,
                sites,
                apps,
            })?;
            print_response(&response);
            exit_for_response(response);
        }
        Command::Status => {
            let response = send(CommandRequest::Status)?;
            print_response(&response);
            exit_for_response(response);
        }
        Command::Recover => {
            let response = send(CommandRequest::Recover)?;
            print_response(&response);
            exit_for_response(response);
        }
        Command::Doctor => doctor(),
    }
}

fn parse_duration(input: &str) -> std::result::Result<u64, String> {
    parse_duration_secs(input).map_err(|err| err.to_string())
}

fn send(request: CommandRequest) -> Result<CommandResponse> {
    let addr = DEFAULT_DAEMON_ADDR
        .to_socket_addrs()
        .context("resolving daemon address")?
        .next()
        .ok_or_else(|| anyhow!("no daemon address resolved"))?;

    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .with_context(|| format!("connecting to daemon at {DEFAULT_DAEMON_ADDR}"))?;
    serde_json::to_writer(&mut stream, &request).context("encoding request")?;
    stream.write_all(b"\n").context("sending request")?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .context("reading daemon response")?;
    serde_json::from_str(&line).context("decoding daemon response")
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
        if let Some(session) = &status.active_session {
            println!("session: {}", session.id);
            println!("started_at: {}", session.started_at);
            println!("end_time: {}", session.end_time);
            if !session.sites.is_empty() {
                println!("sites: {}", session.sites.join(","));
            }
            if !session.apps.is_empty() {
                println!("apps: {}", session.apps.join(","));
            }
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
    println!("daemon_addr: {DEFAULT_DAEMON_ADDR}");
    println!("data_dir: {}", paths.data_dir.display());
    println!("config_dir: {}", paths.config_dir.display());
    println!("runtime_dir: {}", paths.runtime_dir.display());
    println!("state_file: {}", paths.state_file.display());
    println!("key_file: {}", paths.key_file.display());
    println!("hosts_file: {}", paths.hosts_file.display());

    match send(CommandRequest::Status) {
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
}
