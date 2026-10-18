#!/bin/bash
# SPDX-License-Identifier: LGPL-2.1-or-later
#
# Builds everything needed for the ELAN 04f3:0c57 fingerprint reader, as a
# normal user, into ./build/stage. Nothing is installed: run install.sh as root
# afterwards.
#
#  1. libfprint-TOD (libfprint able to load binary "Touch OEM Drivers")
#  2. the ELAN TOD driver for 04f3:0c4b (Lenovo), downloaded from its original
#     source, verified and patched for the 0c57
#  3. libelancrypto.so: OpenSSL 1.1 compatibility shim + LED support
#
# Environment overrides: PREFIX (default /usr/local), LIBDIR (default lib64)

set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
LIBDIR="${LIBDIR:-lib64}"

TOD_REPO="https://gitlab.freedesktop.org/3v1n0/libfprint.git"
TOD_BRANCH="tod"
TOD_COMMIT="17b3bfda0174c0034f9dc8b11d67339e46eb928c"      # libfprint-TOD v1.95.1+tod1

DRIVER_REPO="https://github.com/TonyHoyle/libfprint-2-tod1-elan"
DRIVER_COMMIT="f2aabc25b1cffa28b69bebefeccf772910c51a84"
DRIVER_PATH="usr/lib/x86_64-linux-gnu/libfprint-2/tod-1/libfprint-2-tod1-elan.so"
DRIVER_SHA256="34087826dee386a2919e8b0cd2663d1c249be1f8f1a3951bc1f2c88038d30605"
PATCHED_SHA256="87ee037cc85fee844b54ab867fed07376a963a2bf2d990f5b8acd0b9015a8677"
DRIVER_NAME="libfprint-2-tod1-elan-0c57.so"

TOP="$(cd "$(dirname "$0")" && pwd)"
BUILD="$TOP/build"
STAGE="$BUILD/stage"
STAGE_LIB="$STAGE$PREFIX/$LIBDIR"

die () { echo "ERROR: $*" >&2; exit 1; }
step () { echo; echo "==> $*"; }

[ "$(id -u)" -ne 0 ] || die "run build.sh as a normal user, not as root"
[ "$(uname -m)" = "x86_64" ] || die "the ELAN driver is only available for x86_64"

for tool in git gcc meson ninja pkg-config sha256sum dd cmp; do
  command -v "$tool" >/dev/null || die "missing tool: $tool"
done
for pc in glib-2.0 gusb gio-unix-2.0 gmodule-2.0 openssl; do
  pkg-config --exists "$pc" || die "missing development package for '$pc' (see README.md)"
done

mkdir -p "$BUILD"

# --- 1. libfprint-TOD -------------------------------------------------------
step "Fetching libfprint-TOD ($TOD_COMMIT)"
if [ ! -d "$BUILD/libfprint-tod/.git" ]; then
  git clone --branch "$TOD_BRANCH" "$TOD_REPO" "$BUILD/libfprint-tod"
fi
git -C "$BUILD/libfprint-tod" fetch --quiet origin "$TOD_BRANCH"
git -C "$BUILD/libfprint-tod" checkout --quiet --detach "$TOD_COMMIT"

step "Building libfprint-TOD (prefix $PREFIX, libdir $LIBDIR)"
# Only the virtual_image driver is built in: the ELAN reader is handled by the
# TOD driver, and the in-tree elan driver must not claim the device.
rm -rf "$BUILD/libfprint-tod/_build"
meson setup "$BUILD/libfprint-tod/_build" "$BUILD/libfprint-tod" \
  --prefix="$PREFIX" --libdir="$LIBDIR" --buildtype=release \
  -Ddrivers=virtual_image -Ddoc=false -Dgtk-examples=false \
  -Dudev_rules=disabled -Dudev_hwdb=disabled -Dintrospection=false \
  -Dinstalled-tests=false >/dev/null
ninja -C "$BUILD/libfprint-tod/_build"

rm -rf "$STAGE"
DESTDIR="$STAGE" meson install -C "$BUILD/libfprint-tod/_build" --no-rebuild >/dev/null
# headers, pkg-config files and docs are not needed at runtime
rm -rf "$STAGE$PREFIX/include" "$STAGE_LIB/pkgconfig" "$STAGE$PREFIX/share" "$STAGE$PREFIX/libexec"

# --- 2. ELAN TOD driver -----------------------------------------------------
step "Fetching the ELAN TOD driver ($DRIVER_COMMIT)"
if [ ! -d "$BUILD/elan-driver/.git" ]; then
  git clone "$DRIVER_REPO" "$BUILD/elan-driver"
fi
git -C "$BUILD/elan-driver" checkout --quiet --detach "$DRIVER_COMMIT"
ORIG="$BUILD/elan-driver/$DRIVER_PATH"
echo "$DRIVER_SHA256  $ORIG" | sha256sum --check --quiet || die "unexpected driver binary (checksum mismatch)"

step "Patching the driver for the 0c57"
OUT="$BUILD/$DRIVER_NAME"
cp "$ORIG" "$OUT"

# patch FILE OFFSET EXPECTED NEW: replaces EXPECTED (checked) with NEW at OFFSET
patch_bytes () {
  local file="$1" offset="$2" expected="$3" new="$4"
  [ "${#expected}" -eq "${#new}" ] || die "internal error: patch length mismatch"
  local found
  found="$(dd if="$file" bs=1 skip="$offset" count="${#expected}" status=none)"
  [ "$found" = "$expected" ] || die "unexpected bytes at offset $offset"
  printf '%s' "$new" | dd of="$file" bs=1 seek="$offset" conv=notrunc status=none
}

# a) USB id table: second entry 04f3:0c4b -> 04f3:0c57 (0x35630, low byte of the pid)
[ "$(dd if="$OUT" bs=1 skip=$((0x35630)) count=1 status=none | od -An -tx1 | tr -d ' ')" = "4b" ] \
  || die "unexpected id table content"
printf '\x57' | dd of="$OUT" bs=1 seek=$((0x35630)) conv=notrunc status=none
# b) DT_NEEDED libcrypto.so.1.1 -> libelancrypto.so (our OpenSSL 1.1 shim)
patch_bytes "$OUT" 12997 "libcrypto.so.1.1" "libelancrypto.so"
# c) import g_usb_device_bulk_transfer_async -> e_usb_... (LED hook in the shim)
patch_bytes "$OUT" 14704 "g_usb_device_bulk_transfer_async" "e_usb_device_bulk_transfer_async"

echo "$PATCHED_SHA256  $OUT" | sha256sum --check --quiet || die "patched driver checksum mismatch"

# --- 3. libelancrypto.so ----------------------------------------------------
step "Building libelancrypto.so"
gcc -shared -fPIC -O2 -Wall -o "$BUILD/libelancrypto.so" \
  -Wl,-soname,libelancrypto.so -Wl,--version-script="$TOP/src/elancrypto.ver" \
  "$TOP/src/crypto-compat.c" "$TOP/src/elan-led.c" \
  $(pkg-config --cflags --libs gusb) -ldl

# --- stage ------------------------------------------------------------------
install -Dm755 "$BUILD/libelancrypto.so" "$STAGE_LIB/libelancrypto.so"
install -Dm755 "$OUT" "$STAGE_LIB/libfprint-2/tod-1/$DRIVER_NAME"
printf 'PREFIX=%s\nLIBDIR=%s\n' "$PREFIX" "$LIBDIR" > "$BUILD/stage.conf"

step "Done. Files ready to be installed:"
(cd "$STAGE" && find . \( -type f -o -type l \) | sed 's|^\.||' | sort)
echo
echo "Now run:  sudo ./install.sh"
