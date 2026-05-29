# Cold Cat

Cold Cat is a native, cross-platform website and application blocker. The project is Windows-first in this workspace, with Linux support kept behind platform-specific backends for later native Ubuntu validation.

The enforcement model is owner-controlled: Cold Cat resists casual bypass by using a privileged background service, duplicated signed active state, restrictive file permissions, hosts-file hardening, and daemon-side refusal to clear active sessions before expiry. It does not use stealth techniques or claim to defeat a determined administrator/root user with recovery access.

## Language choice

Cold Cat is written in Rust. Rust gives us native compiled binaries for Windows and Linux, strong error handling around privileged filesystem/process operations, and clean `cfg(target_os)` separation for platform enforcement. The shared policy code lives in `coldcat-core`; OS-specific service, hosts, state, and process behavior lives in `coldcat-platform` and `coldcat-daemon`.

## Build

Install Rust stable, then run:

```powershell
cargo build --release
cargo test --workspace
```

The two binaries are:

- `target\release\coldcat.exe`
- `target\release\coldcat-daemon.exe`

## Windows service setup

Run PowerShell as Administrator:

```powershell
cargo build --release
.\packaging\windows\install-service.ps1
```

The service stores privileged state under `%ProgramData%\ColdCat`, keeps a signed backup copy of active session state, listens on `127.0.0.1:47842`, and configures Windows service recovery to restart the daemon if it crashes.

Start a blocking session:

```powershell
.\target\release\coldcat.exe start --duration 2h --apps discord,steam --sites reddit.com,old.reddit.com
```

Check status:

```powershell
.\target\release\coldcat.exe status
```

Run post-expiry cleanup if needed:

```powershell
.\target\release\coldcat.exe recover
```

## Linux notes

Linux support is included for native Ubuntu testing later. Do not validate the Linux backend in virtualization; test it on the separate native Ubuntu device so systemd, `/etc/hosts`, `chattr`, `/proc`, and process signal behavior are representative.

On Ubuntu, build the release binaries and install the systemd unit:

```bash
cargo build --release
sudo install -m 0755 target/release/coldcat /usr/local/bin/coldcat
sudo install -m 0755 target/release/coldcat-daemon /usr/local/bin/coldcat-daemon
sudo install -m 0644 packaging/linux/coldcat.service /etc/systemd/system/coldcat.service
sudo systemctl daemon-reload
sudo systemctl enable --now coldcat.service
```

Linux website blocking writes a managed section to `/etc/hosts` and applies `chattr +i` while a session is active. The daemon removes immutability and restores the managed section after expiry.

## Recovery

Normal `coldcat recover` is refused until the active end timestamp has passed. If administrator/root recovery is required after expiry or because of a failed service restore, stop the service, remove read-only/immutable flags from the managed files, remove only the `# BEGIN COLD-CAT-BLOCK` to `# END COLD-CAT-BLOCK` section from the hosts file, and then restart the service.

This recovery path is intentional. Cold Cat is designed to discourage casual circumvention, not to behave like malware or permanently lock the owner out of their machine.
