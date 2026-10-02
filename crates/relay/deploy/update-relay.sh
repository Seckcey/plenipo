#!/usr/bin/env bash
# Plenipo's relay: install the newest release's relay program and switch to it (Phase 14,
# ADR-149, Plenipo runs its own relay). Made by 8 West Ventures, LLC.
#
# Runs from a systemd timer (plenipo-relay-update.timer), every 15 minutes, as root (it restarts a
# system service). Each run:
#   1. asks GitHub for the latest published release (not a draft or pre-release);
#   2. if the relay running now is that version and healthy, stops there;
#   3. downloads the release's plenipo-relay-<version>-linux-x86_64 and its .sha256, checks the
#      checksum, checks the program says that version and carries no test hooks, and puts it in a
#      new, never-edited release folder;
#   4. points `current` at it (one atomic switch) and restarts the relay (a second or two: PCs
#      reconnect by themselves, phones reconnect when opened);
#   5. checks the relay is running and /healthz says ok; if not, points back at the previous
#      release, restarts, and checks again.
# It touches only Plenipo's relay: never another service, the firewall, DNS, or the proxy.
#
# Usage: update-relay.sh [--check] [--force] [--version X.Y.Z]
#   --check     say what would happen; change nothing
#   --force     install and switch even if the relay already runs that version
#   --version   install this published release instead of the latest (e.g. to go back)
#
# Settings come from the environment, or /etc/plenipo-relay/relay.env (PLENIPO_RELAY_LISTEN gives
# the port for the health check).

set -Eeuo pipefail
umask 022

APP_DIR="${APP_DIR:-/opt/plenipo-relay}"
ENV_FILE="${ENV_FILE:-/etc/plenipo-relay/relay.env}"
if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  source "$ENV_FILE"
fi
REPO_API="${REPO_API:-https://api.github.com/repos/Seckcey/plenipo}"
DOWNLOADS="${DOWNLOADS:-https://github.com/Seckcey/plenipo/releases/download}"
SERVICE="${SERVICE:-plenipo-relay}"
LISTEN="${PLENIPO_RELAY_LISTEN:-127.0.0.1:8790}"
RELEASES_DIR="$APP_DIR/releases"
STATE_DIR="$APP_DIR/state"
VERSION_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+$'
# The line a copy built for the tests prints: a release never carries it.
TEST_HOOKS_MARK='the relay listens on'

check_only=false
force=false
wanted_version=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) check_only=true ;;
    --force) force=true ;;
    --version) wanted_version="${2:?--version needs X.Y.Z}"; shift ;;
    -h | --help) sed -n '2,26p' "$0"; exit 0 ;;
    *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
fail() { log "STOPPED: $*"; exit 1; }

if [[ "$(id -u)" != 0 ]]; then
  fail "Run this as root: it installs under $APP_DIR and restarts the $SERVICE service"
fi
for tool in curl jq sha256sum flock systemctl; do
  command -v "$tool" > /dev/null || fail "$tool is not installed (apt install $tool)"
done
[[ -d "$APP_DIR" ]] || fail "$APP_DIR does not exist (run install-relay.sh first)"
mkdir -p "$RELEASES_DIR" "$STATE_DIR"

# One run at a time.
exec 8> "$STATE_DIR/run.lock"
if ! flock -n 8; then
  log "Another update is still running; leaving it to finish."
  exit 0
fi

health_url="http://$LISTEN/healthz"
healthy() { curl -fsS --max-time 5 "$health_url" 2> /dev/null | grep -qx ok; }

# --- 1. The release to install -----------------------------------------------------------------
if [[ -n "$wanted_version" ]]; then
  release_url="$REPO_API/releases/tags/v${wanted_version#v}"
else
  release_url="$REPO_API/releases/latest"
fi
release_json="$(curl -fsSL --retry 3 --max-time 30 -H 'Accept: application/vnd.github+json' "$release_url")" \
  || fail "GitHub did not answer for $release_url"
tag="$(jq -r '.tag_name // empty' <<< "$release_json")"
version="${tag#v}"
[[ "$version" =~ $VERSION_PATTERN ]] || fail "The release tag \"$tag\" is not a full release version"
[[ "$(jq -r '.draft' <<< "$release_json")" == false ]] || fail "$tag is a draft"
[[ "$(jq -r '.prerelease' <<< "$release_json")" == false ]] || fail "$tag is a pre-release; the relay runs full releases only"
program="plenipo-relay-$version-linux-x86_64"
for asset in "$program" "$program.sha256"; do
  jq -e --arg name "$asset" '.assets | any(.name == $name and .state == "uploaded")' <<< "$release_json" > /dev/null \
    || fail "$tag has no $asset attached (releases before 1.19.3 carry no relay)"
done

# --- 2. What runs now --------------------------------------------------------------------------
current=""
if [[ -L "$APP_DIR/current" ]]; then
  current="$(basename "$(readlink "$APP_DIR/current")")"
fi
running=false
systemctl is-active --quiet "$SERVICE" && running=true
log "Latest release: $tag. Installed now: ${current:-nothing} (service $([[ $running == true ]] && echo running || echo not running))."
if [[ "$current" == "$version" && "$force" == false && "$running" == true ]] && healthy; then
  log "Up to date and healthy. Nothing to do."
  exit 0
fi
if [[ "$check_only" == true ]]; then
  log "Check only: would install $tag to $RELEASES_DIR/$version and restart $SERVICE. Nothing was changed."
  exit 0
fi

# --- 3. Download and check ---------------------------------------------------------------------
release_dir="$RELEASES_DIR/$version"
if [[ ! -x "$release_dir/plenipo-relay" ]]; then
  staging="$(mktemp -d "$RELEASES_DIR/.staging.XXXXXX")"
  trap 'rm -rf "${staging:?}"' EXIT
  curl -fsSL --retry 3 --max-time 300 -o "$staging/plenipo-relay" "$DOWNLOADS/$tag/$program" \
    || fail "Could not download $program"
  curl -fsSL --retry 3 --max-time 30 -o "$staging/checksum.sha256" "$DOWNLOADS/$tag/$program.sha256" \
    || fail "Could not download $program.sha256"
  want="$(cut -d' ' -f1 < "$staging/checksum.sha256")"
  got="$(sha256sum "$staging/plenipo-relay" | cut -d' ' -f1)"
  [[ -n "$want" && "$want" == "$got" ]] || fail "$program does not match its checksum"
  chmod 0755 "$staging/plenipo-relay"
  says="$("$staging/plenipo-relay" --version 2> /dev/null || true)"
  [[ "$says" == "plenipo-relay $version" ]] || fail "The program says \"$says\", not \"plenipo-relay $version\""
  if grep -q "$TEST_HOOKS_MARK" "$staging/plenipo-relay"; then
    fail "$program was built with its test hooks; a release never is"
  fi
  mv "$staging" "$release_dir"
  chmod 0755 "$release_dir"
  trap - EXIT
fi

# --- 4. Switch and restart ---------------------------------------------------------------------
previous="$current"
point_at() { # point_at <version>: one atomic switch of the program the service starts
  ln -sfn "releases/$1" "$APP_DIR/current.next"
  mv -Tf "$APP_DIR/current.next" "$APP_DIR/current"
}
restart() {
  systemctl restart "$SERVICE"
}
point_at "$version"
restart || log "systemctl restart $SERVICE failed"

# --- 5. Check, and go back if anything is wrong ------------------------------------------------
verify() { # verify: prints what failed
  local tries
  for tries in $(seq 1 20); do
    if systemctl is-active --quiet "$SERVICE" && healthy; then
      return 0
    fi
    sleep 0.5
  done
  if ! systemctl is-active --quiet "$SERVICE"; then
    echo "the service is not running ($(systemctl is-active "$SERVICE" || true))"
  else
    echo "$health_url did not say ok"
  fi
  return 1
}

if problem="$(verify)"; then
  printf 'VERSION=%s\nINSTALLED_AT=%s\n' "$version" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$STATE_DIR/current.env"
  log "Running $tag."
  exit 0
fi
log "The check of $tag failed: $problem"
journalctl -u "$SERVICE" -n 20 --no-pager 2> /dev/null || true
if [[ -n "$previous" && -x "$RELEASES_DIR/$previous/plenipo-relay" ]]; then
  point_at "$previous"
  restart || true
  if again="$(verify)"; then
    fail "Went back to $previous, which passes its checks"
  fi
  fail "Went back to $previous, which also fails: $again"
fi
fail "Nothing earlier to go back to"
