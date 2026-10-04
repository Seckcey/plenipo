#!/usr/bin/env bash
# The server updaters install only what 8 West signed (ADR-210, the page and the relay are signed
# like the installer; security finding P-SRV-1). Made by 8 West Ventures, LLC.
#
# Runs crates/relay/deploy/update-relay.sh and apps/remote/deploy/update-page.sh against a
# stand-in for GitHub (files on this machine, read over file://) with a throwaway key made here,
# stand-ins for systemctl and docker, and a small web server for each health check, and checks:
#   - a release signed with the trusted key is installed (the relay) and served (the page);
#   - a release with no signature, one made with another key, one made for another file (the
#     installer's name on the relay's program, the relay's on the page), one made for another
#     version, or a file changed after it was signed is refused before the file is used at all:
#     the relay's program never runs, and the page is never unpacked;
#   - a damaged download (checksum) is still refused, before the program runs;
#   - an older "latest" release is refused, and --version takes the step back on purpose;
#   - the relay's program never runs as root;
#   - with no trusted key, or a key folder that is not root's alone, nothing is installed;
#   - a key is accepted as `tauri signer generate` prints it (one base64 line) and as a minisign
#     public key file.
#
# Linux only, as root (sudo): it makes folders root owns and runs the relay's program as another
# user (PLENIPO_TEST_USER, default nobody). Needs minisign, jq, curl, unzip, zip, python3, and
# runuser (CI installs minisign; Ubuntu 24.04 has the rest):
#   sudo bash scripts/updater-signing-check.sh
# It changes nothing outside a temporary folder it deletes at the end.

set -Eeuo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELAY_SCRIPT="$HERE/crates/relay/deploy/update-relay.sh"
PAGE_SCRIPT="$HERE/apps/remote/deploy/update-page.sh"
OTHER_USER="${PLENIPO_TEST_USER:-nobody}"
V=1.2.3      # the release under test
OLDER=1.2.2  # an older release, for the step back
RELAY_NAME="plenipo-relay-$V-linux-x86_64"
PAGE_NAME="plenipo-phone-page-$V.zip"

say() { printf '%s\n' "$*"; }
die() { say "FAILED: $*"; exit 1; }

[[ "$(id -u)" == 0 ]] || die "run this as root (sudo bash $0): it makes folders root owns and runs a program as $OTHER_USER"
for tool in minisign jq curl unzip zip python3 runuser sha256sum flock base64 timeout stat; do
  command -v "$tool" > /dev/null || die "$tool is not installed"
done
id "$OTHER_USER" > /dev/null 2>&1 || die "there is no user $OTHER_USER (set PLENIPO_TEST_USER)"
OTHER_UID="$(id -u "$OTHER_USER")"
[[ "$OTHER_UID" != 0 ]] || die "PLENIPO_TEST_USER must not be root"

T="$(mktemp -d /tmp/plenipo-updaters.XXXXXX)"
chmod 0755 "$T"
# Copies of the updaters that $OTHER_USER can read, wherever the checkout lives.
cp "$RELAY_SCRIPT" "$T/update-relay.sh"
cp "$PAGE_SCRIPT" "$T/update-page.sh"
chmod 0755 "$T/update-relay.sh" "$T/update-page.sh"
RELAY_SCRIPT="$T/update-relay.sh"
PAGE_SCRIPT="$T/update-page.sh"
servers=()
cleanup() {
  local pid
  for pid in "${servers[@]:-}"; do
    [[ -n "$pid" ]] && kill "$pid" 2> /dev/null || true
  done
  rm -rf "$T"
}
trap cleanup EXIT
umask 022

# --- Keys: a trusted one and another, both throwaway ------------------------------------------
minisign -G -W -p "$T/server.pub" -s "$T/server.key" > /dev/null 2>&1 || die "minisign could not make a key (is it 0.10 or newer?)"
minisign -G -W -p "$T/other.pub" -s "$T/other.key" > /dev/null 2>&1
# The relay's folder gets the key as `tauri signer generate` prints it; the page's as minisign
# wrote it. Both root-owned, like the real ones.
install -d -m 0755 "$T/relay-keys" "$T/page-keys" "$T/no-keys"
base64 -w0 "$T/server.pub" > "$T/relay-keys/8west-server.pub"
cp "$T/server.pub" "$T/page-keys/8west-server.pub"
chmod 0644 "$T/relay-keys/8west-server.pub" "$T/page-keys/8west-server.pub"

sign() { # sign <file> <key> <signed file name> <version>: writes <file>.sig as Tauri's tool does
  local file="$1" key="$2" name="$3" version="$4"
  rm -f "$T/raw.sig"
  minisign -S -W -s "$key" -x "$T/raw.sig" -c "signature from tauri secret key" \
    -t "$(printf 'timestamp:1700000000\tfile:%s\tversion:%s' "$name" "$version")" -m "$file" > /dev/null 2>&1 \
    || die "minisign could not sign $file"
  base64 -w0 "$T/raw.sig" > "$file.sig"
}

# --- A stand-in for GitHub --------------------------------------------------------------------
API="$T/api"
DL="$T/dl"
publish() { # publish <version> <latest: yes|no> <asset names...>: the release's JSON
  local version="$1" latest="$2"
  shift 2
  mkdir -p "$API/releases/tags"
  jq -n --arg tag "v$version" \
    '{tag_name: $tag, draft: false, prerelease: false, assets: [$ARGS.positional[] | {name: ., state: "uploaded"}]}' \
    --args "$@" > "$API/releases/tags/v$version"
  if [[ "$latest" == yes ]]; then
    cp "$API/releases/tags/v$version" "$API/releases/latest"
  fi
}

make_relay() { # make_relay <version> <how: good|garbage|other|wrongfile|wrongversion|badsum|tampered>
  local version="$1" how="$2" name="plenipo-relay-$1-linux-x86_64" dir="$DL/v$1"
  rm -rf "$dir"
  mkdir -p "$dir"
  # The program is a stand-in: it writes down who ran it and answers --version. (It must not
  # carry the line a test copy of the relay prints.)
  printf '#!/bin/sh\nid -u >> %s\n[ "$1" = "--version" ] && echo "plenipo-relay %s"\nexit 0\n' "$T/ran-as" "$version" > "$dir/$name"
  (cd "$dir" && sha256sum "$name" > "$name.sha256")
  case "$how" in
    good) sign "$dir/$name" "$T/server.key" "$name" "$version" ;;
    garbage) echo "not a signature" > "$dir/$name.sig" ;;
    other) sign "$dir/$name" "$T/other.key" "$name" "$version" ;;
    wrongfile) sign "$dir/$name" "$T/server.key" "Plenipo_${version}_x64-setup.exe" "$version" ;;
    wrongversion) sign "$dir/$name" "$T/server.key" "$name" "9.9.9" ;;
    badsum)
      sign "$dir/$name" "$T/server.key" "$name" "$version"
      echo "0000000000000000000000000000000000000000000000000000000000000000  $name" > "$dir/$name.sha256"
      ;;
    tampered) # signed, then changed: one byte more
      sign "$dir/$name" "$T/server.key" "$name" "$version"
      printf 'x' >> "$dir/$name"
      ;;
  esac
  publish "$version" yes "$name" "$name.sig" "$name.sha256"
}

make_page() { # make_page <version> <how: good|garbage|other|wrongfile|wrongversion|tampered>
  local version="$1" how="$2" name="plenipo-phone-page-$1.zip" dir="$DL/v$1" src="$T/page-src"
  rm -rf "$dir" "$src"
  mkdir -p "$dir" "$src"
  printf '<!doctype html><meta http-equiv="Content-Security-Policy" content="connect-src wss://relay.getplenipo.com"><title>Plenipo %s</title>\n' "$version" > "$src/index.html"
  printf '// %s\nconst relay = "wss://relay.getplenipo.com/plenipo/v1/phone";\n' "$version" > "$src/sw.js"
  printf '{"name":"Plenipo"}\n' > "$src/manifest.webmanifest"
  (cd "$src" && zip -qr "$dir/$name" .)
  (cd "$dir" && sha256sum "$name" > "$name.sha256")
  case "$how" in
    good) sign "$dir/$name" "$T/server.key" "$name" "$version" ;;
    garbage) echo "not a signature" > "$dir/$name.sig" ;;
    other) sign "$dir/$name" "$T/other.key" "$name" "$version" ;;
    wrongfile) sign "$dir/$name" "$T/server.key" "plenipo-relay-$version-linux-x86_64" "$version" ;;
    wrongversion) sign "$dir/$name" "$T/server.key" "$name" "9.9.9" ;;
    tampered) # signed, then changed: one byte more
      sign "$dir/$name" "$T/server.key" "$name" "$version"
      printf 'x' >> "$dir/$name"
      ;;
  esac
  publish "$version" yes "$name" "$name.sig" "$name.sha256"
}

# --- Stand-ins for systemctl, journalctl, and docker, and the health checks' web servers --------
mkdir -p "$T/bin"
printf '#!/bin/sh\necho "$*" >> %s\ncase "$1" in\n  is-active) [ "$2" = "--quiet" ] || echo active ;;\nesac\nexit 0\n' "$T/systemctl.log" > "$T/bin/systemctl"
printf '#!/bin/sh\nexit 0\n' > "$T/bin/journalctl"
printf '#!/bin/sh\ncase "$1 $2" in\n  "ps -q") echo stand-in-container ;;\nesac\nexit 0\n' > "$T/bin/docker"
chmod 0755 "$T/bin/"*

free_port() { python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()'; }
wait_for() { # wait_for <url>
  local tries
  for tries in $(seq 1 50); do
    curl -fsS --max-time 2 "$1" > /dev/null 2>&1 && return 0
    sleep 0.1
  done
  die "no answer at $1"
}

# The relay's health check: /healthz says ok.
RELAY_PORT="$(free_port)"
mkdir -p "$T/relay-www"
echo ok > "$T/relay-www/healthz"
python3 -m http.server --bind 127.0.0.1 --directory "$T/relay-www" "$RELAY_PORT" > /dev/null 2>&1 &
servers+=("$!")
wait_for "http://127.0.0.1:$RELAY_PORT/healthz"

# The page's web server: the served folder, with the headers the updater checks.
PAGE_PORT="$(free_port)"
cat > "$T/page-server.py" << 'PY'
import http.server, sys
root, port = sys.argv[1], int(sys.argv[2])
class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=root, **kwargs)
    def end_headers(self):
        self.send_header("Content-Security-Policy", "default-src 'self'; frame-ancestors 'none'")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()
    def do_GET(self):
        if self.path == "/healthz":
            body = b"ok\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        super().do_GET()
    def log_message(self, *args):
        pass
http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
PY
python3 "$T/page-server.py" "$T/page/current" "$PAGE_PORT" > /dev/null 2>&1 &
servers+=("$!")
wait_for "http://127.0.0.1:$PAGE_PORT/healthz"

# --- Running the updaters --------------------------------------------------------------------
reset_relay() {
  rm -rf "$T/relay"
  mkdir -p "$T/relay"
  : > "$T/ran-as"
  chmod 0666 "$T/ran-as"
  : > "$T/systemctl.log"
}
run_relay() { # run_relay [keys dir] -- <args...>: exit status in $status, output in $T/out
  local keys="$T/relay-keys"
  if [[ "${1:-}" != "--" ]]; then keys="$1"; shift; fi
  [[ "${1:-}" == "--" ]] && shift
  status=0
  env -i PATH="$T/bin:/usr/sbin:/usr/bin:/sbin:/bin" HOME=/root \
    APP_DIR="$T/relay" ENV_FILE=/nonexistent REPO_API="file://$API" DOWNLOADS="file://$DL" \
    SERVICE_USER="$OTHER_USER" TRUSTED_KEYS_DIR="$keys" PLENIPO_RELAY_LISTEN="127.0.0.1:$RELAY_PORT" \
    bash "$RELAY_SCRIPT" "$@" > "$T/out" 2>&1 || status=$?
}
reset_page() {
  rm -rf "$T/page"
  mkdir -p "$T/page/deploy"
  echo "services: {web: {image: stand-in}}" > "$T/page/deploy/compose.yaml"
  chown -R "$OTHER_USER" "$T/page"
}
run_page() { # run_page [keys dir] -- <args...>: as the deploy user, never root
  local keys="$T/page-keys"
  if [[ "${1:-}" != "--" ]]; then keys="$1"; shift; fi
  [[ "${1:-}" == "--" ]] && shift
  status=0
  runuser -u "$OTHER_USER" -- env -i PATH="$T/bin:/usr/sbin:/usr/bin:/sbin:/bin" HOME=/nonexistent \
    APP_DIR="$T/page" PORT="$PAGE_PORT" REPO_API="file://$API" DOWNLOADS="file://$DL" \
    TRUSTED_KEYS_DIR="$keys" \
    bash "$PAGE_SCRIPT" "$@" > "$T/out" 2>&1 || status=$?
}

stopped_with() { # stopped_with <text>: the run stopped, and said why
  [[ "$status" != 0 ]] || { cat "$T/out"; die "$case: the updater did not stop"; }
  grep -Fq "STOPPED: " "$T/out" || { cat "$T/out"; die "$case: no STOPPED line"; }
  grep -Fq "$1" "$T/out" || { cat "$T/out"; die "$case: expected \"$1\""; }
}
relay_never_ran() {
  [[ ! -s "$T/ran-as" ]] || { cat "$T/ran-as"; die "$case: the relay's program ran"; }
  [[ ! -e "$T/relay/releases/$V" ]] || die "$case: a release folder was made"
}
page_never_unpacked() {
  [[ ! -e "$T/page/releases/$V" ]] || die "$case: the page was unpacked"
  [[ -z "$(ls -A "$T/page/releases" 2> /dev/null)" ]] || { ls -la "$T/page/releases"; die "$case: something was left in releases/"; }
}
pass() { say "ok: $case"; }

# ================================================================================================
say "The relay's updater"

case="a signed release is installed, and its program runs as $OTHER_USER, never root"
reset_relay
make_relay "$V" good
run_relay --
[[ "$status" == 0 ]] || { cat "$T/out"; die "$case: exit $status"; }
grep -Fq "$RELAY_NAME is signed with 8 West's server key (8west-server.pub) for $V." "$T/out" || { cat "$T/out"; die "$case: no signature line"; }
grep -Fq "Running v$V." "$T/out" || { cat "$T/out"; die "$case: not running"; }
[[ "$(readlink "$T/relay/current")" == "releases/$V" ]] || die "$case: current points elsewhere"
grep -Fxq "VERSION=$V" "$T/relay/state/current.env" || die "$case: no VERSION in current.env"
grep -Fq "restart plenipo-relay" "$T/systemctl.log" || die "$case: the service was not restarted"
[[ -s "$T/ran-as" ]] || die "$case: the program never ran"
while read -r uid; do
  [[ "$uid" == "$OTHER_UID" ]] || { cat "$T/ran-as"; die "$case: the program ran as uid $uid, not $OTHER_UID"; }
done < "$T/ran-as"
[[ "$(stat -c '%U %a' "$T/relay/releases/$V")" == "root 755" ]] || die "$case: the release folder is not root's, 0755"
pass

case="an older latest release is refused on its own, and installed with --version"
make_relay "$OLDER" good
: > "$T/ran-as"
run_relay --
stopped_with "GitHub's latest release is v$OLDER, older than the $V installed"
[[ ! -s "$T/ran-as" ]] || die "$case: the program ran"
[[ "$(readlink "$T/relay/current")" == "releases/$V" ]] || die "$case: current moved"
run_relay -- --version "$OLDER"
[[ "$status" == 0 ]] || { cat "$T/out"; die "$case: --version exit $status"; }
grep -Fq "Running v$OLDER." "$T/out" || { cat "$T/out"; die "$case: not running the older one"; }
[[ "$(readlink "$T/relay/current")" == "releases/$OLDER" ]] || die "$case: current did not move"
pass

case="no signature: refused before the program runs"
reset_relay
make_relay "$V" garbage
run_relay --
stopped_with "$RELAY_NAME.sig is not a signature file"
relay_never_ran
pass

case="a signature with another key: refused before the program runs"
reset_relay
make_relay "$V" other
run_relay --
stopped_with "$RELAY_NAME is not signed with 8 West's server key"
relay_never_ran
pass

case="the installer's signature on the relay's name: refused before the program runs"
reset_relay
make_relay "$V" wrongfile
run_relay --
stopped_with "$RELAY_NAME's signature was made for \"Plenipo_${V}_x64-setup.exe\", not for $RELAY_NAME"
relay_never_ran
pass

case="a signature for another version: refused before the program runs"
reset_relay
make_relay "$V" wrongversion
run_relay --
stopped_with "$RELAY_NAME was signed as version \"9.9.9\", but the release says $V"
relay_never_ran
pass

case="a file changed after it was signed: refused before the program runs"
reset_relay
make_relay "$V" tampered
run_relay --
stopped_with "$RELAY_NAME is not signed with 8 West's server key"
relay_never_ran
pass

case="a damaged download: refused before the program runs"
reset_relay
make_relay "$V" badsum
run_relay --
stopped_with "$RELAY_NAME does not match its checksum"
relay_never_ran
pass

case="a release with no .sig attached: refused"
reset_relay
make_relay "$V" good
publish "$V" yes "$RELAY_NAME" "$RELAY_NAME.sha256"
run_relay --
stopped_with "v$V has no $RELAY_NAME.sig attached"
relay_never_ran
pass

case="no trusted key: nothing is installed"
reset_relay
make_relay "$V" good
run_relay "$T/no-keys" --
stopped_with "No *.pub in $T/no-keys"
relay_never_ran
pass

case="a key folder that is not root's alone: nothing is installed"
reset_relay
install -d -m 0775 "$T/loose-keys"
cp "$T/relay-keys/8west-server.pub" "$T/loose-keys/"
run_relay "$T/loose-keys" --
stopped_with "$T/loose-keys must be owned by root and writable by root only"
relay_never_ran
chmod 0755 "$T/loose-keys"
chmod 0666 "$T/loose-keys/8west-server.pub"
run_relay "$T/loose-keys" --
stopped_with "$T/loose-keys/8west-server.pub must be owned by root and writable by root only"
relay_never_ran
pass

case="--check with a signed release changes nothing"
reset_relay
make_relay "$V" good
run_relay -- --check
[[ "$status" == 0 ]] || { cat "$T/out"; die "$case: exit $status"; }
grep -Fq "Check only" "$T/out" || { cat "$T/out"; die "$case: no check-only line"; }
relay_never_ran
pass

# ================================================================================================
say "The phone page's updater"

case="a signed release is served (the key as a minisign public key file)"
reset_page
make_page "$V" good
run_page --
[[ "$status" == 0 ]] || { cat "$T/out"; die "$case: exit $status"; }
grep -Fq "$PAGE_NAME is signed with 8 West's server key (8west-server.pub) for $V." "$T/out" || { cat "$T/out"; die "$case: no signature line"; }
grep -Fq "Serving v$V." "$T/out" || { cat "$T/out"; die "$case: not serving"; }
[[ "$(readlink "$T/page/current")" == "releases/$V" ]] || die "$case: current points elsewhere"
grep -Fxq "VERSION=$V" "$T/page/state/current.env" || die "$case: no VERSION in current.env"
curl -fsS "http://127.0.0.1:$PAGE_PORT/" | grep -Fq "Plenipo $V" || die "$case: the page is not served"
pass

case="an older latest release is refused on its own, and served with --version"
make_page "$OLDER" good
run_page --
stopped_with "GitHub's latest release is v$OLDER, older than the $V served"
[[ "$(readlink "$T/page/current")" == "releases/$V" ]] || die "$case: current moved"
run_page -- --version "$OLDER"
[[ "$status" == 0 ]] || { cat "$T/out"; die "$case: --version exit $status"; }
grep -Fq "Serving v$OLDER." "$T/out" || { cat "$T/out"; die "$case: not serving the older one"; }
curl -fsS "http://127.0.0.1:$PAGE_PORT/" | grep -Fq "Plenipo $OLDER" || die "$case: the older page is not served"
pass

case="no signature: refused before the page is unpacked"
reset_page
make_page "$V" garbage
run_page --
stopped_with "$PAGE_NAME.sig is not a signature file"
page_never_unpacked
pass

case="a signature with another key: refused before the page is unpacked"
reset_page
make_page "$V" other
run_page --
stopped_with "$PAGE_NAME is not signed with 8 West's server key"
page_never_unpacked
pass

case="the relay's signature on the page's name: refused before the page is unpacked"
reset_page
make_page "$V" wrongfile
run_page --
stopped_with "$PAGE_NAME's signature was made for \"plenipo-relay-$V-linux-x86_64\", not for $PAGE_NAME"
page_never_unpacked
pass

case="a signature for another version: refused before the page is unpacked"
reset_page
make_page "$V" wrongversion
run_page --
stopped_with "$PAGE_NAME was signed as version \"9.9.9\", but the release says $V"
page_never_unpacked
pass

case="a file changed after it was signed: refused before the page is unpacked"
reset_page
make_page "$V" tampered
run_page --
stopped_with "$PAGE_NAME is not signed with 8 West's server key"
page_never_unpacked
pass

case="no trusted key: nothing is served"
reset_page
make_page "$V" good
run_page "$T/no-keys" --
stopped_with "No *.pub in $T/no-keys"
page_never_unpacked
pass

case="the key as tauri signer generate prints it works for the page too"
reset_page
make_page "$V" good
run_page "$T/relay-keys" --
[[ "$status" == 0 ]] || { cat "$T/out"; die "$case: exit $status"; }
grep -Fq "Serving v$V." "$T/out" || { cat "$T/out"; die "$case: not serving"; }
pass

say "All checks passed: the updaters install only what 8 West's server key signed."
