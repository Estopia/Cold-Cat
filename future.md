# Linux AI handoff

This file is for the future AI agent running on native Ubuntu/Linux. Your job is to test the Linux backend on the real machine, finish missing Linux-specific development, and leave the project in a releasable Linux state.

Do not treat Windows behavior as proof that Linux works. The Linux blocker must be validated against real `systemd`, `/etc/hosts`, `chattr`, process signals, Snap/Flatpak layouts, and desktop launcher behavior.

## Ground rules

1. Run on native Ubuntu hardware, not a VM, unless you are only doing non-privileged compile/test work.
2. Do not leave `/etc/hosts` immutable after testing. Always restore it with `sudo chattr -i /etc/hosts` if anything fails.
3. Preserve the managed hosts section markers and only edit/remove Cold Cat's own section.
4. Do not weaken the anti-circumvention model just to make tests easier.
5. Commit small, verified Linux changes with clear messages.

## First checks

From the repository root:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release
```

Then install the current Linux service manually:

```bash
sudo install -m 0755 target/release/coldcat /usr/local/bin/coldcat
sudo install -m 0755 target/release/coldcat-daemon /usr/local/bin/coldcat-daemon
sudo install -m 0755 target/release/coldcat-gui /usr/local/bin/coldcat-gui
sudo install -m 0644 packaging/linux/coldcat.service /etc/systemd/system/coldcat.service
sudo systemctl daemon-reload
sudo systemctl enable --now coldcat.service
systemctl status coldcat.service --no-pager
```

## Must-test Linux behavior

1. Service lifecycle
   - `sudo systemctl start coldcat.service`
   - `sudo systemctl restart coldcat.service`
   - Kill the daemon process during an active block and confirm systemd restarts it.
   - Reboot during an active block and confirm the session resumes.

2. Website blocking
   - Start a short block with sites like `reddit.com`, `old.reddit.com`, and `youtube.com`.
   - Confirm the Cold Cat managed section appears in `/etc/hosts`.
   - Confirm `/etc/hosts` becomes immutable during the block.
   - Confirm manual edits fail while the block is active.
   - Confirm expiry/recovery removes the managed section and clears immutability.

3. State hardening
   - Confirm state files are under the Linux data path and owned by root.
   - Confirm a normal user cannot edit sealed session state.
   - Confirm daemon restart does not lose active sessions.
   - Add a Linux permission repair command/path if state ownership or mode breaks.

4. App blocking
   - Test raw process-name rules against common apps.
   - Test GUI-launched apps, terminal-launched apps, AppImages, Snap apps, Flatpak apps, Steam, and browser child processes.
   - Decide per app type whether Cold Cat should kill processes, chmod launch targets, or both.
   - Implement missing rule expansion so the GUI/CLI can block Linux apps by friendly app name, desktop file, binary path, Snap name, and Flatpak app ID.

5. GUI on Linux
   - Launch `coldcat-gui` from terminal and from a desktop launcher.
   - Verify fonts, layout, scrolling, button hitboxes, vector icons, app discovery, profile saving, daemon status, and error messages.
   - Remove or replace Windows-specific wording on Linux where it appears.

## Development tasks to finish

1. Linux installer/updater
   - Add a one-command Linux installer script or `.deb` package.
   - Install binaries, systemd unit, GUI desktop file, icon, and uninstall/update paths.
   - Make updates preserve active state and config.
   - Add a repair mode for permissions and service setup.

2. systemd hardening
   - Review `packaging/linux/coldcat.service`.
   - Add hardening options where possible without blocking required writes to `/etc/hosts`, state files, runtime files, or process enforcement.
   - Confirm `Restart=always` or equivalent restart behavior is correct during active blocks.

3. Linux app discovery
   - Parse `.desktop` files from `/usr/share/applications` and `~/.local/share/applications`.
   - Add Snap and Flatpak discovery.
   - Normalize discovered apps into rules the daemon can enforce reliably.

4. Linux enforcement improvements
   - Improve process matching to avoid missing child processes or alternate executable names.
   - Add safe chmod support only where it is reversible and does not break package-managed binaries permanently.
   - Add tests around rule normalization and state recovery.

5. Documentation
   - Update `README.md` with Linux install/update/uninstall steps.
   - Add a Linux troubleshooting section for `chattr`, immutable hosts files, systemd failures, Snap/Flatpak limitations, and recovery.
   - Document the manual native-Ubuntu test checklist.

## Acceptance checklist

The Linux work is not done until:

1. Build, tests, clippy, and release build pass on Linux.
2. A real active block survives daemon restart and reboot.
3. `/etc/hosts` is protected during blocks and restored after recovery/expiry.
4. Normal users cannot edit active sealed state.
5. At least one regular app, one Snap or Flatpak app, and one browser/domain workflow are successfully blocked.
6. GUI start/status/profile/app-selection workflows work on Ubuntu.
7. Linux install/update/uninstall instructions are documented and tested.

## Product backlog

1. Ubuntu `.deb` installer for single-package installation without source builds.
2. GitHub Releases packaging for downloadable signed/versioned builds.
3. First-run setup wizard for daemon installation, repair, and status checks.
4. Schedules or recurring blocks for planned focus windows.
5. Import/export profiles for sharing and backing up saved block profiles.
6. Better Linux app-blocking presets for common browsers, games, chat apps, and launchers.
7. Block history or audit log for recent sessions and recovery events.
8. Optional browser helper extension for stronger browser-level blocking.
