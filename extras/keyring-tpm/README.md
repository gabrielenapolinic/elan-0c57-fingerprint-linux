# keyring-tpm: unlock the GNOME keyring after a fingerprint login

Logging in with a fingerprint gives GNOME no password, so the "login" keyring
stays locked and an "Authentication required" dialog appears as soon as an
application needs a stored secret (browser, Wi-Fi, e-mail...).

`keyring-tpm` seals the **login password** (which is also the keyring
password) in the TPM with `systemd-creds` (host key + TPM2, bound to PCR 7,
i.e. the Secure Boot state). A user service runs at the start of the
graphical session, gets the password back from the TPM and unlocks the
keyring over D-Bus. The fingerprint therefore works exactly like the login
password. PAM is not modified.

## Requirements

- GNOME with gnome-keyring, systemd ≥ 256 (`systemd-creds --user`), a TPM2 chip
- `/usr/bin/python3` with PyGObject (`python3-gobject` on Fedora)

## Install

```sh
install -Dm755 keyring-tpm ~/.local/bin/keyring-tpm
install -Dm644 keyring-tpm.service ~/.config/systemd/user/keyring-tpm.service
keyring-tpm setup      # asks for the login password once
```

`setup` checks the password against the keyring (without changing it), seals
it in the TPM, locks the keyring and reopens it from the TPM as a test, then
enables the service. From the next boot, log in with the fingerprint: the
keyring is unlocked without any dialog.

## Commands

| Command | What it does |
|---|---|
| `setup` | seal the login password in the TPM and enable automatic unlocking |
| `update` | same as `setup`; run it after changing the login password, or if the TPM no longer releases the secret (BIOS update, Secure Boot change) |
| `unlock` | unlock the keyring (used by the service at login) |
| `status` | show whether the keyring is locked, the secret exists and the service is enabled |
| `remove` | disable the service and delete the sealed secret |

If unlocking fails at login, a desktop notification explains what to do:
unlock the keyring once by hand with the login password, then run
`keyring-tpm update`.

## Security

The secret is stored in `~/.config/keyring-tpm/login.cred` (mode 600) and can
only be decrypted on this machine, with the TPM in the same Secure Boot state.
However, any program running as your user can ask `systemd-creds` to decrypt
it, and it is the same password used for `sudo`. This is the price for having
the fingerprint behave like the password; if you don't accept it, give the
keyring a separate password instead.
