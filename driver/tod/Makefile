# SPDX-License-Identifier: LGPL-2.1-or-later
# Builds the libfprint-TOD driver: the C adapter linked with the Rust library.
#
#   make TOD_SRC=<libfprint-TOD source tree> TOD_BUILD=<its meson build dir>
#
# build.sh at the top of the package passes its own libfprint-TOD; the
# defaults are those of a development checkout.

TOD_SRC   ?= $(HOME)/fprint-porting/libfprint-tod
TOD_BUILD ?= $(TOD_SRC)/build
DRIVER  := libfprint-2-tod1-elan0c57-rs.so
RUSTLIB := ../target/release/libelan_0c57.a
PKGS    := glib-2.0 gobject-2.0 gio-2.0 gmodule-2.0 gusb

CFLAGS  += -O2 -g -fPIC -Wall -Wno-unused-parameter \
           -I$(TOD_SRC)/libfprint -I$(TOD_BUILD)/libfprint -I$(TOD_BUILD) \
           $(shell pkg-config --cflags $(PKGS))
LDFLAGS += -shared -Wl,--version-script=export.ver -Wl,--gc-sections \
           -L$(TOD_BUILD)/libfprint/tod
LDLIBS  := -lfprint-2-tod $(shell pkg-config --libs $(PKGS)) -lusb-1.0 -lpthread -ldl -lm

all: $(DRIVER)

$(RUSTLIB): $(wildcard ../src/*.rs) ../Cargo.toml
	cd .. && cargo build --release --lib

$(DRIVER): elan0c57-tod.c elan0c57.h export.ver $(RUSTLIB)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ elan0c57-tod.c $(RUSTLIB) $(LDLIBS)

clean:
	rm -f $(DRIVER)

.PHONY: all clean
