#!/bin/bash
# SPDX-License-Identifier: LGPL-2.1-or-later
#
# Installs the files prepared by build.sh. Run as root: sudo ./install.sh
#
# - any libfprint already present in $PREFIX/$LIBDIR (not installed by this
#   project) is backed up first, and the backup is verified before anything
#   is overwritten
# - $PREFIX/$LIBDIR is added to the dynamic linker path if needed, so that
#   fprintd picks libfprint-TOD instead of the distribution libfprint
# - fprintd is stopped; it is restarted on demand (D-Bus activation)

set -euo pipefail

TOP="$(cd "$(dirname "$0")" && pwd)"
BUILD="$TOP/build"
MARKER_NAME=".elan-0c57-fingerprint-linux"
LDCONF="/etc/ld.so.conf.d/elan-0c57-fingerprint-linux.conf"

die () { echo "ERROR: $*" >&2; exit 1; }
step () { echo; echo "==> $*"; }

[ "$(id -u)" -eq 0 ] || die "run as root: sudo ./install.sh"
[ -f "$BUILD/stage.conf" ] || die "nothing to install, run ./build.sh first (as a normal user)"
# shellcheck source=/dev/null
. "$BUILD/stage.conf"

STAGE_LIB="$BUILD/stage$PREFIX/$LIBDIR"
DEST="$PREFIX/$LIBDIR"
MARKER="$DEST/libfprint-2/tod-1/$MARKER_NAME"
[ -f "$STAGE_LIB/libfprint-2.so.2.0.0" ] || die "incomplete build, run ./build.sh again"

# --- backup of a foreign libfprint ------------------------------------------
if [ -e "$DEST/libfprint-2.so.2.0.0" ] && [ ! -e "$MARKER" ]; then
  BACKUP="$DEST/libfprint-backup-$(date +%Y%m%d-%H%M%S)"
  step "Backing up the existing libfprint in $DEST to $BACKUP"
  mkdir -p "$BACKUP"
  for f in "$DEST"/libfprint-2.so*; do
    cp -a "$f" "$BACKUP/"
  done
  # never go on without a complete backup
  for f in "$DEST"/libfprint-2.so*; do
    if [ -L "$f" ]; then
      [ "$(readlink "$f")" = "$(readlink "$BACKUP/$(basename "$f")")" ] || die "backup check failed for $f"
    else
      cmp -s "$f" "$BACKUP/$(basename "$f")" || die "backup check failed for $f"
    fi
  done
  echo "$BACKUP" > "$TOP/build/backup-path"
  echo "backup verified"
fi

# --- install ----------------------------------------------------------------
step "Installing into $DEST"
# remove the old symlinks first, cp would otherwise write through them
rm -f "$DEST/libfprint-2.so" "$DEST/libfprint-2.so.2" "$DEST/libfprint-2-tod.so"
cp -a --no-preserve=ownership "$STAGE_LIB/." "$DEST/"
touch "$MARKER"

if ! grep -qsxF "$DEST" /etc/ld.so.conf /etc/ld.so.conf.d/*.conf; then
  step "Adding $DEST to the dynamic linker path ($LDCONF)"
  echo "$DEST" > "$LDCONF"
fi
ldconfig

if command -v restorecon >/dev/null; then
  restorecon -R "$DEST"            # SELinux labels (Fedora)
fi

systemctl stop fprintd.service 2>/dev/null || true

# --- check ------------------------------------------------------------------
step "Check"
FPRINTD="$(command -v fprintd 2>/dev/null || true)"
for f in /usr/libexec/fprintd /usr/lib/fprintd/fprintd "$FPRINTD"; do
  [ -n "$f" ] && [ -x "$f" ] && FPRINTD="$f" && break
done
if [ -n "$FPRINTD" ]; then
  ldd "$FPRINTD" | grep -E 'libfprint-2(-tod)?\.so'
  ldd "$FPRINTD" | grep -q "$DEST/libfprint-2.so.2" \
    || echo "WARNING: fprintd does not use $DEST/libfprint-2.so.2"
else
  echo "WARNING: fprintd not found, install it with your package manager"
fi

echo
echo "Installed. Now enroll a finger as your user (18 touches):"
echo "  fprintd-enroll -f right-index-finger"
echo "and test it with:"
echo "  fprintd-verify"
