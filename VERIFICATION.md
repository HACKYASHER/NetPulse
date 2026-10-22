# Verification вЂ” NetPulse 1.0.0 beta

Validation was performed on Windows x64 on October 22, 2026. The native client
was built with Qt 6.8.3 and MSVC 2022. The network layer is retained from
NetPulse 3.0.4, with separate service and data identities.

## Executed checks

| Area | Result |
|---|---|
| Rust network unit tests | 79 passed, 0 failed |
| Additional tests with the actual bundled engines | 18 passed, 0 failed |
| Packaged Qt UI | All 10 pages rendered in RU/dark, EN/light and EN/dark; 0 QML warnings/errors |
| Private controller protocol | 8 grouped integration checks passed |
| Packaged service self-test | SID validation, security descriptor and protected directory checks passed |
| Portable dependencies | UI launched with a Windows-only PATH and no external Qt import/plugin paths |
| Per-user installer round-trip | Installation, installed UI launch, uninstall and registration cleanup passed |
| Version metadata | Client and controller report 1.0.0 beta / 1.0.0-beta |

Engine-backed checks exercised configuration validation, supported transport
selection, real local HTTPS probes, speed payload limits, connection switching,
rollback, direct exceptions, closed-flow accounting and kill-on-close Jobs.
They used owned loopback fixtures and did not enable system TUN or change
Windows' global proxy configuration.

Controller integration covered offline engine preparation, malformed/unknown
request rejection, encrypted profile persistence, selection, settings validation,
10 concurrent response IDs, a real diagnostic job, safe log paths and restart
persistence. No real user profile or subscription was used.

The service self-test checks local security primitives without installing or
starting a Windows service. The smoke run saves screenshots in an isolated
directory and never connects. Generated test data is excluded from release
archives and the Git repository.

Installer round-trip results and release hashes are recorded in the accompanying
`VERIFICATION.json` and `checksums.sha256` after packaging. The installer uses
a separate per-user installation key and does not install TUN automatically.

## Remaining limits

- A full elevated TUN connection through a remote server and a physical network
  adapter has not been tested in this environment.
- Recovery of physical adapter/DNS state after an OS-level failure needs a
  dedicated Windows test. Existing engine/watchdog ownership logic is retained.
- No production subscription, real remote VPN account or provider endpoint was tested.
- The application and installer are unsigned.
- Advanced transport/routing parameters are exposed through the validated JSON
  editor; not all specialized editors or graphs from the previous UI are present.

These checks demonstrate the stated local behavior, not a guarantee of remote
server availability, anonymity, censorship resistance or performance.
