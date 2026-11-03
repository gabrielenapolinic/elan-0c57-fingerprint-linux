# gdm-fingerprint-login: fingerprint on the login screen (Fedora 45)

After upgrading to **Fedora 45** the fingerprint still works for `sudo` and
for the lock screen, but the **GDM login screen no longer offers it**. The
driver is not involved: `fprintd` is simply never contacted at login.

## Why

GNOME Shell 51 (Fedora 45) connects the login screen to `fprintd` only to
unlock an existing session (reauthentication). In
`gdm/authServicesLegacy.js`:

```js
_connectFingerprintManager() {
    if (!this._reauthOnly)
        return;
```

PAM (`gdm-fingerprint`), `authselect ... with-fingerprint` and the dconf key
`enable-fingerprint-authentication` are all still correct; it is only this
check that stops the first login. It is probably intentional: a fingerprint
gives no password to unlock the keyring (see [Keyring](#keyring)).

## What the tool does

`gdm-fingerprint-login install`:

1. extracts `authServicesLegacy.js` from the **installed** gnome-shell
   (`/usr/lib64/gnome-shell/libshell-*.so`) and removes those two lines; the
   result is checked, and nothing is done if the code is not exactly as
   expected;
2. stores the copy in `/usr/local/share/elan-0c57-fingerprint-linux/gdm-fingerprint-login/`;
3. adds a drop-in for the login screen's shell,
   `/etc/systemd/user/org.gnome.Shell@gdm.service.d/elan-0c57-fingerprint-login.conf`,
   which sets `G_RESOURCE_OVERLAYS` so that only that file is replaced, only on
   the login screen;
4. enables `gdm-fingerprint-login.service`, which runs before GDM at every
   boot: after a gnome-shell update the copy is generated again from the new
   version; if the patch no longer applies, the overlay is removed and the
   login screen goes back to the stock behaviour instead of breaking.

No package is modified, and no GNOME Shell code is shipped in this
repository: the file is always generated from the installed gnome-shell.

## Install

Requires `gresource` (`sudo dnf install glib2-devel`) and `/usr/bin/python3`.

```sh
sudo ./gdm-fingerprint-login install
```

Log out (or reboot) and touch the reader on the login screen. Other commands:

```sh
gdm-fingerprint-login status
sudo gdm-fingerprint-login refresh     # regenerate now, normally done at boot
sudo gdm-fingerprint-login uninstall
```

`install` also removes the first, manual version of this fix
(`fingerprint-login.conf` and `/usr/local/share/gdm-fingerprint-fix/`), if
present.

If the login screen ever fails to start, switch to a text console
(Ctrl+Alt+F3), log in and run `sudo gdm-fingerprint-login uninstall`.

## Keyring

Fedora 45 replaced gnome-keyring with **oo7** (`pam_oo7`). After a
fingerprint login oo7 receives no password, so the login keyring stays locked
until it is unlocked with the password. [`keyring-tpm`](../keyring-tpm)
targets gnome-keyring and does **not** work with oo7 yet.

## Tested on

Fedora 45, GNOME Shell 51.1, GDM 51.0, fprintd 1.94.5.
