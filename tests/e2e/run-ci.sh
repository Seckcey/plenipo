#!/usr/bin/env bash
# The real-app tests on Linux CI, inside the run's own session bus (`dbus-run-session`).
#
# ADR-153: the Vault on Linux is the desktop's password store. The runner has none, so GNOME
# Keyring is started here, unlocked. It runs in the foreground (in this script's background):
# --daemonize sends the daemon's own messages to /dev/null, and twice the daemon was gone partway
# through a run with nothing said (every later suite: "password store is locked"). Its messages,
# and how it ended, go to $PLENIPO_E2E_KEYRING_LOG; tests/e2e/lib/app.mjs says whether it is still
# there each time a suite starts or closes Plenipo.
set -u
log="${PLENIPO_E2E_KEYRING_LOG:?set PLENIPO_E2E_KEYRING_LOG}"
mkdir -p "$(dirname "$log")"

gnome-keyring-daemon --components=secrets --foreground --unlock <<< plenipo-ci > /dev/null 2> "$log" &
keyring=$!
export PLENIPO_E2E_KEYRING_PID=$keyring

# Ready once it serves the password store on the bus (--daemonize returned only then).
for _ in $(seq 100); do
  if dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus.NameHasOwner string:org.freedesktop.secrets 2> /dev/null |
    grep -q "boolean true"; then
    break
  fi
  sleep 0.1
done

xvfb-run -a pnpm e2e
status=$?

if kill -0 "$keyring" 2> /dev/null; then
  echo "The keyring daemon ($keyring) was still running at the end." >> "$log"
  kill "$keyring"
else
  wait "$keyring"
  ended=$?
  echo "The keyring daemon ($keyring) had ended, with status $ended (above 128: ended by signal $((ended - 128)))." >> "$log"
fi
exit "$status"
