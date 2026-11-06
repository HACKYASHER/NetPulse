# Third-party components

[<kbd>Русский</kbd>](docs/ru/THIRD-PARTY.md)

| Component | Version | Purpose | Source |
|---|---|---|---|
| Qt / Qt Quick | 6.8.3 | Dynamically linked desktop UI | https://code.qt.io/cgit/qt/qtbase.git/ and https://code.qt.io/cgit/qt/qtdeclarative.git/, tag `v6.8.3` |
| sing-box | 1.14.4 | Primary network engine | https://github.com/SagerNet/sing-box/tree/v1.14.4 |
| Xray | 26.10.31 | Additional transports | https://github.com/XTLS/Xray-core/tree/v26.10.31 |
| Rust dependencies | Cargo.lock | Controller, storage and diagnostics | `backend/Cargo.lock`; notices in `licenses/retained-backend` |
| Microsoft C++ Runtime | 14.x | Compiled C++/Qt support | Microsoft Visual C++ Redistributable |

Qt is distributed under its applicable LGPLv3/GPLv3 terms and linked dynamically.
Compatible Qt DLLs can be replaced, and the application can be rebuilt from the
supplied sources. License texts and source links are in `licenses/qt/`; SPDX
manifests from the SDK are in `licenses/qt/sbom/`.

The matching engine and dependency sources ship as `NetPulse-Engine-Sources.zip`.
The repository includes the [engine source manifest](resources/cores/source-manifest.json)
and [component licenses](resources/cores/licenses).

`licenses/NETPULSE-V2-NOTICE.md` documents the source release. Some former
Tauri/React/WebView2 notices are preserved in the license archive for provenance;
these are not dependencies of the new executable.

NetPulse source is GPL-3.0-or-later. See `LICENSE.txt`.

Country flags: [flag-icons 7.5.0](https://github.com/lipis/flag-icons/tree/v7.5.0), MIT license. Full license: `licenses/flag-icons.txt`.

English and Russian territory names come from [Unicode CLDR 48.0.0](https://github.com/unicode-org/cldr-json/tree/48.0.0). The Unicode License v3 is in `licenses/unicode-cldr.txt`.

The bundled sing-box executable is the unchanged official v1.14.4 Windows release. Xray is the unchanged official v26.10.31 Windows release.
