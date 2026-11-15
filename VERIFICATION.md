# NetPulse 1.0.10 verification

[<kbd>Русский</kbd>](docs/ru/VERIFICATION.md)

November 15, 2026. [Machine-readable report](VERIFICATION.json) · [Release notes](RELEASE_NOTES.md)

| Area | Checks |
|---|---|
| Network diagnostics | Clock offset fixtures cover a four-second drift, unreachable and conflicting peers, a clock adjustment during sampling, and invalid NTP replies. Configuration tests distinguish differences from missing runtime evidence. |
| File repair | A packaged isolated profile reported a missing sing-box executable by name. The bundled repair restored the verified file, and repeating the audit cleared the issue. Empty files are covered by a unit test. |
| Interface | 11 native and 50 QML results passed. All eight pages and modal flows were checked in both languages and themes, with animations disabled for English/light. Shared controls were also checked across an on/off switch. |
| Installation | Installation, update/repair, uninstall, and reinstall preserved isolated profile data. The harness exercised eight simulated service errors. |
| Controller and data | Audit storage, controller and service unit checks passed, along with packaged IPC, startup, single-instance handling and controller recovery. |

These tests exercise client behavior and use a separate Windows namespace for installer and repair scenarios. Live TUN installation and VPN-provider connectivity remain outside the test scope. The configuration comparison records whether saved values match the active session. Proving DNS, TLS or per-app routing behavior requires targeted traffic tests.

[How the diagnostics work](docs/HEALTH-CHECK.md)
