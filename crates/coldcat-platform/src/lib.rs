use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::env;

use anyhow::{anyhow, bail, Context, Result};
use coldcat_core::apps::{process_matches_rule, ProcessRecord};
use coldcat_core::hosts::{apply_hosts_block, strip_managed_block};
use coldcat_core::session::ActiveSession;
use coldcat_core::state::{
    decode_key, encode_key, generate_key, open_session, seal_session, SealedSession,
};
use sysinfo::{ProcessExt, Signal, System, SystemExt};

#[derive(Debug, Clone)]
pub struct PlatformPaths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub state_dir: PathBuf,
    pub state_file: PathBuf,
    pub state_backup_file: PathBuf,
    pub key_file: PathBuf,
    pub hosts_file: PathBuf,
}

impl PlatformPaths {
    pub fn detect() -> Self {
        let data_dir = default_data_dir();
        let config_dir = default_config_dir();
        let runtime_dir = default_runtime_dir(&data_dir);
        let state_dir = data_dir.join("state");
        let state_file = state_dir.join("session.json.sealed");
        let state_backup_file = state_dir.join("session.json.sealed.bak");
        let key_file = config_dir.join("state.key");
        let hosts_file = default_hosts_file();

        Self {
            data_dir,
            config_dir,
            runtime_dir,
            state_dir,
            state_file,
            state_backup_file,
            key_file,
            hosts_file,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SystemEnforcer {
    paths: PlatformPaths,
}

impl SystemEnforcer {
    pub fn new(paths: PlatformPaths) -> Self {
        Self { paths }
    }

    pub fn detect() -> Self {
        Self::new(PlatformPaths::detect())
    }

    pub fn paths(&self) -> &PlatformPaths {
        &self.paths
    }

    pub fn ensure_layout(&self) -> Result<()> {
        fs::create_dir_all(&self.paths.state_dir)
            .with_context(|| format!("creating {}", self.paths.state_dir.display()))?;
        fs::create_dir_all(&self.paths.config_dir)
            .with_context(|| format!("creating {}", self.paths.config_dir.display()))?;
        fs::create_dir_all(&self.paths.runtime_dir)
            .with_context(|| format!("creating {}", self.paths.runtime_dir.display()))?;
        restrict_private_path(&self.paths.data_dir, true)?;
        restrict_private_path(&self.paths.state_dir, true)?;
        restrict_private_path(&self.paths.config_dir, true)?;
        restrict_private_path(&self.paths.runtime_dir, true)?;
        Ok(())
    }

    pub fn apply_hosts(&self, sites: &[String]) -> Result<()> {
        let existing = read_text_or_empty(&self.paths.hosts_file)?;
        let desired = apply_hosts_block(&existing, sites)?;
        if desired != existing {
            set_file_hardened(&self.paths.hosts_file, false)?;
            write_text(&self.paths.hosts_file, &desired)?;
        }
        if !sites.is_empty() {
            set_file_hardened(&self.paths.hosts_file, true)?;
        }
        Ok(())
    }

    pub fn clear_hosts(&self) -> Result<()> {
        set_file_hardened(&self.paths.hosts_file, false)?;
        if !self.paths.hosts_file.exists() {
            return Ok(());
        }
        let existing = read_text_or_empty(&self.paths.hosts_file)?;
        let desired = strip_managed_block(&existing)?;
        if desired != existing {
            write_text(&self.paths.hosts_file, &desired)?;
        }
        Ok(())
    }

    pub fn enforce_apps(&self, apps: &[String]) -> Result<usize> {
        if apps.is_empty() {
            return Ok(0);
        }

        let mut system = System::new_all();
        system.refresh_processes();
        let current_pid = sysinfo::get_current_pid().ok();
        let mut killed = 0_usize;
        let mut term_sent = Vec::new();

        for (pid, process) in system.processes() {
            if current_pid.as_ref() == Some(pid) {
                continue;
            }

            let record = ProcessRecord {
                name: process.name().to_string(),
                exe: Some(process.exe().to_string_lossy().to_string()),
            };

            if apps.iter().any(|rule| process_matches_rule(rule, &record)) {
                if process.kill_with(Signal::Term).unwrap_or(false) {
                    killed += 1;
                    term_sent.push(*pid);
                } else if process.kill() {
                    killed += 1;
                }
            }
        }

        if !term_sent.is_empty() {
            thread::sleep(Duration::from_millis(250));
            system.refresh_processes();
            for pid in term_sent {
                if let Some(process) = system.process(pid) {
                    let _ = process.kill();
                }
            }
        }

        Ok(killed)
    }
}

#[derive(Debug, Clone)]
pub struct StateStore {
    paths: PlatformPaths,
    key: Vec<u8>,
}

impl StateStore {
    pub fn open(paths: PlatformPaths) -> Result<Self> {
        fs::create_dir_all(&paths.state_dir)
            .with_context(|| format!("creating {}", paths.state_dir.display()))?;
        fs::create_dir_all(&paths.config_dir)
            .with_context(|| format!("creating {}", paths.config_dir.display()))?;
        fs::create_dir_all(&paths.runtime_dir)
            .with_context(|| format!("creating {}", paths.runtime_dir.display()))?;
        restrict_private_path(&paths.data_dir, true)?;
        restrict_private_path(&paths.state_dir, true)?;
        restrict_private_path(&paths.config_dir, true)?;
        restrict_private_path(&paths.runtime_dir, true)?;

        let key = if paths.key_file.exists() {
            decode_key(&fs::read_to_string(&paths.key_file)?)
                .map_err(|err| anyhow!("failed to decode state key: {err}"))?
        } else {
            let key = generate_key();
            write_text(&paths.key_file, &encode_key(&key))?;
            restrict_private_path(&paths.key_file, false)?;
            set_file_hardened(&paths.key_file, true)?;
            key
        };

        Ok(Self { paths, key })
    }

    pub fn load(&self) -> Result<Option<Vec<ActiveSession>>> {
        match self.load_from(&self.paths.state_file) {
            Ok(session @ Some(_)) => Ok(session),
            Ok(None) => {
                let backup = self.load_from(&self.paths.state_backup_file)?;
                if let Some(session) = &backup {
                    self.save(session)?;
                }
                Ok(backup)
            }
            Err(primary_err) => match self.load_from(&self.paths.state_backup_file) {
                Ok(Some(session)) => {
                    self.save(&session)?;
                    Ok(Some(session))
                }
                Ok(None) => Err(primary_err),
                Err(backup_err) => Err(anyhow!(
                    "primary and backup sealed state validation failed; primary: {primary_err:#}; backup: {backup_err:#}"
                )),
            },
        }
    }

    pub fn save(&self, sessions: &[ActiveSession]) -> Result<()> {
        set_file_hardened(&self.paths.state_file, false)?;
        set_file_hardened(&self.paths.state_backup_file, false)?;
        restrict_private_path(&self.paths.state_dir, true)?;
        restrict_private_path(&self.paths.state_file, false)?;
        restrict_private_path(&self.paths.state_backup_file, false)?;
        let sealed = seal_session(sessions, &self.key)
            .map_err(|err| anyhow!("failed to seal session: {err}"))?;
        let rendered = serde_json::to_string_pretty(&sealed)?;
        write_text(&self.paths.state_file, &rendered)?;
        write_text(&self.paths.state_backup_file, &rendered)?;
        restrict_private_path(&self.paths.state_file, false)?;
        restrict_private_path(&self.paths.state_backup_file, false)?;
        set_file_hardened(&self.paths.state_file, true)?;
        set_file_hardened(&self.paths.state_backup_file, true)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        set_file_hardened(&self.paths.state_file, false)?;
        set_file_hardened(&self.paths.state_backup_file, false)?;
        if self.paths.state_file.exists() {
            fs::remove_file(&self.paths.state_file)
                .with_context(|| format!("removing {}", self.paths.state_file.display()))?;
        }
        if self.paths.state_backup_file.exists() {
            fs::remove_file(&self.paths.state_backup_file)
                .with_context(|| format!("removing {}", self.paths.state_backup_file.display()))?;
        }
        Ok(())
    }

    fn load_from(&self, path: &Path) -> Result<Option<Vec<ActiveSession>>> {
        if !path.exists() {
            return Ok(None);
        }
        let sealed: SealedSession = serde_json::from_str(&fs::read_to_string(path)?)
            .with_context(|| format!("reading sealed state from {}", path.display()))?;
        let sessions = open_session(&sealed, &self.key).map_err(|err| {
            anyhow!(
                "sealed session validation failed for {}: {err}",
                path.display()
            )
        })?;
        Ok(Some(sessions))
    }
}

fn read_text_or_empty(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(value) => Ok(value),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

fn write_text(path: &Path, value: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(path, value).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(windows)]
fn default_data_dir() -> PathBuf {
    env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("ColdCat")
}

#[cfg(not(windows))]
fn default_data_dir() -> PathBuf {
    PathBuf::from("/var/lib/cold-cat")
}

#[cfg(windows)]
fn default_config_dir() -> PathBuf {
    env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("ColdCat")
        .join("config")
}

#[cfg(not(windows))]
fn default_config_dir() -> PathBuf {
    PathBuf::from("/etc/cold-cat")
}

#[cfg(windows)]
fn default_runtime_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("run")
}

#[cfg(not(windows))]
fn default_runtime_dir(_data_dir: &Path) -> PathBuf {
    PathBuf::from("/run/cold-cat")
}

#[cfg(windows)]
fn default_hosts_file() -> PathBuf {
    env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("drivers")
        .join("etc")
        .join("hosts")
}

#[cfg(not(windows))]
fn default_hosts_file() -> PathBuf {
    PathBuf::from("/etc/hosts")
}

#[cfg(windows)]
fn set_file_hardened(path: &Path, hardened: bool) -> Result<()> {
    if !path_should_be_hardened(path) {
        return Ok(());
    }
    let flag = if hardened { "+R" } else { "-R" };
    let status = Command::new("attrib")
        .arg(flag)
        .arg(path)
        .status()
        .with_context(|| format!("running attrib for {}", path.display()))?;
    if !status.success() {
        bail!("attrib {flag} failed for {}", path.display());
    }
    Ok(())
}

#[cfg(windows)]
fn restrict_private_path(path: &Path, directory: bool) -> Result<()> {
    if !path_should_be_hardened(path) {
        return Ok(());
    }
    let system_rule = if directory {
        "*S-1-5-18:(OI)(CI)F"
    } else {
        "*S-1-5-18:F"
    };
    let admins_rule = if directory {
        "*S-1-5-32-544:(OI)(CI)F"
    } else {
        "*S-1-5-32-544:F"
    };
    let status = Command::new("icacls")
        .arg(path)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(system_rule)
        .arg(admins_rule)
        .arg("/remove:g")
        .arg("*S-1-5-11")
        .arg("*S-1-5-32-545")
        .arg("*S-1-1-0")
        .status()
        .with_context(|| format!("running icacls for {}", path.display()))?;
    if !status.success() {
        bail!("icacls hardening failed for {}", path.display());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_file_hardened(path: &Path, hardened: bool) -> Result<()> {
    if !path_should_be_hardened(path) {
        return Ok(());
    }
    let flag = if hardened { "+i" } else { "-i" };
    let status = Command::new("chattr")
        .arg(flag)
        .arg(path)
        .status()
        .with_context(|| format!("running chattr for {}", path.display()))?;
    if !status.success() {
        bail!("chattr {flag} failed for {}", path.display());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn restrict_private_path(path: &Path, directory: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if !path_should_be_hardened(path) {
        return Ok(());
    }

    let mode = if directory { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("setting mode {:o} on {}", mode, path.display()))?;
    Ok(())
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn set_file_hardened(_path: &Path, _hardened: bool) -> Result<()> {
    Ok(())
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn restrict_private_path(_path: &Path, _directory: bool) -> Result<()> {
    Ok(())
}

fn path_should_be_hardened(path: &Path) -> bool {
    path.try_exists().unwrap_or(true)
}
