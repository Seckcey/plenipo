#!/usr/bin/env bash
# Plenipo's relay: install the newest release's relay program and switch to it (Phase 14,
# ADR-149, Plenipo runs its own relay; ADR-210, the page and the relay are signed like the
# installer). Made by 8 West Ventures, LLC.
#
# Runs from a systemd timer (plenipo-relay-update.timer), every 15 minutes, as root (it restarts a
# system service). Each run:
#   1. asks GitHub for the latest published release (not a draft or pre-release), and refuses one
#      older than the release installed (a step back is taken only by hand, with --version);
#   2. if the relay running now is that version and healthy, stops there;
#   3. downloads the release's plenipo-relay-<version>-linux-x86_64, its .sig, and its .sha256,
#      checks the signature first (made with 8 West's server key, over this very file, for this
#      version; the trusted keys live in /etc/plenipo-relay/trusted-keys.d), then the checksum,
#      then, as the relay's own unprivileged user and never as root, that the program says that
#      version and carries no test hooks, and puts it in a new, never-edited release folder;
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
# The relay's own user (install-relay.sh adds it): the downloaded program runs as it, never as root.
SERVICE_USER="${SERVICE_USER:-plenipo-relay}"
# 8 West's server key, its public half, installed once by hand (README): every *.pub in here, as
# `tauri signer generate` prints it (one base64 line) or as a minisign public key file. Root owns
# the folder and the files, and nobody else may write them.
TRUSTED_KEYS_DIR="${TRUSTED_KEYS_DIR:-/etc/plenipo-relay/trusted-keys.d}"
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
    -h | --help) sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
fail() { log "STOPPED: $*"; exit 1; }

if [[ "$(id -u)" != 0 ]]; then
  fail "Run this as root: it installs under $APP_DIR and restarts the $SERVICE service"
fi
for tool in curl jq sha256sum flock systemctl minisign base64 runuser timeout; do
  command -v "$tool" > /dev/null || fail "$tool is not installed (apt install $tool)"
done
[[ -d "$APP_DIR" ]] || fail "$APP_DIR does not exist (run install-relay.sh first)"
id "$SERVICE_USER" > /dev/null 2>&1 || fail "There is no user $SERVICE_USER (run install-relay.sh first)"
mkdir -p "$RELEASES_DIR" "$STATE_DIR"

# One run at a time.
exec 8> "$STATE_DIR/run.lock"
if ! flock -n 8; then
  log "Another update is still running; leaving it to finish."
  exit 0
fi

health_url="http://$LISTEN/healthz"
healthy() { curl -fsS --max-time 5 "$health_url" 2> /dev/null | grep -qx ok; }

# --- The trusted keys ------------------------------------------------------------------------
# Only root may write the folder and the keys: a key anyone else could add would sign anything.
root_only() { # root_only <path>: owned by root, and no write bit for the group or others
  [[ "$(stat -c '%U %a' "$1")" =~ ^root\ [0-7]?[0-7][0145][0145]$ ]]
}
[[ -d "$TRUSTED_KEYS_DIR" ]] \
  || fail "$TRUSTED_KEYS_DIR does not exist: install 8 West's server key first (README, \"the server key\")"
root_only "$TRUSTED_KEYS_DIR" \
  || fail "$TRUSTED_KEYS_DIR must be owned by root and writable by root only (chown root:root; chmod 0755)"
trusted_keys=()
for key in "$TRUSTED_KEYS_DIR"/*.pub; do
  [[ -f "$key" ]] || continue
  root_only "$key" || fail "$key must be owned by root and writable by root only (chown root:root; chmod 0644)"
  trusted_keys+=("$key")
done
[[ ${#trusted_keys[@]} -gt 0 ]] \
  || fail "No *.pub in $TRUSTED_KEYS_DIR: install 8 West's server key first (README, \"the server key\")"

# verify_signature <file> <signature file> <asset name> <version>: the signature was made with one
# of the trusted keys, over this file, and (the signature covers its own trusted comment) names
# this asset and this version. Anything else stops the update before the file is used at all.
verify_signature() {
  local file="$1" sig_b64="$2" name="$3" version="$4"
  local work raw_sig key pub comment signed_file signed_version signed_by=""
  work="$(mktemp -d)"
  raw_sig="$work/signature"
  # The release carries the signature as Tauri's tool writes it: one base64 line of a minisign
  # signature file. (Spaces and line breaks around it, or in a pasted key, are dropped.)
  tr -d '[:space:]' < "$sig_b64" | base64 -d > "$raw_sig" 2> /dev/null && grep -q '^trusted comment: ' "$raw_sig" \
    || { rm -rf "$work"; fail "$name.sig is not a signature file"; }
  for key in "${trusted_keys[@]}"; do
    if grep -q '^untrusted comment: ' "$key"; then
      pub="$key"
    else
      pub="$work/key.pub"
      tr -d '[:space:]' < "$key" | base64 -d > "$pub" 2> /dev/null && grep -q '^untrusted comment: ' "$pub" \
        || { rm -rf "$work"; fail "$key is not a public key"; }
    fi
    if minisign -Vq -m "$file" -x "$raw_sig" -p "$pub" > /dev/null 2>&1; then
      signed_by="$key"
      break
    fi
  done
  [[ -n "$signed_by" ]] || { rm -rf "$work"; fail "$name is not signed with 8 West's server key (no key in $TRUSTED_KEYS_DIR accepts its signature)"; }
  # Only now is the trusted comment known to be 8 West's: the signature covers it.
  comment="$(sed -n 's/^trusted comment: //p' "$raw_sig" | head -n 1)"
  signed_file="$(tr '\t' '\n' <<< "$comment" | sed -n 's/^file://p' | head -n 1)"
  signed_version="$(tr '\t' '\n' <<< "$comment" | sed -n 's/^version://p' | head -n 1)"
  rm -rf "$work"
  [[ "$signed_file" == "$name" ]] \
    || fail "$name's signature was made for \"${signed_file:-no file}\", not for $name"
  [[ "${signed_version#v}" == "$version" ]] \
    || fail "$name was signed as version \"${signed_version:-none}\", but the release says $version"
  log "$name is signed with 8 West's server key ($(basename "$signed_by")) for $version."
}

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
for asset in "$program" "$program.sig" "$program.sha256"; do
  jq -e --arg name "$asset" '.assets | any(.name == $name and .state == "uploaded")' <<< "$release_json" > /dev/null \
    || fail "$tag has no $asset attached (releases before 1.19.3 carry no relay; the .sig came later, ADR-210)"
done
# Never a step back on its own: an older "latest" (a deleted release, or a wrong answer) is
# refused. The operator goes back with --version.
installed_version=""
if [[ -f "$STATE_DIR/current.env" ]]; then
  installed_version="$(sed -n 's/^VERSION=//p' "$STATE_DIR/current.env" | head -n 1)"
fi
older_than() { # older_than <a> <b>: version a is lower than version b
  [[ "$1" != "$2" && "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n 1)" == "$1" ]]
}
if [[ -z "$wanted_version" && "$installed_version" =~ $VERSION_PATTERN ]] && older_than "$version" "$installed_version"; then
  fail "GitHub's latest release is $tag, older than the $installed_version installed; to go back on purpose, run this with --version $version"
fi

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
  curl -fsSL --retry 3 --max-time 30 -o "$staging/plenipo-relay.sig" "$DOWNLOADS/$tag/$program.sig" \
    || fail "Could not download $program.sig"
  curl -fsSL --retry 3 --max-time 30 -o "$staging/checksum.sha256" "$DOWNLOADS/$tag/$program.sha256" \
    || fail "Could not download $program.sha256"
  # The signature is the gate: nothing below touches the file until it has passed.
  verify_signature "$staging/plenipo-relay" "$staging/plenipo-relay.sig" "$program" "$version"
  # The checksum still catches a damaged download.
  want="$(cut -d' ' -f1 < "$staging/checksum.sha256")"
  got="$(sha256sum "$staging/plenipo-relay" | cut -d' ' -f1)"
  [[ -n "$want" && "$want" == "$got" ]] || fail "$program does not match its checksum"
  if grep -q "$TEST_HOOKS_MARK" "$staging/plenipo-relay"; then
    fail "$program was built with its test hooks; a release never is"
  fi
  # The program runs as the relay's own user, never as root: the folder lets that user in and
  # nobody else, and the program answers --version with a time limit.
  chown "root:$(id -gn "$SERVICE_USER")" "$staging"
  chmod 0750 "$staging"
  chmod 0755 "$staging/plenipo-relay"
  says="$(timeout 20 runuser -u "$SERVICE_USER" -- "$staging/plenipo-relay" --version 2> /dev/null || true)"
  [[ "$says" == "plenipo-relay $version" ]] || fail "The program says \"$says\", not \"plenipo-relay $version\""
  mv "$staging" "$release_dir"
  chown root:root "$release_dir"
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
