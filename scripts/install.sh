#!/usr/bin/env bash
# Builds and installs the game into the stock menu's Ports list. RGDS_HOST overrides the target.
set -euo pipefail
cd "$(dirname "$0")/.."
host="${RGDS_HOST:-root@anbernic.local}"
ports=/mnt/sdcard/PORTS
scripts/build-device.sh
ssh "$host" "mkdir -p $ports/twinbit $ports/Imgs"
scp -q target/aarch64-unknown-linux-gnu/release/twinbit "$host:$ports/twinbit/twinbit"
scp -q assets/Twinbit.sh "$host:$ports/Twinbit.sh"
scp -q assets/Twinbit.png "$host:$ports/Imgs/Twinbit.png"
echo "installed: Ports > Twinbit"
