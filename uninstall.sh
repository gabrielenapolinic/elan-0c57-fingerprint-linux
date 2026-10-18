#!/bin/bash
# SPDX-License-Identifier: LGPL-2.1-or-later
#
# Removes what install.sh installed. Run as root: sudo ./uninstall.sh
# If install.sh made a backup of a previous libfprint, it is restored;
# otherwise fprintd goes back to the distribution libfprint.

set -euo pipefail

TOP="$(cd "$(dirname "$0")" && pwd)"
BUILD="$TOP/build"
MARKER_NAME=".elan-0c57-fingerprint-linux"
LDCONF="/etc/ld.so.conf.d/elan-0c57-fingerprint-linux.conf"

die () { echo "ERROR: $*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "run as root: sudo ./uninstall.sh"

PREFIX=/usr/local
LIBDIR=lib64
# shellcheck source=/dev/null
[ -f "$BUILD/stage.conf" ] && . "$BUILD/stage.conf"
DEST="$PREFIX/$LIBDIR"

[ -e "$DEST/libfprint-2/tod-1/$MARKER_NAME" ] || die "no installation found in $DEST"

rm -f "$DEST/libfprint-2/tod-1/libfprint-2-tod1-elan-0c57.so" \
      "$DEST/libfprint-2/tod-1/$MARKER_NAME" \
      "$DEST/libelancrypto.so" \
      "$DEST/libfprint-2-tod.so" "$DEST/libfprint-2-tod.so.1" \
      "$DEST/libfprint-2.so" "$DEST/libfprint-2.so.2" "$DEST/libfprint-2.so.2.0.0"
rmdir "$DEST/libfprint-2/tod-1" "$DEST/libfprint-2" 2>/dev/null || true

if [ -f "$BUILD/backup-path" ] && [ -d "$(cat "$BUILD/backup-path")" ]; then
  BACKUP="$(cat "$BUILD/backup-path")"
  echo "Restoring the previous libfprint from $BACKUP"
  cp -a "$BACKUP"/libfprint-2.so* "$DEST/"
else
  rm -f "$LDCONF"
fi

ldconfig
command -v restorecon >/dev/null && restorecon -R "$DEST"
systemctl stop fprintd.service 2>/dev/null || true

echo "Uninstalled. Prints enrolled with the ELAN driver can't be used by other"
echo "drivers, remove them with:  fprintd-delete \$USER"
