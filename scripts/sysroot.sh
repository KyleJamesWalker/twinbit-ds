#!/usr/bin/env bash
# Pulls the device libraries the build links against into .sysroot/.
# ALSA_LIB names a local aarch64 libasound.so.2 to use instead, as in CI.
set -euo pipefail
host="${RGDS_HOST:-root@anbernic.local}"
root="$(cd "$(dirname "$0")/.." && pwd)/.sysroot"
mkdir -p "$root/usr/lib/pkgconfig"
if [ -n "${ALSA_LIB:-}" ]; then
  cp -L "$ALSA_LIB" "$root/usr/lib/libasound.so.2.0.0"
else
  scp -q "$host:/usr/lib/libasound.so.2.0.0" "$root/usr/lib/"
fi
ln -sf libasound.so.2.0.0 "$root/usr/lib/libasound.so.2"
ln -sf libasound.so.2 "$root/usr/lib/libasound.so"
cat > "$root/usr/lib/pkgconfig/alsa.pc" <<PC
prefix=/usr
libdir=\${prefix}/lib
includedir=\${prefix}/include
Name: alsa
Description: ALSA from the device
Version: 1.2.11
Libs: -L\${libdir} -lasound
Cflags: -I\${includedir}
PC
echo "sysroot ready: $root"
