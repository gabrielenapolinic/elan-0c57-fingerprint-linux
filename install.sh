#!/bin/bash
# SPDX-License-Identifier: LGPL-2.1-or-later
#
# Installs the files prepared by build.sh. Run as root: sudo ./install.sh
#
# - any libfprint already present in $PREFIX/$LIBDIR (not installed by this
#   project) is backed up first, and the backup is verified before anything
#   is overwritten
# - the files of ELAN's proprietary driver, installed by earlier versions of
#   this project, are removed
# - $PREFIX/$LIBDIR is added to the dynamic linker path if needed, so that
#   fprintd picks libfprint-TOD instead of the distribution libfprint
# - fprintd is stopped; it is restarted on demand (D-Bus activation)

set -euo pipefail

TOP="$(cd "$(dirname "$0")" && pwd)"
BUILD="$TOP/build"
MARKER_NAME=".elan-0c57-fingerprint-linux"
LDCONF="/etc/ld.so.conf.d/elan-0c57-fingerprint-linux.conf"

# shellcheck source=lib/i18n.sh
. "$TOP/lib/i18n.sh"

[ "$(id -u)" -eq 0 ] || die run_as_root "./install.sh"
[ -f "$BUILD/stage.conf" ] || die not_built
# shellcheck source=/dev/null
. "$BUILD/stage.conf"

STAGE_LIB="$BUILD/stage$PREFIX/$LIBDIR"
DEST="$PREFIX/$LIBDIR"
MARKER="$DEST/libfprint-2/tod-1/$MARKER_NAME"
[ -f "$STAGE_LIB/libfprint-2.so.2.0.0" ] || die incomplete
[ -f "$STAGE_LIB/libfprint-2/tod-1/libfprint-2-tod1-elan0c57-rs.so" ] || die incomplete

# --- backup of a foreign libfprint ------------------------------------------
if [ -e "$DEST/libfprint-2.so.2.0.0" ] && [ ! -e "$MARKER" ]; then
  BACKUP="$DEST/libfprint-backup-$(date +%Y%m%d-%H%M%S)"
  step backup "$DEST" "$BACKUP"
  mkdir -p "$BACKUP"
  for f in "$DEST"/libfprint-2.so*; do
    cp -a "$f" "$BACKUP/"
  done
  # never go on without a complete backup
  for f in "$DEST"/libfprint-2.so*; do
    if [ -L "$f" ]; then
      [ "$(readlink "$f")" = "$(readlink "$BACKUP/$(basename "$f")")" ] || die backup_failed "$f"
    else
      cmp -s "$f" "$BACKUP/$(basename "$f")" || die backup_failed "$f"
    fi
  done
  echo "$BACKUP" > "$TOP/build/backup-path"
  msg backup_ok
fi

# a running fprintd crashes if its libraries change under it; it is
# restarted on demand (D-Bus activation)
systemctl stop fprintd.service 2>/dev/null || true

# --- ELAN's proprietary driver (earlier versions) ---------------------------
LEGACY=(
  "$DEST/libfprint-2/tod-1/libfprint-2-tod1-elan-0c57.so"
  "$DEST/libfprint-2/disabled/libfprint-2-tod1-elan-0c57.so"
  "$DEST/libelancrypto.so"
)
for f in "${LEGACY[@]}"; do
  if [ -e "$f" ]; then
    [ -n "${legacy_found:-}" ] || step legacy
    legacy_found=1
    rm -f "$f"
    msg removed "$f"
  fi
done
rmdir "$DEST/libfprint-2/disabled" 2>/dev/null || true

# --- install ----------------------------------------------------------------
step installing "$DEST"
# remove the old symlinks first, cp would otherwise write through them
rm -f "$DEST/libfprint-2.so" "$DEST/libfprint-2.so.2" "$DEST/libfprint-2-tod.so"
cp -a --no-preserve=ownership "$STAGE_LIB/." "$DEST/"
touch "$MARKER"

if ! grep -qsxF "$DEST" /etc/ld.so.conf /etc/ld.so.conf.d/*.conf; then
  step ldconf "$DEST" "$LDCONF"
  echo "$DEST" > "$LDCONF"
fi
ldconfig

if command -v restorecon >/dev/null; then
  restorecon -R "$DEST"            # SELinux labels (Fedora)
fi

# --- check ------------------------------------------------------------------
step check
FPRINTD="$(command -v fprintd 2>/dev/null || true)"
for f in /usr/libexec/fprintd /usr/lib/fprintd/fprintd "$FPRINTD"; do
  [ -n "$f" ] && [ -x "$f" ] && FPRINTD="$f" && break
done
if [ -n "$FPRINTD" ]; then
  ldd "$FPRINTD" | grep -E 'libfprint-2(-tod)?\.so'
  ldd "$FPRINTD" | grep -q "$DEST/libfprint-2.so.2" \
    || msg fprintd_other_lib "$DEST/libfprint-2.so.2"
else
  msg no_fprintd
fi

echo
msg installed
echo "  fprintd-enroll -f right-index-finger"
msg then_test
echo "  fprintd-verify"
echo
[ -n "${legacy_found:-}" ] && msg old_prints
msg password
