#!/usr/bin/env bash
# Cross-compiles the device binary. Run scripts/sysroot.sh once first.
set -euo pipefail
cd "$(dirname "$0")/.."
sr="$PWD/.sysroot"
export PKG_CONFIG="$PWD/scripts/pkg-config" PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_SYSROOT_DIR="$sr" PKG_CONFIG_LIBDIR="$sr/usr/lib/pkgconfig"
export RUSTFLAGS="-L $sr/usr/lib"
cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.38 -p twinbit "$@"
