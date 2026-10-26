# Verification — NetPulse 1.0.4

Regular GitHub release, Windows x64, validated on October 26, 2026.

| Area | Result |
|---|---|
| Core checks | 117 passed, 2 blocked by the machine's IPv6 loopback restrictions |
| Stable updates | 1.0.4 is newer than 1.0.3-beta and 1.0.4-beta; regular release metadata accepted |
| Service file checks | 8 passed, including preservation of orphaned update directories |
| Service binary | Safe self-test reports version 1.0.4 and no system changes |
| QML interactions | 15 passed, including setup/cleanup |
| Controller integration | 14 groups |
| Packaged UI | 22 frames, 8 pages, RU/EN and dark/light; Activity fits the viewport |
| Installer | Scoped install, repair/update, uninstall; 8 simulated failure cases and subsequent success |

Windows ping to ::1 fails independently of NetPulse; an independent local TCP
IPv6 probe returns access denied (10013). The two affected tests are reported as
blocked, not passed. Their strict source assertions remain in the repository.
The initial run also exposed an HTTP fixture read-mode issue; the fixture now
uses blocking reads and consumes the complete request headers before replying.

The user supplied a Kaspersky Premium context-menu scan reporting no threats
among 3,381 objects. Its exact files and hashes were not shown. The service is
unsigned; the previous behavioral detection has not received a vendor false
positive determination. Actual service maintenance under antivirus protection
and a privileged TUN session were not tested. No protection, quarantine, global
proxy or route settings were modified during these checks.

Machine-readable evidence: VERIFICATION.json. Release notes: RELEASE_NOTES.md.
