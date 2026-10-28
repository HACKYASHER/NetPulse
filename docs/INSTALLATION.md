# NetPulse installation layout

The Windows installer uses one protected root: `C:\Program Files\NetPulse`
(the actual Windows Program Files location may differ).

| Location under NetPulse | Contents |
|---|---|
| `NetPulse.exe`, `netpulsed.exe`, Qt libraries | Client and controller |
| `cores` | Shipped, verified engine bundle |
| `service` | Administrator-protected TUN service and its engines |
| `data\<Windows SID>` | That user's settings, encrypted subscriptions, logs, active engines, downloads and backups |
| `docs`, `licenses` | Documentation and license notices |
| `NetPulse-Maintenance.exe`, `Uninstall.exe` | Repair/update and removal |

Installation requests Windows administrator approval. Normal use does not.
Only the profile subdirectory is writable by its user. Other users cannot read
that profile; service executables and DLLs remain protected. The service retains
its existing owner identity; sharing the installation does not grant another
account control of the first user's TUN session.

On first launch by the original Windows account, NetPulse locks the former
`%LOCALAPPDATA%\NetPulseQt` profile, copies it into the new root, compares file
hashes and checks the database before activating the copy. DPAPI secrets remain
under the same Windows account. A failed check preserves the source. Only source
files that still match the verified copy are removed; changed/blocked files are
preserved. The old location contains a `migrated-to.txt` pointer afterward.
Known files of the previous per-user installation are cleaned on first launch;
unknown files and inaccessible files are preserved, never forcibly deleted.

The service's old Program Files location is accepted for migration only. The new
copy is verified before SCM is changed to `NetPulse\service`. Its old files are
removed after readback confirms the new path. A failure keeps the prior service
or both copies, rather than deleting the registered executable.

Settings → Data and backups shows both paths and provides folder buttons.
Update, repair and uninstall preserve `data`; uninstall removes program files
and the registered service. Personal data can be removed separately through the
client's existing data controls.

If an engine is missing, its precise path and Windows error are shown. Startup
does not repeatedly recreate a missing activated engine. Explicit **Restore
bundled engines** works offline. Explicit installation of a selected release
fills missing companion components from the verified shipped bundle, keeps
present components, validates the selected version and preserves rollback.
Protection/quarantine decisions are not changed by these operations.

Portable ZIP users keep data beside their extracted client as well. Enabling
TUN from a portable copy still requires the protected service installation.
