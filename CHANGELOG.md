# Changelog

## 0.2.0 – 2026-11-03 – Fedora 45

### Added

- **`extras/gdm-fingerprint-login`**: brings the fingerprint back to the GDM
  login screen on Fedora 45. GNOME Shell 51 connects the login screen to
  `fprintd` only to unlock an existing session
  (`if (!this._reauthOnly) return;` in `gdm/authServicesLegacy.js`), so the
  first login only asked for the password, while `sudo` and the lock screen
  kept working. The tool generates a copy of that file without the check from
  the installed gnome-shell and loads it only on the login screen through
  `G_RESOURCE_OVERLAYS`. A boot service regenerates it after GNOME Shell
  updates and disables the overlay if the patch no longer applies. No package
  is modified and no GNOME Shell code is shipped.
- `CHANGELOG.md`.

### Changed

- README: Fedora 45 setup step, tested system updated to Fedora 45
  (GNOME Shell 51.1, GDM 51.0, fprintd 1.94.5).
- `extras/keyring-tpm`: documented that it does not support **oo7**, the
  secret service that replaces gnome-keyring in Fedora 45.
- Driver crate version 0.2.0 (no changes to the driver code: it works on
  Fedora 45 as it is).

## 0.1.0 – 2026-10-27

First public version: fully open source Rust driver for the ELAN `04f3:0c57`
reader through libfprint-TOD, build/install/uninstall scripts in six
languages, `extras/keyring-tpm`, retry after a spurious finger wake on the
first touch after boot. Tested on Fedora 44.
