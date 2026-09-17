#!/usr/bin/env bash
set -euxo pipefail

export DISPLAY=:1
Xvfb "$DISPLAY" -screen 0 "${SCREEN_GEOMETRY:-1440x900x24}" -nolisten tcp &
openbox &
x11vnc -display "$DISPLAY" -N -forever -shared -nopw -rfbport 5900 &
exec websockify --web=/usr/share/novnc 6080 127.0.0.1:5900
