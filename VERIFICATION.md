# Verification — NetPulse 1.0.0 beta

Validated on Windows x64 on October 22, 2026, with Qt 6.8.3 and MSVC 2022.
The retained Rust network core uses the bundled sing-box and Xray executables.

## Executed checks

| Area | Result |
|---|---|
| Core unit tests | 86 passed, 0 failed |
| Additional tests with actual bundled engines | 18 passed, 0 failed |
| Settings matrix | 82 variants saved, reopened and accepted by actual engine configuration validators |
| Invalid settings and combinations | 16 invalid values and 2 incompatible combinations rejected |
| Controller integration | 12 grouped checks, including 59 settings form variants |
| Packaged Qt UI | 8 pages and 17 page/tab/dialog frames in RU/dark, EN/light and EN/dark; 0 QML issues |
| Portable dependencies | Windows-only PATH; no external Qt import/plugin paths |
| Playground | Owned TCP connection found by PID and EXE group; network-focused process data |
| Service self-test | SID, security descriptor and protected-directory lookup passed |
| Scoped installer lifecycle | Install, update/repair, damaged-controller replacement, installed UI and uninstall passed; test data preserved |

The settings matrix exercises DNS modes/routes, MTU boundaries and presets,
connection switches, engine selection, SNI, TLS fingerprints, HTTP Host,
fragmentation, multiplexing, application and domain routing, ping preferences,
themes/languages and launch preferences. Each valid variant is reopened from
SQLite and its generated configuration is checked by the real bundled cores.
These are configuration checks, not remote VPN sessions.

Controller tests verify empty new rule lists, encrypted profile storage, DNS
persistence, EXE basename migration/deduplication, the dedicated routing JSON,
pinning, atomic failed saves, request correlation, diagnostic history and journal
selection/path validation. Real owned Windows sockets are inspected by PID and
EXE group. Restart persistence is checked. Autostart intent is saved in isolation
without editing the user's Windows Run entry.

Engine-backed tests exercise authenticated local HTTPS, transport selection,
bounded speed payloads, routing, switching/rollback, closed-flow accounting and
Windows Job cleanup. They use owned loopback fixtures, with no system TUN or
global proxy changes.

UI rendering covers all seven Network tabs plus the connection-mode, import and
measurement-history dialogs. It also exposed and helped fix a light-theme tray
menu crash. Unsaved forms survive snapshot polling; ping mode is no longer reset
when Diagnostics is reopened. Manual interactive coverage remains partial.

Installer tests use a separate installation folder, registry and shortcut names.
Both elevated service hooks are disabled in the test copy. They verify update
and repair semantics by replacing a deliberately damaged controller, then check
installed UI launch, uninstall registration cleanup and retained test data. The
production installer attempts service installation through normal Windows UAC.
Elevated service installation itself was not exercised.

## MTU comparison

The algorithm requires repeated candidate measurements and a stable original-MTU
control. It rejects failed samples and large variation, and compares speed and
latency against the better original-MTU samples to avoid attributing a general
network improvement to MTU. Tests cover noise, failed probes, baseline drift,
repeatability and crash recovery preserving unrelated preferences.

The optional live comparison changes only MTU during trials, reconnects the active
owned TUN session, limits download payload to 36 MiB plus overhead and keeps a
recovery journal. The original MTU is restored while the session/settings remain
owned; unrelated user changes take priority. A candidate is applied manually.
HTTPS measurements do not establish game-UDP latency or a universal optimum.

DF path analysis keeps measured IPv4 path MTU separate from a conservative VPN
estimate. Timeouts do not establish a fragmentation boundary. Reference material:
[Windows ping](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/ping),
[RFC 8899](https://www.rfc-editor.org/rfc/rfc8899.html), and
[the supplied MTU article](https://serveradmin.ru/kak-pravilno-vybrat-mtu-na-setevom-interfejse/).

## Remaining limits

- Elevated TUN through a real remote VPN and a physical adapter, including the
  live MTU benchmark, has not been verified in this environment.
- Physical adapter/DNS recovery after an OS-level failure needs a dedicated
  Windows test. Existing ownership and recovery safeguards are retained.
- No production subscription, provider endpoint or remote VPN account was tested.
- Application and installer are unsigned.

The machine-readable report is included as `VERIFICATION.json`. Release asset
hashes are listed in `checksums.sha256`. Test data, build caches and private
journals are excluded from source and portable archives.
