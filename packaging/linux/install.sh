#!/usr/bin/env bash
set -euo pipefail

ACTION="install"
PREFIX="/usr/local"
NO_BUILD=0
NO_START=0

usage() {
    cat <<'USAGE'
Cold Cat Linux installer

Usage:
  sudo packaging/linux/install.sh [--install|--update|--repair|--uninstall] [options]

Options:
  --prefix PATH   Install binaries under PATH/bin (default: /usr/local)
  --no-build      Use existing target/release binaries instead of running cargo build
  --no-start      Install files but do not enable/start the systemd service
  -h, --help      Show this help

State under /var/lib/cold-cat and configuration under /etc/cold-cat are preserved
during install, update, repair, and uninstall.
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --install)
            ACTION="install"
            ;;
        --update)
            ACTION="update"
            ;;
        --repair)
            ACTION="repair"
            NO_BUILD=1
            ;;
        --uninstall)
            ACTION="uninstall"
            NO_BUILD=1
            ;;
        --prefix)
            shift
            if [[ $# -eq 0 ]]; then
                echo "--prefix requires a path" >&2
                exit 2
            fi
            PREFIX="$1"
            ;;
        --no-build)
            NO_BUILD=1
            ;;
        --no-start)
            NO_START=1
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

if [[ "${EUID}" -ne 0 ]]; then
    echo "run this installer with sudo/root" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BINDIR="${PREFIX%/}/bin"
SERVICE_PATH="/etc/systemd/system/coldcat.service"
DESKTOP_PATH="/usr/share/applications/coldcat.desktop"
ICON_PATH="/usr/share/icons/hicolor/scalable/apps/coldcat.svg"

build_release() {
    if [[ "${NO_BUILD}" -eq 0 ]]; then
        cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"
    fi
}

install_state_dirs() {
    install -d -m 0700 /var/lib/cold-cat
    install -d -m 0700 /var/lib/cold-cat/state
    install -d -m 0700 /etc/cold-cat
    install -d -m 0700 /run/cold-cat

    chown root:root /var/lib/cold-cat /var/lib/cold-cat/state /etc/cold-cat /run/cold-cat

    for path in \
        /etc/cold-cat/state.key \
        /var/lib/cold-cat/state/session.json.sealed \
        /var/lib/cold-cat/state/session.json.sealed.bak
    do
        if [[ -e "${path}" ]]; then
            local was_immutable=0
            if lsattr "${path}" 2>/dev/null | awk '{print $1}' | grep -q i; then
                chattr -i "${path}"
                was_immutable=1
            fi
            chown root:root "${path}"
            chmod 0600 "${path}"
            if [[ "${was_immutable}" -eq 1 ]]; then
                chattr +i "${path}"
            fi
        fi
    done
}

install_binaries() {
    install -d -m 0755 "${BINDIR}"
    for binary in coldcat coldcat-daemon coldcat-gui; do
        local source="${REPO_ROOT}/target/release/${binary}"
        if [[ ! -x "${source}" ]]; then
            echo "missing release binary: ${source}" >&2
            echo "rerun without --no-build or run cargo build --release first" >&2
            exit 1
        fi
        install -m 0755 "${source}" "${BINDIR}/${binary}"
    done
}

install_service() {
    local tmp_service
    tmp_service="$(mktemp)"
    sed "s#/usr/local/bin#${BINDIR}#g" "${SCRIPT_DIR}/coldcat.service" > "${tmp_service}"
    install -m 0644 "${tmp_service}" "${SERVICE_PATH}"
    rm -f "${tmp_service}"
    systemctl daemon-reload
}

install_desktop_launcher() {
    install -d -m 0755 /usr/share/applications
    install -d -m 0755 /usr/share/icons/hicolor/scalable/apps

    local tmp_desktop
    tmp_desktop="$(mktemp)"
    sed "s#@BINDIR@#${BINDIR}#g" "${SCRIPT_DIR}/coldcat.desktop" > "${tmp_desktop}"
    install -m 0644 "${tmp_desktop}" "${DESKTOP_PATH}"
    rm -f "${tmp_desktop}"

    install -m 0644 "${SCRIPT_DIR}/coldcat.svg" "${ICON_PATH}"

    if command -v update-desktop-database >/dev/null 2>&1; then
        if ! update-desktop-database /usr/share/applications; then
            echo "warning: update-desktop-database failed; launcher files were still installed" >&2
        fi
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        if ! gtk-update-icon-cache -q /usr/share/icons/hicolor; then
            echo "warning: gtk-update-icon-cache failed; icon file was still installed" >&2
        fi
    fi
}

start_service() {
    if [[ "${NO_START}" -eq 1 ]]; then
        return
    fi
    systemctl enable coldcat.service
    if systemctl is-active --quiet coldcat.service; then
        systemctl restart coldcat.service
    else
        systemctl start coldcat.service
    fi
}

wait_for_service() {
    if [[ "${NO_START}" -eq 1 ]]; then
        return
    fi

    for _ in {1..20}; do
        if "${BINDIR}/coldcat" status >/dev/null 2>&1; then
            return
        fi
        sleep 0.25
    done

    systemctl status coldcat.service --no-pager --lines=20 >&2 || true
    echo "coldcat.service started but the daemon did not become reachable on 127.0.0.1:47842" >&2
    exit 1
}

remove_installed_files() {
    if [[ -x "${BINDIR}/coldcat" ]]; then
        local state
        state="$("${BINDIR}/coldcat" status 2>/dev/null | awk '/^state:/ {print $2; exit}' || true)"
        if [[ "${state}" == "Active" ]]; then
            echo "refusing to uninstall while a block is active; wait for expiry and run coldcat recover first" >&2
            exit 1
        fi
    fi

    if systemctl list-unit-files coldcat.service >/dev/null 2>&1; then
        if systemctl is-active --quiet coldcat.service; then
            systemctl stop coldcat.service
        fi
        systemctl disable coldcat.service >/dev/null 2>&1 || true
    fi

    rm -f "${SERVICE_PATH}" "${DESKTOP_PATH}" "${ICON_PATH}"
    rm -f "${BINDIR}/coldcat" "${BINDIR}/coldcat-daemon" "${BINDIR}/coldcat-gui"
    systemctl daemon-reload
}

case "${ACTION}" in
    install|update)
        build_release
        install_state_dirs
        install_binaries
        install_service
        install_desktop_launcher
        start_service
        wait_for_service
        echo "Cold Cat ${ACTION} complete."
        ;;
    repair)
        install_state_dirs
        if [[ -x "${REPO_ROOT}/target/release/coldcat" ]]; then
            install_binaries
        fi
        install_service
        install_desktop_launcher
        start_service
        wait_for_service
        echo "Cold Cat repair complete."
        ;;
    uninstall)
        remove_installed_files
        echo "Cold Cat uninstall complete. Preserved /var/lib/cold-cat and /etc/cold-cat."
        ;;
    *)
        echo "unknown action: ${ACTION}" >&2
        exit 2
        ;;
esac
