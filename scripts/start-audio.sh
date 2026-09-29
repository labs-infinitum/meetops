#!/bin/sh
# Best-effort: give Chrome a real (null) PulseAudio output. Without any audio device
# Chromium plays WebRTC audio into its fake output, which makes tab audio capture
# unreliable. Sourced by the worker entrypoints; never fails the container.
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/runtime-$(id -u)}"
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
if command -v pulseaudio >/dev/null 2>&1; then
  if pulseaudio --daemonize=yes --exit-idle-time=-1 --log-target=stderr \
    && pactl load-module module-null-sink sink_name=meetops_null >/dev/null \
    && pactl set-default-sink meetops_null; then
    echo "PulseAudio null sink ready"
  else
    echo "WARNING: PulseAudio setup failed; Chrome falls back to its fake audio output"
  fi
fi
