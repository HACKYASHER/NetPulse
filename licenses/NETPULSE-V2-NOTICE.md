# License and third-party notices

NetPulse 2.0 source is GPL-3.0-or-later. Corresponding source is supplied at https://github.com/HACKYASHER/NetPulse in apps/desktop-v2 and third-party/sources, with GPL text in LICENSE.txt. This does not relicense the separate legacy project or third-party components.

The installer includes unmodified official sing-box 1.14.3, libcronet.dll, and Xray-core 26.10.17. Original engine/module notices are retained in cores/licenses. The exact binaries are verified using pinned SHA-256 hashes. sing-box retains its upstream GPL terms and naming restrictions; Xray retains MPL-2.0. NetPulse is an independent client, not an official engine client or renamed engine fork.

The repository third-party/sources/engine directory contains original tagged sing-box and Xray source archives and 168 original Go dependency archives, with SHA-256 and Go h1 checksums from the bundled binaries in manifest.json. Original cronet-go Windows module sources and license notices are included in that inventory. Keep equivalent access to these corresponding sources available alongside binary downloads. Upstream sources: https://github.com/SagerNet/sing-box/tree/v1.14.3 and https://github.com/XTLS/Xray-core/tree/v26.10.17 .

DEPENDENCIES.json lists the Windows Cargo graph, including build tools, and frontend runtime packages. The repository third-party/sources directory contains original registry .crate archives and exact npm runtime packages as source ZIPs. Cargo.lock records Rust checksums. Original notices are supplied in licenses/; missing package-level files retain provenance in apps/desktop-v2/licenses/provenance.json. Third-party authorship and license terms are unchanged.

Microsoft WebView2 is installed separately. No FlClash, v2rayN, Hiddify, mihomo or Remnawave application code or logos were copied. The pulse icon and SVG country flags are application assets. The local development toolchain is not included in the installer.
