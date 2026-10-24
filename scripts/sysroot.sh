#!/usr/bin/env bash
# Pulls the device libraries the build links against into .sysroot/.
set -euo pipefail
host="${RGDS_HOST:-root@anbernic.local}"
root="$(cd "$(dirname "$0")/.." && pwd)/.sysroot"
mkdir -p "$root/usr/lib/pkgconfig"
scp -q "$host:/usr/lib/libasound.so.2.0.0" "$root/usr/lib/"
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
