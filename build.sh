#!/bin/bash
# SPDX-License-Identifier: LGPL-2.1-or-later
#
# Builds everything needed for the ELAN 04f3:0c57 fingerprint reader, as a
# normal user, into ./build/stage. Nothing is installed: run install.sh as root
# afterwards.
#
#  1. libfprint-TOD: libfprint able to load drivers built outside of it
#  2. the open source driver in driver/: a Rust library (sensor protocol,
#     image processing, matching) and a small C adapter for libfprint-TOD
#
# Environment overrides: PREFIX (default /usr/local), LIBDIR (default lib64)

set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
LIBDIR="${LIBDIR:-lib64}"

TOD_REPO="https://gitlab.freedesktop.org/3v1n0/libfprint.git"
TOD_BRANCH="tod"
TOD_COMMIT="17b3bfda0174c0034f9dc8b11d67339e46eb928c"      # libfprint-TOD v1.95.1+tod1

DRIVER_NAME="libfprint-2-tod1-elan0c57-rs.so"

TOP="$(cd "$(dirname "$0")" && pwd)"
BUILD="$TOP/build"
STAGE="$BUILD/stage"
STAGE_LIB="$STAGE$PREFIX/$LIBDIR"
TOD_SRC="$BUILD/libfprint-tod"
TOD_BUILD="$TOD_SRC/_build"

# shellcheck source=lib/i18n.sh
. "$TOP/lib/i18n.sh"

[ "$(id -u)" -ne 0 ] || die run_as_user "build.sh"
for tool in git gcc make meson ninja pkg-config cargo strip; do
  command -v "$tool" >/dev/null || die missing_tool "$tool"
done
for pc in glib-2.0 gusb gio-unix-2.0 gmodule-2.0 libusb-1.0; do
  pkg-config --exists "$pc" || die missing_pkg "$pc"
done

mkdir -p "$BUILD"

# --- 1. libfprint-TOD -------------------------------------------------------

step fetch_tod "$TOD_COMMIT"
if [ ! -d "$TOD_SRC/.git" ]; then
  git clone --branch "$TOD_BRANCH" "$TOD_REPO" "$TOD_SRC"
fi
git -C "$TOD_SRC" fetch --quiet origin "$TOD_BRANCH"
git -C "$TOD_SRC" checkout --quiet --detach "$TOD_COMMIT"

step build_tod "$PREFIX" "$LIBDIR"
# Only the virtual_image driver is built in: the reader is handled by the TOD
# driver, and the in-tree elan driver must not claim the device.
rm -rf "$TOD_BUILD"
meson setup "$TOD_BUILD" "$TOD_SRC" \
  --prefix="$PREFIX" --libdir="$LIBDIR" --buildtype=release \
  -Ddrivers=virtual_image -Ddoc=false -Dgtk-examples=false \
  -Dudev_rules=disabled -Dudev_hwdb=disabled -Dintrospection=false \
  -Dinstalled-tests=false >/dev/null
ninja -C "$TOD_BUILD"

rm -rf "$STAGE"
DESTDIR="$STAGE" meson install -C "$TOD_BUILD" --no-rebuild >/dev/null
# headers, pkg-config files and docs are not needed at runtime
rm -rf "$STAGE$PREFIX/include" "$STAGE_LIB/pkgconfig" "$STAGE$PREFIX/share" "$STAGE$PREFIX/libexec"

# --- 2. open source driver --------------------------------------------------

step build_driver
(cd "$TOP/driver" && cargo build --release --lib)
make -C "$TOP/driver/tod" --no-print-directory TOD_SRC="$TOD_SRC" TOD_BUILD="$TOD_BUILD" -B
install -Dm755 "$TOP/driver/tod/$DRIVER_NAME" "$STAGE_LIB/libfprint-2/tod-1/$DRIVER_NAME"
strip --strip-debug "$STAGE_LIB/libfprint-2/tod-1/$DRIVER_NAME"

# --- stage ------------------------------------------------------------------

printf 'PREFIX=%s\nLIBDIR=%s\n' "$PREFIX" "$LIBDIR" > "$BUILD/stage.conf"

step build_done
(cd "$STAGE" && find . \( -type f -o -type l \) | sed 's|^\.||' | sort)
echo
msg next_install
