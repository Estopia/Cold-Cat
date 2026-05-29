use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;

include!(concat!(env!("OUT_DIR"), "/payload.rs"));

const SERVICE_NAME: &str = "ColdCatDaemon";
const BINARIES: &[&str] = &["coldcat.exe", "coldcat-daemon.exe", "coldcat-gui.exe"];

#[derive(Debug, Parser)]
#[command(name = "coldcat-setup")]
#[command(about = "Install or update Cold Cat on Windows")]
struct Args {
    /// Install directory. Defaults to %ProgramFiles%\Cold Cat.
    #[arg(long)]
    install_dir: Option<PathBuf>,
    /// Do not start the daemon after installing/updating.
    #[arg(long)]
    no_start: bool,
    /// Remove the Windows service and Start Menu shortcut, but leave ProgramData state alone.
    #[arg(long)]
    uninstall: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure_windows()?;
    ensure_elevated()?;

    if args.uninstall {
        uninstall(args.install_dir)?;
    } else {
        install_or_update(args.install_dir, args.no_start)?;
    }

    Ok(())
}

fn install_or_update(install_dir: Option<PathBuf>, no_start: bool) -> Result<()> {
    let install_dir = install_dir.unwrap_or_else(default_install_dir);
    println!("Installing Cold Cat to {}", install_dir.display());
    fs::create_dir_all(&install_dir)
        .with_context(|| format!("creating {}", install_dir.display()))?;

    stop_and_remove_service()?;
    repair_program_data_acl()?;

    for binary in BINARIES {
        install_binary(binary, &install_dir)?;
    }

    create_service(&install_dir)?;
    create_start_menu_shortcut(&install_dir)?;

    if !no_start {
        run_command("sc.exe", &["start", SERVICE_NAME], false)?;
    }

    println!("Cold Cat installation/update complete.");
    println!("Launch it from the Windows Start Menu as 'Cold Cat'.");
    Ok(())
}

fn uninstall(install_dir: Option<PathBuf>) -> Result<()> {
    let install_dir = install_dir.unwrap_or_else(default_install_dir);
    stop_and_remove_service()?;
    remove_start_menu_shortcut()?;
    println!("Removed Cold Cat service and Start Menu shortcut.");
    println!(
        "Installed binaries remain at {} for manual removal.",
        install_dir.display()
    );
    Ok(())
}

fn install_binary(binary: &str, install_dir: &Path) -> Result<()> {
    let destination = install_dir.join(binary);
    if let Some(payload) = payload_for(binary) {
        fs::write(&destination, payload)
            .with_context(|| format!("writing bundled {}", destination.display()))?;
        println!("Installed bundled {binary}");
        return Ok(());
    }

    let source = side_by_side_binary(binary)?;
    fs::copy(&source, &destination)
        .with_context(|| format!("copying {} to {}", source.display(), destination.display()))?;
    println!("Installed {binary} from {}", source.display());
    Ok(())
}

fn side_by_side_binary(binary: &str) -> Result<PathBuf> {
    let setup_dir = env::current_exe()
        .context("locating setup executable")?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("setup executable has no parent directory"))?;
    let source = setup_dir.join(binary);
    if source.exists() {
        Ok(source)
    } else {
        bail!(
            "{binary} was not bundled and was not found next to the setup executable at {}",
            source.display()
        )
    }
}

fn repair_program_data_acl() -> Result<()> {
    let data_dir = default_data_dir();
    let state_dir = data_dir.join("state");
    let config_dir = data_dir.join("config");
    let runtime_dir = data_dir.join("run");

    for dir in [&data_dir, &state_dir, &config_dir, &runtime_dir] {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        harden_directory_acl(dir)?;
    }

    for file in [
        state_dir.join("session.json.sealed"),
        state_dir.join("session.json.sealed.bak"),
        config_dir.join("state.key"),
    ] {
        if file.try_exists().unwrap_or(true) {
            harden_file_acl(&file)?;
        }
    }

    Ok(())
}

fn harden_directory_acl(path: &Path) -> Result<()> {
    let path = path.to_string_lossy().into_owned();
    run_command(
        "icacls.exe",
        &[
            path.as_str(),
            "/inheritance:r",
            "/grant:r",
            "*S-1-5-18:(OI)(CI)F",
            "*S-1-5-32-544:(OI)(CI)F",
            "/remove:g",
            "*S-1-5-11",
            "*S-1-5-32-545",
            "*S-1-1-0",
        ],
        true,
    )
    .with_context(|| format!("repairing ACL on {path}"))
}

fn harden_file_acl(path: &Path) -> Result<()> {
    let path = path.to_string_lossy().into_owned();
    run_command(
        "icacls.exe",
        &[
            path.as_str(),
            "/inheritance:r",
            "/grant:r",
            "*S-1-5-18:F",
            "*S-1-5-32-544:F",
            "/remove:g",
            "*S-1-5-11",
            "*S-1-5-32-545",
            "*S-1-1-0",
        ],
        true,
    )
    .with_context(|| format!("repairing ACL on {path}"))
}

fn create_service(install_dir: &Path) -> Result<()> {
    let daemon = install_dir.join("coldcat-daemon.exe");
    let bin_path = format!("\"{}\" service", daemon.display());

    run_command(
        "sc.exe",
        &[
            "create",
            SERVICE_NAME,
            "binPath=",
            &bin_path,
            "start=",
            "auto",
            "DisplayName=",
            "Cold Cat Daemon",
        ],
        true,
    )?;
    run_command(
        "sc.exe",
        &[
            "description",
            SERVICE_NAME,
            "Privileged Cold Cat website and application blocking service.",
        ],
        false,
    )?;
    run_command(
        "sc.exe",
        &[
            "failure",
            SERVICE_NAME,
            "reset=",
            "0",
            "actions=",
            "restart/1000/restart/1000/restart/1000",
        ],
        false,
    )?;
    Ok(())
}

fn stop_and_remove_service() -> Result<()> {
    if !service_exists()? {
        return Ok(());
    }

    let _ = run_command("sc.exe", &["stop", SERVICE_NAME], false);
    thread::sleep(Duration::from_secs(2));
    let _ = run_command("sc.exe", &["delete", SERVICE_NAME], false);
    thread::sleep(Duration::from_secs(2));
    Ok(())
}

fn service_exists() -> Result<bool> {
    let status = Command::new("sc.exe")
        .args(["query", SERVICE_NAME])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("checking existing service")?;
    Ok(status.success())
}

fn create_start_menu_shortcut(install_dir: &Path) -> Result<()> {
    let gui = install_dir.join("coldcat-gui.exe");
    let escaped_gui = escape_powershell_single_quoted(&gui.to_string_lossy());
    let script = format!(
        "$dir = [Environment]::GetFolderPath('CommonPrograms'); \
         $shortcutPath = Join-Path $dir 'Cold Cat.lnk'; \
         $shell = New-Object -ComObject WScript.Shell; \
         $shortcut = $shell.CreateShortcut($shortcutPath); \
         $shortcut.TargetPath = '{escaped_gui}'; \
         $shortcut.WorkingDirectory = '{}'; \
         $shortcut.Description = 'Launch Cold Cat'; \
         $shortcut.Save();",
        escape_powershell_single_quoted(&install_dir.to_string_lossy())
    );
    run_command(
        "powershell.exe",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ],
        true,
    )
}

fn remove_start_menu_shortcut() -> Result<()> {
    let script = "$path = Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'Cold Cat.lnk'; if (Test-Path $path) { Remove-Item $path -Force }";
    run_command(
        "powershell.exe",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
        false,
    )
}

fn run_command(program: &str, args: &[&str], required: bool) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("running {program}"))?;
    if output.status.success() || !required {
        return Ok(());
    }

    bail!(
        "{} failed with status {:?}: {}{}",
        program,
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn ensure_windows() -> Result<()> {
    if cfg!(windows) {
        Ok(())
    } else {
        bail!("Cold Cat setup currently installs the Windows build only")
    }
}

fn ensure_elevated() -> Result<()> {
    let status = Command::new("net")
        .arg("session")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("checking administrator privileges")?;
    if status.success() {
        Ok(())
    } else {
        bail!("Run ColdCatSetup.exe from an elevated Administrator terminal")
    }
}

fn default_install_dir() -> PathBuf {
    env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
        .join("Cold Cat")
}

fn default_data_dir() -> PathBuf {
    env::var_os("PROGRAMDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("ColdCat")
}

fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}
