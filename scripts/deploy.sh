#!/usr/bin/env bash
# Builds, copies to the device, and (re)starts the game. RGDS_HOST overrides the target.
set -euo pipefail
cd "$(dirname "$0")/.."
host="${RGDS_HOST:-root@anbernic.local}"
dir=/userdata/twinbit
scripts/build-device.sh
ssh "$host" "mkdir -p $dir"
scp -q scripts/device-run.sh "$host:$dir/run.sh"
scp -q target/aarch64-unknown-linux-gnu/release/twinbit "$host:$dir/twinbit.new"
ssh "$host" "cd $dir; if pidof twinbit >/dev/null; then touch /tmp/twinbit.restart; killall twinbit; else CAPTURE=${CAPTURE:-} DEBUG=${DEBUG:-} nohup ./run.sh >/dev/null 2>&1 </dev/null & fi"
echo "deployed; logs: ssh $host tail -f $dir/run.log"
