#!/usr/bin/env bash
# Plenipo website: follow the latest published release automatically (ADR-069).
#
# Runs on Coastline from a systemd timer (plenipo-website-update.timer), every 15 minutes. Each run:
#   1. asks GitHub for the latest published release (not a draft or pre-release) and checks its
#      Windows installer is attached;
#   2. reads the release's notes, docs/releases/vX.Y.Z.md, from `main` (so a correction made after
#      the release shows), or from the release's tag if `main` has none;
#   3. if the running site already shows that version with those same notes, it stops there
#      (nothing changes); otherwise it exports the website from `main` (the reviewed website code)
#      and the notes into a new, never-edited release folder, and builds the image;
#   4. swaps the running container while holding the shared port-allocation lock, then checks
#      health, pages, version, notes, source, image, a real 404, and the loopback-only binding;
#   5. if any check fails, puts the previous image back and checks it again.
# It never prunes images, stops other services, or touches the firewall, DNS, or the Tunnel.
#
# Usage: auto-release.sh [--check] [--force] [--version X.Y.Z] [--source-ref REF]
#   --check        say what would happen; change nothing
#   --force        rebuild and redeploy even if the site already shows the release
#   --version      deploy this published release instead of the latest (e.g. to go back)
#   --source-ref   website code from this branch, tag, or commit instead of `main`
#
# Settings come from the environment, or from $APP_DIR/auto-release.env if it exists.

set -Eeuo pipefail
umask 022

APP_DIR="${APP_DIR:-/srv/8west/apps/plenipo-website}"
if [[ -f "$APP_DIR/auto-release.env" ]]; then
  # shellcheck disable=SC1091
  source "$APP_DIR/auto-release.env"
fi
REPO_URL="${REPO_URL:-https://github.com/Seckcey/plenipo.git}"
REPO_API="${REPO_API:-https://api.github.com/repos/Seckcey/plenipo}"
SOURCE_REF="${SOURCE_REF:-main}"
PROJECT="${PROJECT:-plenipo-website}"
PORT="${PORT:-14380}"
SUBNET="${SUBNET:-10.204.229.0/28}"
ALLOCATION_LOCK="${ALLOCATION_LOCK:-/srv/8west/port-allocations/allocations.lock}"
STATE_DIR="$APP_DIR/auto-release"
SOURCE_GIT="$STATE_DIR/source.git"
RELEASES_DIR="$APP_DIR/releases"
ORIGIN="http://127.0.0.1:$PORT"
VERSION_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+$'

check_only=false
force=false
wanted_version=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) check_only=true ;;
    --force) force=true ;;
    --version) wanted_version="${2:?--version needs X.Y.Z}"; shift ;;
    --source-ref) SOURCE_REF="${2:?--source-ref needs a branch, tag, or commit}"; shift ;;
    -h | --help) sed -n '2,24p' "$0"; exit 0 ;;
    *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
fail() { log "STOPPED: $*"; exit 1; }

# Files made as root would lock out the deploy user the timer runs as.
if [[ "$(id -u)" == 0 ]]; then
  fail "Do not run this as root; run it as the account that owns $APP_DIR (sudo -u <that account> $0)"
fi
for tool in curl jq git docker flock sha256sum; do
  command -v "$tool" > /dev/null || fail "$tool is not installed (sudo apt install $tool)"
done
docker compose version > /dev/null 2>&1 || fail "docker compose is not available"
[[ -d "$APP_DIR" ]] || fail "$APP_DIR does not exist"
mkdir -p "$STATE_DIR" "$RELEASES_DIR"

# One run at a time; a run that finds another still going just leaves.
exec 8> "$STATE_DIR/run.lock"
if ! flock -n 8; then
  log "Another update is still running; leaving it to finish."
  exit 0
fi

# --- 1. The release to show -------------------------------------------------------------------
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
[[ "$(jq -r '.prerelease' <<< "$release_json")" == false ]] || fail "$tag is a pre-release; the site shows full releases only"
installer="Plenipo_${version}_x64-setup.exe"
jq -e --arg name "$installer" '.assets | any(.name == $name and .state == "uploaded")' <<< "$release_json" > /dev/null \
  || fail "$tag has no $installer attached; the download buttons would break"
# One byte is enough to prove the link works (the signed storage link it redirects to may refuse a
# headers-only request).
curl -fsSL --max-time 60 -r 0-0 -o /dev/null "https://github.com/Seckcey/plenipo/releases/download/$tag/$installer" \
  || fail "The $installer download link does not work"

# --- 2. Source and notes -----------------------------------------------------------------------
if [[ ! -d "$SOURCE_GIT" ]]; then
  log "First run: making a local copy of $REPO_URL"
  git clone --quiet --bare "$REPO_URL" "$SOURCE_GIT"
fi
git --git-dir="$SOURCE_GIT" fetch --quiet --prune --force origin \
  '+refs/heads/*:refs/heads/*' '+refs/tags/*:refs/tags/*' || fail "Could not fetch $REPO_URL"
revision="$(git --git-dir="$SOURCE_GIT" rev-parse --verify --quiet "$SOURCE_REF^{commit}")" \
  || fail "$SOURCE_REF is not a branch, tag, or commit in $REPO_URL"
git --git-dir="$SOURCE_GIT" rev-parse --verify --quiet "refs/tags/$tag^{commit}" > /dev/null \
  || fail "The tag $tag is not in $REPO_URL"
# The notes as they read now on the website's source (a correction after the release shows), or
# as they were at the release's tag.
if notes="$(git --git-dir="$SOURCE_GIT" show "$revision:docs/releases/$tag.md" 2> /dev/null)"; then
  notes_from="$SOURCE_REF"
elif notes="$(git --git-dir="$SOURCE_GIT" show "refs/tags/$tag:docs/releases/$tag.md" 2> /dev/null)"; then
  notes_from="$tag"
else
  fail "Neither $SOURCE_REF nor $tag has docs/releases/$tag.md"
fi
notes_sha="$(printf '%s\n' "$notes" | sha256sum | cut -d' ' -f1)"
image="plenipo-website:${revision}-v${version}"
release_dir="$RELEASES_DIR/${revision}-v${version}"

# --- 3. What the site shows now ---------------------------------------------------------------
current_json="$(curl -fsS --max-time 10 "$ORIGIN/release.json" 2> /dev/null || true)"
jq -e 'type == "object"' <<< "$current_json" > /dev/null 2>&1 || current_json='{}'
current_version="$(jq -r '.version // empty' <<< "$current_json")"
current_notes="$(jq -r '.releaseNotes // false' <<< "$current_json")"
shown_notes_sha=""
if [[ -f "$STATE_DIR/current.env" ]]; then
  shown_notes_sha="$(sed -n 's/^NOTES_SHA256=//p' "$STATE_DIR/current.env")"
fi
log "Latest release: $tag. The site shows: ${current_version:-nothing (not running?)}, notes: ${current_notes:-false}."
if [[ "$current_version" == "$version" && "$current_notes" == true && "$shown_notes_sha" == "$notes_sha" && "$force" == false ]]; then
  log "Up to date. Nothing to do."
  exit 0
fi
if [[ "$current_version" == "$version" && "$shown_notes_sha" != "$notes_sha" ]]; then
  log "The notes for $tag changed since they were shown; showing the new wording."
fi

log "Plan: website code $SOURCE_REF at $revision, version $version, notes from $notes_from."
log "      release folder $release_dir, image $image."
if [[ "$check_only" == true ]]; then
  log "Check only: nothing was changed."
  exit 0
fi

if [[ ! -d "$release_dir" ]]; then
  staging="$(mktemp -d "$RELEASES_DIR/.staging.XXXXXX")"
  trap 'rm -rf "$staging"' EXIT
  git --git-dir="$SOURCE_GIT" archive "$revision" apps/website | tar -x -C "$staging"
  [[ -f "$staging/apps/website/Dockerfile" ]] || fail "$revision has no apps/website/Dockerfile"
  [[ -d "$staging/apps/website/release-notes" ]] \
    || fail "$revision's website cannot show release notes yet (no apps/website/release-notes)"
  printf '%s\n' "$notes" > "$staging/apps/website/release-notes/$tag.md"
  printf 'revision=%s\nversion=%s\nnotes_sha256=%s\nexported_at=%s\n' \
    "$revision" "$version" "$notes_sha" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$staging/RELEASE"
  chmod -R a-w "$staging"
  mv "$staging" "$release_dir"
  trap - EXIT
fi
website="$release_dir/apps/website"

log "Building $image"
nice -n 10 docker build --quiet \
  --build-arg SOURCE_REVISION="$revision" \
  --build-arg PLENIPO_VERSION="$version" \
  -t "$image" "$website" > /dev/null || fail "The image did not build"
image_id="$(docker image inspect -f '{{.Id}}' "$image")"

# --- 4. Swap, under the shared allocation lock ------------------------------------------------
previous_container="$(docker compose -p "$PROJECT" ps -q web 2> /dev/null | head -n 1 || true)"
previous_image=""
previous_compose_dir="$website"
if [[ -n "$previous_container" ]]; then
  previous_image="$(docker inspect -f '{{.Config.Image}}' "$previous_container")"
fi
if [[ -f "$STATE_DIR/current.env" ]]; then
  # The folder the running release was started from, so going back uses its own Compose files.
  recorded_dir="$(sed -n 's/^WEBSITE_DIR=//p' "$STATE_DIR/current.env")"
  recorded_image="$(sed -n 's/^IMAGE=//p' "$STATE_DIR/current.env")"
  if [[ -n "$recorded_dir" && -d "$recorded_dir" && "$recorded_image" == "$previous_image" ]]; then
    previous_compose_dir="$recorded_dir"
  fi
fi

compose_up() { # compose_up <image> <website dir>
  [[ -f "$ALLOCATION_LOCK" ]] || fail "The shared allocation lock $ALLOCATION_LOCK is missing"
  (
    flock -w 300 9 || { log "Timed out waiting for $ALLOCATION_LOCK"; exit 1; }
    PLENIPO_IMAGE="$1" PLENIPO_PORT="$PORT" PLENIPO_SUBNET="$SUBNET" \
      docker compose -p "$PROJECT" -f "$2/compose.yaml" -f "$2/compose.coastline.yaml" \
      up -d --wait --wait-timeout 120
  ) 9< "$ALLOCATION_LOCK"
}

verify() { # verify <version> <revision> <image id>; prints what failed
  local body release container
  body="$(curl -fsS --max-time 10 "$ORIGIN/healthz")" || { echo "health check failed"; return 1; }
  body="$(curl -fsS --max-time 10 "$ORIGIN/")" || { echo "home page failed"; return 1; }
  grep -q 'Your AI workforce' <<< "$body" || { echo "home page is missing its heading"; return 1; }
  grep -q 'id="whats-new"' <<< "$body" || { echo "home page is missing What's new"; return 1; }
  grep -q "What&rsquo;s new in v$1<" <<< "$body" || { echo "What's new shows the wrong version"; return 1; }
  release="$(curl -fsS --max-time 10 "$ORIGIN/release.json")" || { echo "release.json failed"; return 1; }
  [[ "$(jq -r .version <<< "$release")" == "$1" ]] || { echo "release.json shows the wrong version"; return 1; }
  [[ "$(jq -r .sourceRevision <<< "$release")" == "$2" ]] || { echo "release.json names the wrong source"; return 1; }
  [[ "$(jq -r .releaseNotes <<< "$release")" == true ]] || { echo "the page has no release notes"; return 1; }
  [[ "$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$ORIGIN/no-such-page")" == 404 ]] \
    || { echo "an unknown page is not a 404"; return 1; }
  container="$(docker compose -p "$PROJECT" ps -q web | head -n 1)"
  [[ "$(docker inspect -f '{{.Image}}' "$container")" == "$3" ]] || { echo "the container runs another image"; return 1; }
  [[ "$(docker inspect -f '{{.RestartCount}}' "$container")" == 0 ]] || { echo "the container is restarting"; return 1; }
  [[ "$(docker port "$container" 8080/tcp)" == "127.0.0.1:$PORT" ]] || { echo "the port is not loopback-only"; return 1; }
}

record() { # record <result> <detail>
  jq -cn --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg result "$1" --arg detail "$2" \
    --arg version "$version" --arg revision "$revision" --arg image "$image" --arg imageId "$image_id" \
    --arg releaseDir "$release_dir" --arg notes "$notes_sha" --arg previous "$previous_image" \
    '{at: $at, result: $result, detail: $detail, version: $version, revision: $revision, image: $image,
      imageId: $imageId, releaseDir: $releaseDir, notesSha256: $notes, previousImage: $previous}' \
    >> "$STATE_DIR/history.jsonl"
}

log "Starting $image (previous: ${previous_image:-none})"
problem=""
if ! compose_up "$image" "$website"; then
  problem="the container did not become healthy"
else
  problem="$(verify "$version" "$revision" "$image_id")" || true
fi

if [[ -n "$problem" ]]; then
  log "The new release failed its checks: $problem"
  if [[ -z "$previous_image" ]]; then
    record failed "$problem; nothing to go back to"
    fail "No previous image to go back to; the site may be down. See: docker compose -p $PROJECT logs"
  fi
  log "Going back to $previous_image"
  if compose_up "$previous_image" "$previous_compose_dir" && curl -fsS --max-time 10 "$ORIGIN/healthz" > /dev/null; then
    record rolled-back "$problem"
    fail "Went back to $previous_image, which is healthy. The new release needs a look."
  fi
  record rollback-failed "$problem"
  fail "Going back to $previous_image also failed; the site may be down. See: docker compose -p $PROJECT logs"
fi

# --- 5. Record it ------------------------------------------------------------------------------
curl -fsS --max-time 10 "$ORIGIN/release.json" > "$STATE_DIR/release.json.tmp"
mv "$STATE_DIR/release.json.tmp" "$STATE_DIR/release.json"
{
  printf 'VERSION=%s\nREVISION=%s\nIMAGE=%s\nIMAGE_ID=%s\n' "$version" "$revision" "$image" "$image_id"
  printf 'WEBSITE_DIR=%s\nNOTES_SHA256=%s\nPREVIOUS_IMAGE=%s\nDEPLOYED_AT=%s\n' \
    "$website" "$notes_sha" "$previous_image" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$STATE_DIR/current.env.tmp"
mv "$STATE_DIR/current.env.tmp" "$STATE_DIR/current.env"
record deployed "all checks passed"
log "Done: the site shows v$version with its notes (source $revision, image $image_id)."
