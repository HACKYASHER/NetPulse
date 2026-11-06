# NetPulse 1.0.8 verification

[<kbd>Русский</kbd>](docs/ru/VERIFICATION.md)

November 6, 2026. [Machine-readable report](VERIFICATION.json) · [Release notes](RELEASE_NOTES.md)

| Area | Result |
|---|---|
| Transport | XHTTP REALITY, XHTTP TLS packet-up, gRPC REALITY, and Hysteria2 with Salamander each transferred 2 MiB over HTTPS through local test servers. The separate gRPC TLS test uploaded and downloaded 8 MiB in each direction. |
| Imports and subscriptions | Tests cover REALITY, gRPC, and XHTTP options, the secure Hysteria2 TLS flag, persistence of a URL lock after restoring an older backup, and deletion during a subscription refresh. |
| Interface | Nine native test results and 46 QML test results passed. All eight sections opened without QML errors in four combinations of language and theme. |
| Controller and data | Seven controller tests, 16 groups of IPC checks, profile migration, engine repair, and nine startup scenarios passed. |
| Installer and service | An isolated installer test covered installation, repair, removal, and reinstallation with data retention. Ten maintenance tests and the service self-test passed. |

The main run passed 167 Rust core tests and both gRPC/TLS integration tests.
The Playground test for a TCP listener failed on the local Windows test machine,
as it did with 1.0.7. The IPC check for established TCP connections passed.
Two separate IPv6 checks also failed on the local test machine.

Tests used local servers and fixture keys. Installer and service-repair
scenarios ran in an isolated environment with simulated service exit codes.
The checks left the active TUN, system proxy, and user profile untouched.

The bundled sing-box 1.14.4 and Xray 26.10.31 binaries match the verified
official Windows releases. [Engine source manifest](resources/cores/source-manifest.json)
