# NetPulse 1.0.9 verification

[<kbd>Русский</kbd>](docs/ru/VERIFICATION.md)

November 10, 2026. [Machine-readable report](VERIFICATION.json) · [Release notes](RELEASE_NOTES.md)

| Area | Result |
|---|---|
| Native and QML regressions | 11 native and 50 QML test results passed. Coverage includes bounded controller recovery, background errors, continuous log loading, stale replies, preserved selection, new-entry highlighting and narrow-window controls. |
| Audit storage | 8 tests passed, including keeping daily logs beyond 8 MiB, five-day cleanup and reading older rotated files. |
| Controller and service | 7 controller tests and 10 service maintenance tests passed. |
| Packaged startup and data | IPC/settings persistence, startup and single-instance checks, data relocation and explicit offline engine repair passed. |
| Controller recovery | The isolated packaged GUI survived termination of its own controller, started a fresh controller and completed initialization again. |
| Packaged interface | All 8 sections rendered in Russian/English and dark/light themes: 22 frames per combination, no QML issues, Activity viewport bounds checked. |
| Installer lifecycle | Install, update/repair, uninstall and reinstall preserved test data. Eight simulated service failure codes and successful retry were checked. |

The installer lifecycle uses a separate namespace and stubs elevation, live service management and layout-helper hooks. File migration is also tested separately. The tests do not modify the production profile, TUN or system proxy.

This is a targeted validation of the journal and controller changes. Real provider connectivity and live TUN installation were not tested. Transport integration tests were not rerun; this release does not claim fixes for the IPv6 and Playground listener failures documented in the [1.0.8 report](https://github.com/HACKYASHER/NetPulse/blob/be9b3747657a439e1a68dcc305f159de26860404/VERIFICATION.md).

The bundled official sing-box 1.14.4 and Xray 26.10.31 binaries are unchanged and match their pinned SHA-256 hashes. [Engine source manifest](resources/cores/source-manifest.json)
