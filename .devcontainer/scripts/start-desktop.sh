#!/usr/bin/env bash
set -euo pipefail

/workspace/.devcontainer/scripts/cleanup-cargo-target.sh

export DISPLAY="${DISPLAY:-:1}"
export SCREEN_GEOMETRY="${SCREEN_GEOMETRY:-1440x900x24}"
mkdir -p "${XDG_RUNTIME_DIR:-/tmp/runtime-dir}"
chmod 700 "${XDG_RUNTIME_DIR:-/tmp/runtime-dir}"
if command -v dbus-daemon >/dev/null 2>&1 &&
  [[ ! -S "${XDG_RUNTIME_DIR:-/tmp/runtime-dir}/bus" ]]; then
  dbus-daemon --session --address="unix:path=${XDG_RUNTIME_DIR:-/tmp/runtime-dir}/bus" \
    --nofork --nopidfile >/tmp/dbus.log 2>&1 &
fi
if ! xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
  Xvfb "$DISPLAY" -screen 0 "$SCREEN_GEOMETRY" -nolisten tcp > /tmp/xvfb.log 2>&1 &
  for _ in {1..20}; do
    xdpyinfo -display "$DISPLAY" >/dev/null 2>&1 && break
    sleep 0.25
  done
fi
xdpyinfo -display "$DISPLAY" >/dev/null 2>&1 || {
  echo "Xvfb failed to start on $DISPLAY" >&2
  exit 1
}
pgrep -x openbox >/dev/null 2>&1 || openbox > /tmp/openbox.log 2>&1 &
if ! pgrep -f "x11vnc.*-rfbport 5900" >/dev/null 2>&1; then
  x11vnc -display "$DISPLAY" -N -forever -shared -nopw -rfbport 5900 > /tmp/x11vnc.log 2>&1 &
fi
if ! pgrep -f "websockify.*6080" >/dev/null 2>&1; then
  websockify --web=/usr/share/novnc 6080 127.0.0.1:5900 > /tmp/novnc.log 2>&1 &
fi
sleep 1
curl --fail --silent http://127.0.0.1:6080/vnc.html >/dev/null || {
  echo "noVNC failed to start on port 6080" >&2
  exit 1
}
python3 - <<'PY'
import socket

with socket.create_connection(("127.0.0.1", 5900), timeout=3) as connection:
    if not connection.recv(12).startswith(b"RFB "):
        raise SystemExit("VNC server did not return an RFB greeting")
PY
exec /workspace/scripts/run --watch > /tmp/desktop.log 2>&1
