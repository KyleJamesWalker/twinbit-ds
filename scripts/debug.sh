#!/usr/bin/env bash
# Drives a game started with `DEBUG=1 scripts/deploy.sh`. RGDS_HOST overrides the target.
#   scripts/debug.sh shot [out.png]      save both screens to out.png (default: shot.png)
#   scripts/debug.sh logs                follow the game log
#   scripts/debug.sh <command...>        send a command: press a, tap 500 400, swipe 200 600 700 200, scene dice
set -euo pipefail
host="${RGDS_HOST:-root@anbernic.local}"
pipe=/tmp/twinbit.ctl
send() { ssh "$host" "[ -p $pipe ] || { echo 'game not in debug mode' >&2; exit 1; }; timeout 2 sh -c 'echo \"$*\" > $pipe'"; }
case "${1:-}" in
  shot)
    out="${2:-shot.png}"
    remote=/tmp/twinbit-shot.png
    ssh "$host" "rm -f $remote"
    send shot "$remote"
    ssh "$host" "for i in \$(seq 50); do [ -f $remote ] && exit 0; sleep 0.1; done; echo 'no screenshot' >&2; exit 1"
    scp -q "$host:$remote" "$out"
    echo "$out"
    ;;
  logs) ssh "$host" tail -f /userdata/twinbit/run.log ;;
  "") sed -n '2,5p' "$0" ;;
  *) send "$@" ;;
esac
