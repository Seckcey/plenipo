#!/usr/bin/env bash
# Plenipo's relay: set it up on a Linux server, once (Phase 14, ADR-149, Plenipo runs its own
# relay; ADR-210, the page and the relay are signed like the installer). Made by 8 West Ventures,
# LLC.
#
# It looks first and writes down what it saw, then adds only Plenipo's own things: a system user,
# /opt/plenipo-relay, /etc/plenipo-relay, the systemd units, and the newest release's program. It
# stops nothing, changes no other service, and touches neither the firewall nor the proxy. 8
# West's server key must already be in /etc/plenipo-relay/trusted-keys.d (README, "the server
# key"): the updater installs nothing it did not sign.
#
# Usage (as root, from a checkout of this repository, or with these files copied next to it):
#   install-relay.sh --check                    look only; change nothing
#   install-relay.sh [--listen ADDR:PORT]       set it up (default 127.0.0.1:8790)
#   install-relay.sh --listen 172.17.0.1:8790   when the proxy runs in a Docker container
#
# The record of what it saw goes to /opt/plenipo-relay/looked-<time>.txt (and the screen).

set -Eeuo pipefail
umask 022

APP_DIR="${APP_DIR:-/opt/plenipo-relay}"
ETC_DIR="${ETC_DIR:-/etc/plenipo-relay}"
UNIT_DIR="${UNIT_DIR:-/etc/systemd/system}"
KEYS_DIR="${TRUSTED_KEYS_DIR:-$ETC_DIR/trusted-keys.d}"
USER_NAME="plenipo-relay"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

check_only=false
listen="127.0.0.1:8790"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) check_only=true ;;
    --listen) listen="${2:?--listen needs ADDRESS:PORT}"; shift ;;
    -h | --help) sed -n '2,16p' "$0"; exit 0 ;;
    *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
fail() { log "STOPPED: $*"; exit 1; }

[[ "$(id -u)" == 0 ]] || fail "Run this as root"
for tool in curl jq sha256sum flock systemctl ss useradd minisign runuser; do
  command -v "$tool" > /dev/null || fail "$tool is not installed (apt install $tool)"
done
for f in plenipo-relay.service plenipo-relay-update.service plenipo-relay-update.timer relay.env.example update-relay.sh; do
  [[ -f "$HERE/$f" ]] || fail "$HERE/$f is missing (copy crates/relay/deploy whole)"
done
port="${listen##*:}"
[[ "$port" =~ ^[0-9]+$ ]] || fail "--listen needs ADDRESS:PORT, not \"$listen\""

# --- Look first ----------------------------------------------------------------------------------
looked="$(mktemp)"
{
  echo "# What install-relay.sh saw on $(hostname) at $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo
  echo "## System"
  uname -a
  echo
  echo "## Memory (MB)"
  free -m
  echo
  echo "## Swap"
  swapon --show || true
  echo
  echo "## Disk"
  df -h / /opt 2> /dev/null || df -h /
  echo
  echo "## Listening ports (TCP)"
  ss -ltnp || ss -ltn
  echo
  echo "## Services already running"
  systemctl list-units --type=service --state=running --no-pager --no-legend || true
  echo
  echo "## Plenipo's relay, if any"
  systemctl list-units 'plenipo-relay*' --all --no-pager --no-legend || true
  id "$USER_NAME" 2> /dev/null || echo "no user $USER_NAME"
  ls -la "$APP_DIR" 2> /dev/null || echo "no $APP_DIR"
  ls -la "$ETC_DIR" 2> /dev/null || echo "no $ETC_DIR"
  echo
  echo "## 8 West's server key (ADR-210): the updater installs nothing it did not sign"
  ls -la "$KEYS_DIR" 2> /dev/null || echo "no $KEYS_DIR"
  if command -v docker > /dev/null && docker ps > /dev/null 2>&1; then
    echo
    echo "## Containers"
    docker ps --format '{{.Names}}\t{{.Image}}\t{{.Ports}}' || true
    echo
    echo "## The proxy (Nginx Proxy Manager): how it reaches this machine"
    for c in $(docker ps -q 2> /dev/null); do
      image="$(docker inspect -f '{{.Config.Image}}' "$c" 2> /dev/null || true)"
      case "$image" in
        *nginx-proxy-manager*)
          echo "container $(docker inspect -f '{{.Name}}' "$c"): network mode $(docker inspect -f '{{.HostConfig.NetworkMode}}' "$c"), extra hosts $(docker inspect -f '{{.HostConfig.ExtraHosts}}' "$c")"
          ;;
      esac
    done
    if ip -4 addr show docker0 > /dev/null 2>&1; then
      echo "this machine on the Docker bridge: $(ip -4 -o addr show docker0 | awk '{print $4}')"
    fi
  fi
} > "$looked" 2>&1 || true
cat "$looked"
echo

# The port must be free.
if ss -ltn | awk '{print $4}' | grep -Eq "(^|:)$port\$"; then
  fail "Port $port is already in use on this machine (see the listening ports above); pick another with --listen"
fi
# 8 West's server key must be there before anything is set up: without it the updater refuses
# every release, so the setup would stop half way.
has_key=false
for key in "$KEYS_DIR"/*.pub; do
  [[ -f "$key" ]] && has_key=true
done
if [[ "$check_only" == true ]]; then
  if [[ "$has_key" == true ]]; then
    log "8 West's server key is in $KEYS_DIR."
  else
    log "No server key in $KEYS_DIR yet: do the README's step \"the server key\" before installing."
  fi
  log "Check only: would add user $USER_NAME, $APP_DIR, $ETC_DIR/relay.env (PLENIPO_RELAY_LISTEN=$listen), the units, and the newest release. Nothing was changed."
  rm -f "${looked:?}"
  exit 0
fi
if [[ "$has_key" != true ]]; then
  rm -f "${looked:?}"
  fail "No server key in $KEYS_DIR: do the README's step \"the server key\" first, then run this again"
fi

# --- Set it up -----------------------------------------------------------------------------------
mkdir -p "$APP_DIR/deploy" "$APP_DIR/releases" "$APP_DIR/state" "$ETC_DIR"
mv "$looked" "$APP_DIR/looked-$(date -u +%Y%m%dT%H%M%SZ).txt"
if ! id "$USER_NAME" > /dev/null 2>&1; then
  useradd --system --home-dir /nonexistent --no-create-home --shell /usr/sbin/nologin "$USER_NAME"
  log "Added the system user $USER_NAME."
fi
cp "$HERE/update-relay.sh" "$APP_DIR/deploy/update-relay.sh"
chmod 0755 "$APP_DIR/deploy/update-relay.sh"
if [[ ! -f "$ETC_DIR/relay.env" ]]; then
  sed "s|^PLENIPO_RELAY_LISTEN=.*|PLENIPO_RELAY_LISTEN=$listen|" "$HERE/relay.env.example" > "$ETC_DIR/relay.env"
  log "Wrote $ETC_DIR/relay.env (PLENIPO_RELAY_LISTEN=$listen)."
else
  log "$ETC_DIR/relay.env is already there; left as it is."
fi
chmod 0644 "$ETC_DIR/relay.env"
chmod 0755 "$ETC_DIR"
for unit in plenipo-relay.service plenipo-relay-update.service plenipo-relay-update.timer; do
  cp "$HERE/$unit" "$UNIT_DIR/$unit"
  chmod 0644 "$UNIT_DIR/$unit"
done
systemctl daemon-reload
# The newest release's program, checked, then the service starts on it.
systemctl enable plenipo-relay.service > /dev/null 2>&1 || true
APP_DIR="$APP_DIR" ENV_FILE="$ETC_DIR/relay.env" "$APP_DIR/deploy/update-relay.sh" --force
systemctl enable --now plenipo-relay-update.timer
log "Done. The relay runs as $USER_NAME on $listen; the timer checks GitHub every 15 minutes and installs only what 8 West's server key signed."
log "Next: the proxy host for relay.getplenipo.com in Nginx Proxy Manager, then the Cloudflare record (README)."
