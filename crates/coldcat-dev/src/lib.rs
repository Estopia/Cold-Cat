use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use coldcat_core::client::send_command_to;
use coldcat_core::protocol::CommandRequest;
use coldcat_core::DAEMON_ADDR_ENV;
use coldcat_platform::{CONFIG_DIR_ENV, DATA_DIR_ENV, HOSTS_FILE_ENV, RUNTIME_DIR_ENV};

pub const DEV_DAEMON_ADDR: &str = "127.0.0.1:47852";

const DEV_HOSTS_CONTENT: &str = "127.0.0.1 localhost\n";
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Debug, Parser)]
#[command(name = "coldcat-dev")]
#[command(about = "Run Cold Cat locally against a workspace sandbox")]
struct Args {
    #[command(subcommand)]
    command: Option<DevCommand>,
}

#[derive(Debug, Subcommand)]
enum DevCommand {
    /// Build and launch the daemon plus native GUI against the local sandbox.
    Gui,
    /// Build and launch only the daemon against the local sandbox.
    Daemon,
    /// Build the daemon plus CLI, start the daemon, and run a CLI command against it.
    Cli {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Print the sandbox paths and environment overrides.
    Env,
}

#[derive(Debug, Clone)]
pub struct DevPaths {
    pub workspace_root: PathBuf,
    pub root: PathBuf,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub run_dir: PathBuf,
    pub hosts_file: PathBuf,
    xdg_config_home: PathBuf,
    appdata_dir: PathBuf,
    local_appdata_dir: PathBuf,
}

impl DevPaths {
    pub fn new(workspace_root: &Path) -> Self {
        let root = workspace_root.join(".coldcat-dev");
        Self {
            workspace_root: workspace_root.to_path_buf(),
            root: root.clone(),
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            run_dir: root.join("run"),
            hosts_file: root.join("hosts"),
            xdg_config_home: root.join("xdg-config"),
            appdata_dir: root.join("appdata"),
            local_appdata_dir: root.join("local-appdata"),
        }
    }

    pub fn env_pairs(&self) -> Vec<(String, String)> {
        let mut env_pairs = vec![
            (DAEMON_ADDR_ENV.to_string(), DEV_DAEMON_ADDR.to_string()),
            (DATA_DIR_ENV.to_string(), display_path(&self.data_dir)),
            (CONFIG_DIR_ENV.to_string(), display_path(&self.config_dir)),
            (RUNTIME_DIR_ENV.to_string(), display_path(&self.run_dir)),
            (HOSTS_FILE_ENV.to_string(), display_path(&self.hosts_file)),
            (
                "XDG_CONFIG_HOME".to_string(),
                display_path(&self.xdg_config_home),
            ),
        ];

        if cfg!(windows) {
            env_pairs.push(("APPDATA".to_string(), display_path(&self.appdata_dir)));
            env_pairs.push((
                "LOCALAPPDATA".to_string(),
                display_path(&self.local_appdata_dir),
            ));
        }

        env_pairs
    }

    pub fn prepare(&self) -> Result<()> {
        fs::create_dir_all(&self.data_dir)
            .with_context(|| format!("creating {}", self.data_dir.display()))?;
        fs::create_dir_all(&self.config_dir)
            .with_context(|| format!("creating {}", self.config_dir.display()))?;
        fs::create_dir_all(&self.run_dir)
            .with_context(|| format!("creating {}", self.run_dir.display()))?;
        fs::create_dir_all(&self.xdg_config_home)
            .with_context(|| format!("creating {}", self.xdg_config_home.display()))?;

        if cfg!(windows) {
            fs::create_dir_all(&self.appdata_dir)
                .with_context(|| format!("creating {}", self.appdata_dir.display()))?;
            fs::create_dir_all(&self.local_appdata_dir)
                .with_context(|| format!("creating {}", self.local_appdata_dir.display()))?;
        }

        if !self.hosts_file.exists() {
            fs::write(&self.hosts_file, DEV_HOSTS_CONTENT)
                .with_context(|| format!("writing {}", self.hosts_file.display()))?;
        }

        Ok(())
    }
}

pub fn run() -> Result<()> {
    run_from_iter(env::args_os())
}

pub fn run_from_iter<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args = Args::parse_from(args);
    let paths = DevPaths::new(&workspace_root()?);

    match args.command.unwrap_or(DevCommand::Gui) {
        DevCommand::Gui => run_gui(&paths),
        DevCommand::Daemon => run_daemon(&paths),
        DevCommand::Cli { args } => run_cli(&paths, args),
        DevCommand::Env => {
            print_env(&paths);
            Ok(())
        }
    }
}

fn run_gui(paths: &DevPaths) -> Result<()> {
    paths.prepare()?;
    build_packages(&paths.workspace_root, &["coldcat-daemon", "coldcat-gui"])?;

    let mut daemon = spawn_daemon(paths)?;
    let run_result = (|| {
        wait_for_daemon(&mut daemon)?;
        let status = binary_command(paths, "coldcat-gui")
            .status()
            .context("starting coldcat-gui")?;
        if !status.success() {
            bail!("coldcat-gui exited with status {status}");
        }
        Ok(())
    })();

    stop_child(&mut daemon)?;
    run_result
}

fn run_daemon(paths: &DevPaths) -> Result<()> {
    paths.prepare()?;
    build_packages(&paths.workspace_root, &["coldcat-daemon"])?;

    let status = binary_command(paths, "coldcat-daemon")
        .arg("run")
        .status()
        .context("starting coldcat-daemon")?;
    if !status.success() {
        bail!("coldcat-daemon exited with status {status}");
    }
    Ok(())
}

fn run_cli(paths: &DevPaths, args: Vec<String>) -> Result<()> {
    paths.prepare()?;
    build_packages(&paths.workspace_root, &["coldcat-daemon", "coldcat-cli"])?;

    let mut daemon = spawn_daemon(paths)?;
    let run_result = (|| {
        wait_for_daemon(&mut daemon)?;
        let mut command = binary_command(paths, "coldcat");
        if args.is_empty() {
            command.arg("doctor");
        } else {
            command.args(args);
        }
        let status = command.status().context("starting coldcat CLI")?;
        if !status.success() {
            bail!("coldcat CLI exited with status {status}");
        }
        Ok(())
    })();

    stop_child(&mut daemon)?;
    run_result
}

fn print_env(paths: &DevPaths) {
    println!("workspace_root: {}", paths.workspace_root.display());
    println!("sandbox_root: {}", paths.root.display());
    println!("daemon_addr: {DEV_DAEMON_ADDR}");
    for (key, value) in paths.env_pairs() {
        println!("{key}={value}");
    }
    println!(
        "hosts_note: sandbox mode writes managed hosts entries to {} instead of the system hosts file",
        paths.hosts_file.display()
    );
}

fn build_packages(workspace_root: &Path, packages: &[&str]) -> Result<()> {
    let mut command = Command::new(cargo_program());
    command
        .current_dir(workspace_root)
        .arg("build")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    for package in packages {
        command.arg("-p").arg(package);
    }

    let status = command.status().context("building dev binaries")?;
    if !status.success() {
        bail!("cargo build exited with status {status}");
    }
    Ok(())
}

fn spawn_daemon(paths: &DevPaths) -> Result<Child> {
    binary_command(paths, "coldcat-daemon")
        .arg("run")
        .spawn()
        .context("starting coldcat-daemon")
}

fn wait_for_daemon(daemon: &mut Child) -> Result<()> {
    let start = Instant::now();

    while start.elapsed() < READY_TIMEOUT {
        if let Some(status) = daemon.try_wait().context("checking daemon status")? {
            bail!("coldcat-daemon exited before it became ready: {status}");
        }

        if send_command_to(
            DEV_DAEMON_ADDR,
            CommandRequest::Status,
            Duration::from_millis(250),
        )
        .is_ok()
        {
            return Ok(());
        }

        thread::sleep(READY_POLL_INTERVAL);
    }

    bail!("timed out waiting for coldcat-daemon at {DEV_DAEMON_ADDR}")
}

fn stop_child(child: &mut Child) -> Result<()> {
    if child.try_wait().context("checking daemon exit state")?.is_some() {
        return Ok(());
    }

    child.kill().context("stopping coldcat-daemon")?;
    child.wait().context("waiting for coldcat-daemon to exit")?;
    Ok(())
}

fn binary_command(paths: &DevPaths, binary_name: &str) -> Command {
    let mut command = Command::new(binary_path(paths, binary_name));
    command
        .current_dir(&paths.workspace_root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    for (key, value) in paths.env_pairs() {
        command.env(key, value);
    }

    command
}

fn binary_path(paths: &DevPaths, binary_name: &str) -> PathBuf {
    target_dir(&paths.workspace_root)
        .join("debug")
        .join(binary_file_name(binary_name))
}

fn binary_file_name(binary_name: &str) -> String {
    if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    }
}

fn target_dir(workspace_root: &Path) -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root.join("target"))
}

fn cargo_program() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn workspace_root() -> Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("could not determine workspace root"))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn builds_workspace_local_sandbox_paths() {
        let paths = DevPaths::new(Path::new("/tmp/coldcat-workspace"));

        assert_eq!(paths.root, PathBuf::from("/tmp/coldcat-workspace/.coldcat-dev"));
        assert_eq!(paths.data_dir, PathBuf::from("/tmp/coldcat-workspace/.coldcat-dev/data"));
        assert_eq!(
            paths.config_dir,
            PathBuf::from("/tmp/coldcat-workspace/.coldcat-dev/config")
        );
        assert_eq!(paths.run_dir, PathBuf::from("/tmp/coldcat-workspace/.coldcat-dev/run"));
        assert_eq!(
            paths.hosts_file,
            PathBuf::from("/tmp/coldcat-workspace/.coldcat-dev/hosts")
        );
    }

    #[test]
    fn exports_expected_override_env_pairs() {
        let env_pairs = DevPaths::new(Path::new("/tmp/coldcat-workspace")).env_pairs();
        let env_pairs: BTreeMap<_, _> = env_pairs.into_iter().collect();

        assert_eq!(
            env_pairs.get("COLDCAT_DATA_DIR"),
            Some(&"/tmp/coldcat-workspace/.coldcat-dev/data".to_string())
        );
        assert_eq!(
            env_pairs.get("COLDCAT_CONFIG_DIR"),
            Some(&"/tmp/coldcat-workspace/.coldcat-dev/config".to_string())
        );
        assert_eq!(
            env_pairs.get("COLDCAT_RUNTIME_DIR"),
            Some(&"/tmp/coldcat-workspace/.coldcat-dev/run".to_string())
        );
        assert_eq!(
            env_pairs.get("COLDCAT_HOSTS_FILE"),
            Some(&"/tmp/coldcat-workspace/.coldcat-dev/hosts".to_string())
        );
        assert_eq!(env_pairs.get("COLDCAT_DAEMON_ADDR"), Some(&DEV_DAEMON_ADDR.to_string()));
    }
}