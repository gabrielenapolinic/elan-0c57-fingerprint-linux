#!/bin/bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Tries the driver with libfprint's example programs, without installing it.
#   ./try.sh enroll    enroll the right index (12 touches)
#   ./try.sh verify [N]  verify it N times in a row (default 1)
# Prints are kept in try/test-storage.variant.
set -euo pipefail
cd "$(dirname "$0")"
TOD_SRC="${TOD_SRC:-$HOME/fprint-porting/libfprint-tod}"
TOD_BUILD="${TOD_BUILD:-$TOD_SRC/build}"
export TOD_SRC TOD_BUILD
action="${1:?usage: $0 enroll|verify [N]}"
times="${2:-1}"
make -s
mkdir -p try/drivers
cp libfprint-2-tod1-elan0c57-rs.so try/drivers/
cd try
export FP_TOD_DRIVERS_DIR="$PWD/drivers"
export LD_LIBRARY_PATH="$TOD_BUILD/libfprint:$TOD_BUILD/libfprint/tod"
# Show the scores of the driver (threshold 0.86).
export G_MESSAGES_DEBUG="${G_MESSAGES_DEBUG:-libfprint-elan0c57rs}"
# [6] = right index; then answer "verify again?" N-1 times yes, then no.
{ printf '6\n'; for ((i = 1; i < times; i++)); do printf 'y\n'; done; printf 'n\n'; } \
  | "$TOD_BUILD/examples/$action"
