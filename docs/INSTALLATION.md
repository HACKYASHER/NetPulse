# NetPulse installation layout

Choose the installation folder on the Windows installer's directory page.
`C:\Program Files\NetPulse` is the default, and another local folder such as
`D:\NetPulse` can be selected. The client, engines, service and personal data
remain under that selected root. Program files and service executables are
administrator-owned; a protected parent path is required for the privileged
service. System folders, volume roots, links, network/removable drives and paths
whose writable parents could replace the service are rejected with an explanation.
New parent directories created by the installer are protected as well.

| Relative path | Contents |
|---|---|
| NetPulse.exe, netpulsed.exe and Qt libraries | Client and controller |
| cores | Official Xray 26.10.21 and unchanged official sing-box 1.14.4 bundle |
| service | Protected service and engine copies |
| data\<Windows SID> | Private settings, encrypted profiles, active engines, logs and backups |
| docs, licenses | Documentation and original redistribution notices |
| NetPulse-Maintenance.exe, Uninstall.exe | Repair/update and uninstall |

The machine-wide registration records the selected root and former installation
roots. The service path follows that registration rather than a fixed Program
Files directory. A service relocation verifies the new copy before switching SCM
and removes old known components after readback confirms the registered path.

Changing the selected folder preserves personal data. On first launch by the
original Windows account, the client locks source profiles, verifies database and
file hashes and activates the copy under the new root. DPAPI secrets stay under
that account. Per-account private ACLs are retained. Unchanged copied source files
are cleaned; changed/blocked or unknown files are preserved. Other Windows users
migrate their own profiles when they launch the new installation.

After successful service setup the installer cleans known files of the former
registered application, without removing personal data or unrelated files.
Update, repair and uninstall preserve data. Settings shows the actual application
and data paths and provides buttons to open them.

Fresh installs work with the bundled engines without downloading them.
Old unmodified bundled engines upgrade on first launch; explicitly selected
approved core versions remain a user choice. Xray installation validates supported
profiles; Hysteria2 continues using sing-box and no longer blocks an Xray update.
Engine downloads run outside the controller lock. Release queries time out after
12 seconds and successful results are cached for five minutes, with stable
releases shown first. Archive checksums, version checks and rollback remain active.

Portable users keep data beside the extracted client; TUN still requires the
protected registered Windows service. Antivirus protection and quarantine are
never changed by these operations.

Uninstall removes the Apps & Features entry while retaining the protected profile-location identity. Reinstall can reuse retained private data; changed installation roots remain available for per-user migration. A protected data-only root left by older uninstallers is also recognized without accepting arbitrary untrusted payload folders.
