# Third-party components

| Component | Version | Purpose | Source |
|---|---|---|---|
| Qt / Qt Quick | 6.8.3 | Dynamically linked desktop UI | https://code.qt.io/cgit/qt/qtbase.git/ and https://code.qt.io/cgit/qt/qtdeclarative.git/, tag `v6.8.3` |
| sing-box | 1.14.4-netpulse.1 | Primary network engine | https://github.com/SagerNet/sing-box/tree/v1.14.4 |
| Xray | 26.10.21 | Additional transports | https://github.com/XTLS/Xray-core/tree/v26.10.21 |
| Rust dependencies | Cargo.lock | Controller, storage and diagnostics | `backend/Cargo.lock`; notices in `licenses/retained-backend` |
| Microsoft C++ Runtime | 14.x | Compiled C++/Qt support | Microsoft Visual C++ Redistributable |

Qt is distributed under its applicable LGPLv3/GPLv3 terms and linked dynamically.
Compatible Qt DLLs can be replaced, and the application can be rebuilt from the
supplied sources. License texts and source links are in `licenses/qt/`; SPDX
manifests from the SDK are in `licenses/qt/sbom/`.

The retained complete engine source bundle is supplied as
`NetPulse-Engine-Sources.zip`. Provenance and component licenses also appear
in `cores/source-manifest.json` and `cores/licenses/`.

`licenses/NETPULSE-V2-NOTICE.md` documents the source release. Some former
Tauri/React/WebView2 notices are preserved in the license archive for provenance;
these are not dependencies of the new executable.

NetPulse source is GPL-3.0-or-later. See `LICENSE.txt`.

Country flags: [flag-icons 7.5.0](https://github.com/lipis/flag-icons/tree/v7.5.0), MIT license. Full license: `licenses/flag-icons.txt`.

The sing-box executable is a separately marked NetPulse build based on the linked upstream v1.14.4 source, with pinned dependency updates. See [build provenance](docs/SING-BOX-NETPULSE-BUILD.md).
