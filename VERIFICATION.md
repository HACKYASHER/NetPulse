# Verification — NetPulse 1.0.5

Regular Windows x64 release. Validation date: October 28, 2026.

| Area | Result |
|---|---|
| Core checks | 126 passed; two IPv6 checks blocked by this machine |
| Profile migration | Packaged controller, real synthetic database, encrypted profiles and settings preserved |
| File security | Private profile ACLs inspected; reparse paths and unsafe installer manifests rejected |
| Engine repair | Missing engine stays absent on startup; explicit offline restoration succeeds |
| Official engine updates | Xray 26.10.17 → 26.10.21 and sing-box 1.14.4 installed, authenticated and rolled back in a temporary directory |
| Missing companion | Explicit selected-core installation succeeds after the other core is removed |
| Service file transactions | 8 passed |
| Service self-test | Version 1.0.5; no system changes |
| QML interactions | 15 passed with the application's Basic style |
| Controller integration | 14 groups |
| Packaged UI | 22 frames × RU/EN and dark/light; eight pages; no QML warnings |
| Installer | Isolated install, repair/update, uninstall and eight simulated service failures |
| Data during removal | Personal files under the installation's data directory preserved |

An independent IPv6 TCP loopback probe fails with Windows 10013 and Windows
ping to ::1 fails. The strict IPv6 tests remain in source and are classified as
environment-blocked, not passed. IPv4 loopback checks pass.

The installer tests use a unique temporary namespace and stub the privileged
service/layout hooks. Migration, ACL preparation and file repair are also tested
separately against actual temporary files. Actual elevated installation, SCM
relocation and TUN maintenance under antivirus protection were not exercised on
the user's live service. Existing service files, protection, quarantine and global
network settings were not changed. No new antivirus-vendor verdict or signing
certificate was obtained; the earlier screenshot's scan does not cover this build.

See [installation layout](docs/INSTALLATION.md), [release notes](RELEASE_NOTES.md)
and the machine-readable VERIFICATION.json.
