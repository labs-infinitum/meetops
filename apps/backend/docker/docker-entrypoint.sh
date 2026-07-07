#!/bin/sh
set -e

# Started as root so we can fix ownership of the /data bind mount, then drop
# privileges — Chrome/xdotool must not run as root and must not write
# root-owned files into the host directory.
if [ "$(id -u)" = "0" ]; then
    if [ "$(stat -c %u /data)" != "$(id -u appuser)" ]; then
        chown -R appuser:appuser /data
    fi
    exec setpriv --reuid appuser --regid appuser --init-groups \
        env HOME=/home/appuser /usr/local/bin/google-meeting-recorder "$@"
fi

exec /usr/local/bin/google-meeting-recorder "$@"
