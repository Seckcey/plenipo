#!/usr/bin/env bash
# Plenipo on your phone: serve the phone's page at remote.getplenipo.com from its own small server
# (Phase 14, ADR-146 and ADR-148; ADR-210, the page and the relay are signed like the installer).
# Made by 8 West Ventures, LLC.
#
# Runs from a systemd timer (plenipo-phone-page-update.timer), every 15 minutes. Each run:
#   1. asks GitHub for the latest published release (not a draft or pre-release), and refuses one
#      older than the release served (a step back is taken only by hand, with --version);
#   2. if the page already shows that version, stops there;
#   3. downloads the release's plenipo-phone-page-<version>.zip, its .sig, and its .sha256, checks
#      the signature first (made with 8 West's server key, over this very file, for this version;
#      the trusted keys live in /etc/plenipo-phone-page/trusted-keys.d), then the checksum, then
#      unpacks it into a new, never-edited release folder, and checks the page names only 8 West's
#      relay (never a test address);
#   4. points the served folder at it (one atomic switch; the small web server keeps running),
#      starting the web server if it is not running;
#   5. checks the page, its background part (sw.js), and its rules (headers); if any check fails,
#      points back at the previous release and checks again.
# It never stops other services, and never touches the firewall, DNS, or the Tunnel.
#
# Usage: update-page.sh [--check] [--force] [--version X.Y.Z]
#   --check     say what would happen; change nothing
#   --force     unpack and switch even if the page already shows the release
#   --version   serve this published release instead of the latest (e.g. to go back)
#
# Settings come from the environment, or from $APP_DIR/update-page.env if it exists.

set -Eeuo pipefail
umask 022

APP_DIR="${APP_DIR:-/srv/8west/apps/plenipo-phone-page}"
if [[ -f "$APP_DIR/update-page.env" ]]; then
  # shellcheck disable=SC1091
  source "$APP_DIR/update-page.env"
fi
REPO_API="${REPO_API:-https://api.github.com/repos/Seckcey/plenipo}"
DOWNLOADS="${DOWNLOADS:-https://github.com/Seckcey/plenipo/releases/download}"
PROJECT="${PROJECT:-plenipo-phone-page}"
PORT="${PORT:?Set PORT to the verified unused loopback port for the phone page}"
# A reserved network, only on a machine whose Docker address pool is full (empty: Docker's own).
SUBNET="${SUBNET:-}"
# A lock shared with other services that pick ports and networks, where a machine has one.
ALLOCATION_LOCK="${ALLOCATION_LOCK:-}"
DEPLOY_DIR="${DEPLOY_DIR:-$APP_DIR/deploy}"
# 8 West's server key, its public half, installed once by hand (README): every *.pub in here, as
# `tauri signer generate` prints it (one base64 line) or as a minisign public key file. Root owns
# the folder and the files, and nobody else may write them: not even the account this runs as.
TRUSTED_KEYS_DIR="${TRUSTED_KEYS_DIR:-/etc/plenipo-phone-page/trusted-keys.d}"
RELEASES_DIR="$APP_DIR/releases"
STATE_DIR="$APP_DIR/state"
ORIGIN="http://127.0.0.1:$PORT"
RELAY="wss://relay.getplenipo.com/plenipo/v1/phone"
VERSION_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+$'

check_only=false
force=false
wanted_version=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) check_only=true ;;
    --force) force=true ;;
    --version) wanted_version="${2:?--version needs X.Y.Z}"; shift ;;
    -h | --help) sed -n '2,27p' "$0"; exit 0 ;;
    *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
fail() { log "STOPPED: $*"; exit 1; }

if [[ "$(id -u)" == 0 ]]; then
  fail "Do not run this as root; run it as the account that owns $APP_DIR"
fi
for tool in curl jq unzip sha256sum docker flock minisign base64; do
  command -v "$tool" > /dev/null || fail "$tool is not installed (sudo apt install $tool)"
done
docker compose version > /dev/null 2>&1 || fail "docker compose is not available"
[[ -d "$APP_DIR" ]] || fail "$APP_DIR does not exist"
[[ -f "$DEPLOY_DIR/compose.yaml" ]] || fail "$DEPLOY_DIR has no compose.yaml (copy apps/remote/deploy there)"
mkdir -p "$RELEASES_DIR" "$STATE_DIR"

# One run at a time.
exec 8> "$STATE_DIR/run.lock"
if ! flock -n 8; then
  log "Another update is still running; leaving it to finish."
  exit 0
fi

# --- The trusted keys ------------------------------------------------------------------------
# Only root may write the folder and the keys: a key anyone else could add would sign anything.
root_only() { # root_only <path>: owned by root, and no write bit for the group or others
  [[ "$(stat -c '%U %a' "$1")" =~ ^root\ [0-7]?[0-7][0145][0145]$ ]]
}
[[ -d "$TRUSTED_KEYS_DIR" ]] \
  || fail "$TRUSTED_KEYS_DIR does not exist: install 8 West's server key first (README, \"the server key\")"
root_only "$TRUSTED_KEYS_DIR" \
  || fail "$TRUSTED_KEYS_DIR must be owned by root and writable by root only (sudo chown root:root; sudo chmod 0755)"
trusted_keys=()
for key in "$TRUSTED_KEYS_DIR"/*.pub; do
  [[ -f "$key" ]] || continue
  root_only "$key" || fail "$key must be owned by root and writable by root only (sudo chown root:root; sudo chmod 0644)"
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

# --- 1. The release to serve -------------------------------------------------------------------
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
[[ "$(jq -r '.prerelease' <<< "$release_json")" == false ]] || fail "$tag is a pre-release; the page shows full releases only"
page="plenipo-phone-page-$version.zip"
for asset in "$page" "$page.sig" "$page.sha256"; do
  jq -e --arg name "$asset" '.assets | any(.name == $name and .state == "uploaded")' <<< "$release_json" > /dev/null \
    || fail "$tag has no $asset attached (releases before 1.19.2 carry no phone page; the .sig came later, ADR-210)"
done
# Never a step back on its own: an older "latest" (a deleted release, or a wrong answer) is
# refused. The operator goes back with --version.
served_version=""
if [[ -f "$STATE_DIR/current.env" ]]; then
  served_version="$(sed -n 's/^VERSION=//p' "$STATE_DIR/current.env" | head -n 1)"
fi
older_than() { # older_than <a> <b>: version a is lower than version b
  [[ "$1" != "$2" && "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n 1)" == "$1" ]]
}
if [[ -z "$wanted_version" && "$served_version" =~ $VERSION_PATTERN ]] && older_than "$version" "$served_version"; then
  fail "GitHub's latest release is $tag, older than the $served_version served; to go back on purpose, run this with --version $version"
fi

# --- 2. What is served now ---------------------------------------------------------------------
current=""
if [[ -L "$APP_DIR/current" ]]; then
  current="$(basename "$(readlink "$APP_DIR/current")")"
fi
log "Latest release: $tag. Served now: ${current:-nothing}."
if [[ "$current" == "$version" && "$force" == false ]] && curl -fsS --max-time 10 "$ORIGIN/healthz" > /dev/null 2>&1; then
  log "Up to date. Nothing to do."
  exit 0
fi
if [[ "$check_only" == true ]]; then
  log "Check only: would serve $tag from $RELEASES_DIR/$version. Nothing was changed."
  exit 0
fi

# --- 3. Download, check, and unpack ------------------------------------------------------------
release_dir="$RELEASES_DIR/$version"
if [[ ! -d "$release_dir" ]]; then
  staging="$(mktemp -d "$RELEASES_DIR/.staging.XXXXXX")"
  trap 'rm -rf "$staging"' EXIT
  curl -fsSL --retry 3 --max-time 120 -o "$staging/$page" "$DOWNLOADS/$tag/$page" \
    || fail "Could not download $page"
  curl -fsSL --retry 3 --max-time 30 -o "$staging/$page.sig" "$DOWNLOADS/$tag/$page.sig" \
    || fail "Could not download $page.sig"
  curl -fsSL --retry 3 --max-time 30 -o "$staging/$page.sha256" "$DOWNLOADS/$tag/$page.sha256" \
    || fail "Could not download $page.sha256"
  # The signature is the gate: nothing below touches the file until it has passed.
  verify_signature "$staging/$page" "$staging/$page.sig" "$page" "$version"
  # The checksum still catches a damaged download.
  want="$(cut -d' ' -f1 < "$staging/$page.sha256")"
  got="$(sha256sum "$staging/$page" | cut -d' ' -f1)"
  [[ -n "$want" && "$want" == "$got" ]] || fail "$page does not match its checksum"
  mkdir "$staging/page"
  unzip -q "$staging/$page" -d "$staging/page"
  [[ -f "$staging/page/index.html" && -f "$staging/page/sw.js" ]] || fail "$page has no index.html or sw.js"
  # The page names only 8 West's relay, never a test address.
  grep -Fq "connect-src wss://relay.getplenipo.com" "$staging/page/index.html" \
    || fail "$page's rules do not name 8 West's relay"
  grep -Fq "$RELAY" "$staging/page/sw.js" || fail "$page does not reach 8 West's relay"
  if grep -rqE '127\.0\.0\.1|localhost:87' "$staging/page"; then
    fail "$page names a test address"
  fi
  # Moved into place first (Linux moves a folder only while it can be written), then made
  # read-only: the release is never edited again.
  mv "$staging/page" "$release_dir"
  chmod -R a-w "$release_dir"
  rm -rf "$staging"
  trap - EXIT
fi

# --- 4. Switch, and start the web server if needed ---------------------------------------------
previous="$current"
point_at() { # point_at <version>: one atomic switch of the served folder
  ln -sfn "releases/$1" "$APP_DIR/current.next"
  mv -Tf "$APP_DIR/current.next" "$APP_DIR/current"
}
point_at "$version"

running="$(docker ps -q --filter "label=com.docker.compose.project=$PROJECT" --filter 'label=com.docker.compose.service=web' | head -n 1)"
if [[ -z "$running" ]]; then
  files=(-f "$DEPLOY_DIR/compose.yaml")
  if [[ -n "$SUBNET" ]]; then
    files+=(-f "$DEPLOY_DIR/compose.subnet.yaml")
  fi
  start() {
    PLENIPO_PAGE_DIR="$APP_DIR" PLENIPO_PORT="$PORT" PLENIPO_SUBNET="$SUBNET" \
      docker compose -p "$PROJECT" "${files[@]}" up -d --wait --wait-timeout 120
  }
  if [[ -n "$ALLOCATION_LOCK" ]]; then
    [[ -f "$ALLOCATION_LOCK" ]] || fail "The shared allocation lock $ALLOCATION_LOCK is missing"
    (
      flock -w 300 9 || { log "Timed out waiting for $ALLOCATION_LOCK"; exit 1; }
      start
    ) 9< "$ALLOCATION_LOCK" || fail "The web server did not start"
  else
    start || fail "The web server did not start"
  fi
fi

# --- 5. Check, and go back if anything is wrong ------------------------------------------------
verify() { # verify <version>: prints what failed
  local head body
  curl -fsS --max-time 10 "$ORIGIN/healthz" > /dev/null || { echo "health check failed"; return 1; }
  head="$(curl -fsSI --max-time 10 "$ORIGIN/")" || { echo "the page failed"; return 1; }
  body="$(curl -fsS --max-time 10 "$ORIGIN/")" || { echo "the page failed"; return 1; }
  grep -Fq "connect-src wss://relay.getplenipo.com" <<< "$body" || { echo "the page's rules are missing"; return 1; }
  grep -qi '^content-security-policy:.*frame-ancestors .none.' <<< "$head" || { echo "the rules header is missing"; return 1; }
  grep -qi '^x-content-type-options: nosniff' <<< "$head" || { echo "nosniff is missing"; return 1; }
  grep -qi '^cache-control: no-cache' <<< "$head" || { echo "the page could be kept stale"; return 1; }
  curl -fsS --max-time 10 "$ORIGIN/sw.js" | grep -Fq "$RELAY" || { echo "sw.js failed"; return 1; }
  # The address a pairing link opens (/#pair=…) is the page itself.
  curl -fsS --max-time 10 "$ORIGIN/manifest.webmanifest" > /dev/null || { echo "the manifest failed"; return 1; }
  [[ "$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$ORIGIN/missing.js")" == 404 ]] \
    || { echo "a missing file did not answer 404"; return 1; }
}

if problem="$(verify "$version")"; then
  printf 'VERSION=%s\nSERVED_AT=%s\n' "$version" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$STATE_DIR/current.env"
  log "Serving $tag."
  exit 0
fi
log "The check of $tag failed: $problem"
if [[ -n "$previous" && -d "$RELEASES_DIR/$previous" ]]; then
  point_at "$previous"
  if again="$(verify "$previous")"; then
    fail "Went back to $previous, which passes its checks"
  fi
  fail "Went back to $previous, which also fails: $again"
fi
fail "Nothing earlier to go back to"
