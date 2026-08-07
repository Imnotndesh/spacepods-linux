#!/usr/bin/env sh
#
# SpacePods Linux installer
# -------------------------
# Installs / removes libspacepods (daemon) and spacepods-ui (GUI).
#
# Usage:
#   ./install.sh [action] [target ...]
#
# Actions:
#   install   (default) Install selected targets
#   upgrade             Re-download and overwrite selected targets
#   remove / uninstall  Remove selected targets
#   help                Show this help
#
# Targets:
#   all          Everything (default)
#   libspacepods The libspacepods daemon + CLI (installed to /usr/local/bin)
#   spacepods-ui The SpacePods GUI binary + desktop entry + icon
#   flatpak      The SpacePods GUI via the Flatpak bundle
#   service      Install/remove the systemd USER unit for the daemon
#                (logs to journald; enable with: systemctl --user enable --now spacepods)
#
# Examples:
#   ./install.sh install all
#   ./install.sh install service --enable     # install + enable at login
#   ./install.sh --no-flatpak install all service
#   ./install.sh remove flatpak
#   ./install.sh remove service
#   ./install.sh upgrade libspacepods
#
# Options:
#   --no-flatpak     Skip the flatpak install even when 'all' is selected
#   --prefix=PATH    Install binaries under PATH instead of /usr/local
#   --no-sudo        Do not use sudo (assume write access to prefix)
#   --enable         With 'install service': run 'systemctl --user enable --now'
#   --no-enable      With 'remove service': do NOT disable/stop before removing
#   --log-level=LVL  Log verbosity for the unit (info|warn|full, default: warn)
#                    'warn' is minimal output; journald captures everything.
#
# Note for private repositories:
#   If the GitHub release is not publicly downloadable, this script will fail
#   with a clear error instead of piping a "Not Found" body to a shell.
#   Authenticated clients must fetch assets through the GitHub API with a token.

set -eu
umask 022

# ----- Configuration -----
GH_REPO="Imnotndesh/spacepods-linux"
# Allow overriding the download base for testing / self-hosted mirrors.
BASE_URL="${SPACEPODS_BASE_URL:-https://github.com/${GH_REPO}/releases/latest/download}"
PREFIX="/usr/local"
USE_SUDO=1
DO_FLATPAK=1
DO_ENABLE=0
DO_DISABLE=1
LOG_LEVEL="warn"

ACTION="install"
TARGETS=""

# ----- Parse arguments -----
for arg in "$@"; do
  case "$arg" in
    install)     ACTION="install" ;;
    upgrade)     ACTION="upgrade" ;;
    remove|uninstall) ACTION="remove" ;;
    all|libspacepods|spacepods-ui|flatpak|service) TARGETS="${TARGETS} $arg" ;;
    --no-flatpak) DO_FLATPAK=0 ;;
    --no-sudo)   USE_SUDO=0 ;;
    --prefix=*)  PREFIX="${arg#--prefix=}" ;;
    --enable)    DO_ENABLE=1 ;;
    --no-enable) DO_DISABLE=0 ;;
    --log-level=*) LOG_LEVEL="${arg#--log-level=}" ;;
    -h|--help|help) ACTION="help" ;;
    *)
      printf '%s\n' "Unknown argument: $arg" >&2
      printf '%s\n' "Run '$0 help' for usage." >&2
      exit 2 ;;
  esac
done

# ----- Defaults -----
[ -z "$TARGETS" ] && TARGETS=" all"

# Validate --log-level
case "$LOG_LEVEL" in
  info|warn|full) : ;;
  *) printf '%s\n' "Invalid --log-level: $LOG_LEVEL (must be info|warn|full)" >&2; exit 2 ;;
esac

sudo_cmd() {
  if [ "$USE_SUDO" = "1" ]; then
    if [ "$(id -u)" -ne 0 ]; then
      if command -v sudo >/dev/null 2>&1; then sudo "$@"; else "$@"; fi
    else
      "$@"
    fi
  else
    "$@"
  fi
}

log() { printf '%b\n' "\033[1;34m[SpacePods]\033[0m $*"; }
warn() { printf '%b\n' "\033[1;33m[SpacePods]\033[0m warning: $*" >&2; }
die()  { printf '%b\n' "\033[1;31m[SpacePods]\033[0m error: $*" >&2; exit 1; }

# Strict download that fails loudly instead of piping an error page to a shell.
fetch() { # $1 = url, $2 = output file
  if ! command -v curl >/dev/null 2>&1; then
    die "curl is required to download from GitHub. Install curl first."
  fi
  curl -fSL --retry 3 -o "$2" "$1" 2>/dev/null || return 1
  # Guard: some servers return an error page with HTTP 200; reject it.
  if [ -s "$2" ] && head -c 128 "$2" | grep -qiE "not found|<html|<!doctype|no such file"; then
    rm -f "$2"
    return 1
  fi
  return 0
}

banner() {
  cat <<'EOF'

  SpacePods Linux - Control your SpaceBuds
EOF
}

show_help() {
  banner
  echo
  sed -n '2,39p' "$0" | sed -n 's/^#//p'
  cat <<'EOF'

Important:
  If your shell prints "command not found", the download returned an error
  page (e.g. GitHub blocked an anonymous/private download). Always download
  the script to a file first and inspect it, never pipe directly into sh:

      curl -fSL -O \
        https://github.com/Imnotndesh/spacepods-linux/releases/latest/download/install.sh
      less install.sh
      sh install.sh install all
EOF
}

# ---------------------------------------------------------------------------
# libspacepods
# ---------------------------------------------------------------------------
install_libspacepods() {
  echo
  banner
  log "Installing libspacepods (daemon + CLI) -> $PREFIX/bin"
  command -v tar >/dev/null 2>&1 || die "tar is required."
  TMP_DIR="$(mktemp -d)"
  trap 'rm -rf "$TMP_DIR"' EXIT
  URL="$BASE_URL/libspacepods-x86_64.tar.gz"
  log "Downloading $URL"
  if ! fetch "$URL" "$TMP_DIR/lib.tar.gz"; then
    die "Failed to download libspacepods. URL: $URL"
  fi
  sudo_cmd mkdir -p "$PREFIX/bin"
  sudo_cmd tar -xzf "$TMP_DIR/lib.tar.gz" -C "$PREFIX/bin"
  sudo_cmd chmod +x "$PREFIX/bin/libspacepods" 2>/dev/null || true
  log "Installed. Start the daemon with:  $PREFIX/bin/libspacepods service"
  trap - EXIT
  rm -rf "$TMP_DIR"
}

remove_libspacepods() {
  echo
  banner
  log "Removing libspacepods"
  sudo_cmd rm -f "$PREFIX/bin/libspacepods"
  log "libspacepods removed."
}

# ---------------------------------------------------------------------------
# systemd USER unit for the daemon (logs to journald)
# ---------------------------------------------------------------------------
# Uses a per-user unit so the user can run:
#   systemctl --user enable --now spacepods   # start at login + now
#   systemctl --user status spacepods
#   journalctl --user -u spacepods -f
SERVICE_UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
SERVICE_UNIT_FILE="$SERVICE_UNIT_DIR/spacepods.service"

# The unit lives in $HOME so it is managed WITHOUT sudo (per-user systemd).
# The binary itself lives under $PREFIX (which may need sudo to install).
write_service_unit() {
  mkdir -p "$SERVICE_UNIT_DIR"
  # Use a heredoc that does NOT expand $PREFIX/$LOG_LEVEL so the unit gets
  # literal placeholders we can substitute safely via sed (paths may contain
  # characters that would confuse replacement).
  cat > "$SERVICE_UNIT_DIR/spacepods.service.in" <<'UNITEOF'
[Unit]
Description=SpacePods daemon (libspacepods)
After=bluetooth.target
Wants=bluetooth.target

[Service]
Type=simple
ExecStart=@PREFIX@/bin/libspacepods service --log-level @LOG_LEVEL@
Restart=on-failure
RestartSec=3
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=default.target
UNITEOF
  sed -e "s|@PREFIX@|$PREFIX|g" -e "s|@LOG_LEVEL@|$LOG_LEVEL|g" \
    "$SERVICE_UNIT_DIR/spacepods.service.in" > "$SERVICE_UNIT_FILE"
  rm -f "$SERVICE_UNIT_DIR/spacepods.service.in"
  systemctl --user daemon-reload 2>/dev/null || true
}

install_service() {
  echo
  banner
  log "Installing systemd user unit for libspacepods"
  command -v systemctl >/dev/null 2>&1 || die "systemd (systemctl) is required for the service target."
  if [ ! -x "$PREFIX/bin/libspacepods" ]; then
    warn "libspacepods is not installed yet; installing it first."
    install_libspacepods
  fi
  write_service_unit
  log "Unit written to $SERVICE_UNIT_FILE"
  log "Log level: $LOG_LEVEL (logs go to journald)"
  if [ "$DO_ENABLE" = "1" ]; then
    systemctl --user enable --now spacepods
    log "Enabled and started. Verify with: systemctl --user status spacepods"
    log "Follow logs with:           journalctl --user -u spacepods -f"
  else
    log "Unit installed but not enabled. To start at login and now, run:"
    log "   systemctl --user enable --now spacepods"
  fi
}

remove_service() {
  echo
  banner
  log "Removing systemd user unit for libspacepods"
  if [ "$DO_DISABLE" = "1" ] && command -v systemctl >/dev/null 2>&1; then
    systemctl --user disable --now spacepods 2>/dev/null || true
  fi
  rm -f "$SERVICE_UNIT_FILE"
  if command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload 2>/dev/null || true
  fi
  log "unit removed."
}

# ---------------------------------------------------------------------------
# spacepods-ui (GUI binary with desktop integration)
# ---------------------------------------------------------------------------
install_spacepods_ui() {
  echo
  banner
  log "Installing spacepods-ui (GUI) -> $PREFIX/bin, $PREFIX/share"
  command -v tar >/dev/null 2>&1 || die "tar is required."
  TMP_DIR="$(mktemp -d)"
  trap 'rm -rf "$TMP_DIR"' EXIT
  URL="$BASE_URL/spacepods-ui-x86_64.tar.gz"
  log "Downloading $URL"
  if ! fetch "$URL" "$TMP_DIR/ui.tar.gz"; then
    die "Failed to download spacepods-ui. URL: $URL"
  fi
  sudo_cmd mkdir -p "$PREFIX/bin" \
    "$PREFIX/share/applications" \
    "$PREFIX/share/icons/hicolor/scalable/apps"
  sudo_cmd tar -xzf "$TMP_DIR/ui.tar.gz" -C "$TMP_DIR"
  # Locate and install the GUI binary.
  if [ -x "$TMP_DIR/spacepods-ui" ]; then
    sudo_cmd install -m755 "$TMP_DIR/spacepods-ui" "$PREFIX/bin/spacepods-ui"
  else
    FOUND=0
    for f in "$TMP_DIR"/*; do
      if [ -f "$f" ]; then
        sudo_cmd install -m755 "$f" "$PREFIX/bin/$(basename "$f")"
        FOUND=1
      fi
    done
    [ "$FOUND" = "1" ] || die "No binary found inside the downloaded tarball."
  fi
  # Desktop entry + icon so the GUI shows up in launchers.
  cat > "$TMP_DIR/com.spacepods.ui.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=SpacePods Linux
Comment=Control your SpaceBuds earbuds
Exec=spacepods-ui
Icon=com.spacepods.ui
Categories=Audio;Settings;HardwareSettings;
Terminal=false
StartupNotify=true
EOF
  cat > "$TMP_DIR/com.spacepods.ui.svg" <<'EOF'
<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256"><rect width="256" height="256" rx="48" fill="#1f57c4"/><text x="128" y="150" font-family="sans-serif" font-size="120" font-weight="bold" text-anchor="middle" fill="#fff">S</text></svg>
EOF
  sudo_cmd install -m644 "$TMP_DIR/com.spacepods.ui.desktop" "$PREFIX/share/applications/com.spacepods.ui.desktop"
  sudo_cmd install -m644 "$TMP_DIR/com.spacepods.ui.svg" "$PREFIX/share/icons/hicolor/scalable/apps/com.spacepods.ui.svg"
  sudo_cmd update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
  sudo_cmd gtk-update-icon-cache "$PREFIX/share/icons/hicolor" 2>/dev/null || true
  log "Installed. Launch it with:  spacepods-ui"
  trap - EXIT
  rm -rf "$TMP_DIR"
}

remove_spacepods_ui() {
  echo
  banner
  log "Removing spacepods-ui"
  sudo_cmd rm -f "$PREFIX/bin/spacepods-ui"
  sudo_cmd rm -f "$PREFIX/share/applications/com.spacepods.ui.desktop"
  sudo_cmd rm -f "$PREFIX/share/icons/hicolor/scalable/apps/com.spacepods.ui.svg"
  sudo_cmd update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
  log "spacepods-ui removed."
}

# ---------------------------------------------------------------------------
# Flatpak GUI
# ---------------------------------------------------------------------------
install_flatpak() {
  echo
  banner
  log "Installing SpacePods GUI via Flatpak"
  command -v flatpak >/dev/null 2>&1 || die "flatpak is required. Install flatpak first."
  TMP_DIR="$(mktemp -d)"
  trap 'rm -rf "$TMP_DIR"' EXIT
  URL="$BASE_URL/spacepods.flatpak"
  log "Downloading $URL"
  if ! fetch "$URL" "$TMP_DIR/spacepods.flatpak"; then
    die "Failed to download flatpak bundle. URL: $URL"
  fi
  flatpak install --user -y "$TMP_DIR/spacepods.flatpak"
  log "Installed. Launch it with:  flatpak run com.spacepods.ui"
  trap - EXIT
  rm -rf "$TMP_DIR"
}

remove_flatpak() {
  echo
  banner
  log "Removing SpacePods Flatpak app"
  if ! command -v flatpak >/dev/null 2>&1; then
    warn "flatpak not installed; nothing to remove."
    return 0
  fi
  flatpak uninstall --user -y com.spacepods.ui 2>/dev/null || true
  log "Flatpak app removed."
}

# ----- Help early-exit -----
if [ "$ACTION" = "help" ]; then
  show_help
  exit 0
fi

echo "==> Action: $ACTION   Targets:$TARGETS"

# ----- Run ------
for t in $TARGETS; do
  case "$t" in
    all)
      if [ "$ACTION" = "remove" ]; then
        remove_service
        remove_spacepods_ui
        remove_flatpak
        remove_libspacepods
      else
        install_libspacepods
        install_service
        if [ "$DO_FLATPAK" = "1" ]; then
          install_flatpak
        else
          install_spacepods_ui
        fi
      fi
      ;;
    libspacepods)
      if [ "$ACTION" = "remove" ]; then remove_libspacepods; else install_libspacepods; fi
      ;;
    service)
      if [ "$ACTION" = "remove" ]; then remove_service; else install_service; fi
      ;;
    spacepods-ui)
      if [ "$ACTION" = "remove" ]; then remove_spacepods_ui; else install_spacepods_ui; fi
      ;;
    flatpak)
      if [ "$ACTION" = "remove" ]; then remove_flatpak; else install_flatpak; fi
      ;;
  esac
done

echo
log "Done."
