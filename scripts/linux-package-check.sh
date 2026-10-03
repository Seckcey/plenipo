#!/usr/bin/env bash
# Phase 23, Wave 2 (ADR-152): the Release workflow's check of the Linux files it just built, on
# GitHub's Ubuntu 22.04, before anything is published. Plenipo is made by 8 West Ventures, LLC.
#
# - Both files are this version's, and the .deb names its maker, what it needs, and the password
#   store it recommends (ADR-153).
# - The .deb installs with apt (so everything it needs is there on Ubuntu), and puts Plenipo in
#   the applications list.
# - The installed Plenipo and the AppImage each start on a desktop (a virtual screen with its own
#   session bus and password store), stay running, and make their data folder.
# - apt removes the program and leaves the person's own folder alone (that is what Settings →
#   "Delete my Plenipo data" is for).
#
# Then it copies both files to linux-dist/ for the release.
#
# Usage: bash scripts/linux-package-check.sh   (from the repository, after `tauri build`)
set -euo pipefail
shopt -s nullglob

fail() {
  echo "::error::$*"
  exit 1
}

version=$(node -p "require('./package.json').version")
debs=(target/release/bundle/deb/*.deb)
appimages=(target/release/bundle/appimage/*.AppImage)
[ "${#debs[@]}" -eq 1 ] || fail "expected one .deb, found ${#debs[@]}"
[ "${#appimages[@]}" -eq 1 ] || fail "expected one AppImage, found ${#appimages[@]}"
deb="${debs[0]}"
appimage="$(realpath "${appimages[0]}")"
[ "${deb##*/}" = "Plenipo_${version}_amd64.deb" ] || fail "the .deb is called ${deb##*/}"
[ "${appimage##*/}" = "Plenipo_${version}_amd64.AppImage" ] || fail "the AppImage is called ${appimage##*/}"

field() { dpkg-deb --field "$deb" "$1"; }
[ "$(field Package)" = "plenipo" ] || fail "the .deb's package is $(field Package)"
[ "$(field Version)" = "$version" ] || fail "the .deb says version $(field Version), not $version"
field Maintainer | grep -q "8 West Ventures, LLC" || fail "the .deb's maker is $(field Maintainer)"
field Depends | grep -q "libwebkit2gtk-4.1-0" || fail "the .deb does not need WebKitGTK: $(field Depends)"
field Depends | grep -q "libayatana-appindicator3-1" || fail "the .deb does not need the tray library: $(field Depends)"
field Recommends | grep -q "gnome-keyring" || fail "the .deb does not recommend a password store"
echo "The .deb: $(field Package) $(field Version); needs $(field Depends); recommends $(field Recommends)"

# Start a program on a virtual screen, with a home folder of its own (left in `started_home`), its
# own session bus, and an unlocked password store; it must still be running after 20 seconds, and
# have made its data folder.
started_home=""
starts() {
  local name="$1"
  shift
  started_home="$(mktemp -d)"
  local status=0
  env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u XDG_CACHE_HOME HOME="$started_home" \
    dbus-run-session -- bash -c '
      gnome-keyring-daemon --components=secrets --daemonize --unlock <<< plenipo-check > /dev/null
      xvfb-run -a timeout -k 5 20 "$@"
    ' starts "$@" > "$RUNNER_TEMP/plenipo-start.txt" 2>&1 || status=$?
  # 124: still running when the time was up, and it stopped when asked; 137: it took more than
  # five seconds to stop, so it was ended. Anything else: it stopped by itself.
  if [ "$status" -ne 124 ] && [ "$status" -ne 137 ]; then
    cat "$RUNNER_TEMP/plenipo-start.txt"
    fail "$name stopped by itself (exit $status) instead of running"
  fi
  [ -d "$started_home/.local/share/com.eightwest.plenipo" ] || {
    cat "$RUNNER_TEMP/plenipo-start.txt"
    fail "$name made no data folder"
  }
  echo "$name started and kept running."
}

sudo apt-get install -y "./$deb"
# The list first, then searched: a search that stops early never fails the listing with it.
installed_files="$(dpkg -L plenipo)"
program="$(grep -E '^/usr/bin/[^/]+$' <<< "$installed_files" | sed -n 1p || true)"
[ -n "$program" ] && [ -x "$program" ] || fail "the .deb put no program in /usr/bin"
grep -qE '^/usr/share/applications/.+\.desktop$' <<< "$installed_files" || fail "the .deb has no applications-list entry"
starts "The installed Plenipo" "$program"
installed_home="$started_home"
# The AppImage unpacks itself instead of needing FUSE on the runner.
starts "The AppImage" env APPIMAGE_EXTRACT_AND_RUN=1 "$appimage"

sudo apt-get remove -y plenipo
[ ! -e "$program" ] || fail "apt left $program behind"
[ -d "$installed_home/.local/share/com.eightwest.plenipo" ] || fail "apt removed the person's own Plenipo data"
echo "apt removed Plenipo and left the person's own data, for \"Delete my Plenipo data\"."

mkdir -p linux-dist
cp "$deb" "$appimage" linux-dist/
ls -l linux-dist
