# NetPulse 1.0.0 beta

A native Windows VPN client edition with a C++ / Qt Quick interface, a separate
Rust controller, and bundled sing-box and Xray engines.

- Rebuilt ten desktop sections in QML: overview, servers, subscriptions, routing,
  network, diagnostics, Playground, activity, settings and engines.
- Retained profile import, subscriptions, DPAPI storage, System Proxy/TUN,
  routing, diagnostics, engine verification and rollback.
- Added private process communication and separate data/service identities.
- Removed the requirement for Tauri, WebView2 and a browser-based renderer.
- Included installer, portable distribution, application and engine sources,
  licenses and SHA-256 checksums.

**Downloads:** `NetPulse-Setup.exe` or `NetPulse-Portable.zip` (Windows x64).
Supply your own server or subscription. TUN requires service installation
with a standard Windows UAC prompt.

**Beta scope:** advanced options are available through a validated JSON editor.
Some former specialized editors and charts have not been recreated. Legacy
client self-updates are disabled. Executables are unsigned. Verification and
physical TUN testing limits are documented in `VERIFICATION.md`.

NetPulse 3.0.4 remains available as an archived fallback pre-release.

**Validation:** 97 network tests passed, all ten pages rendered in three language/theme variants without QML issues, controller integration and service self-tests passed, and the installer completed an install/launch/uninstall round trip. Physical elevated TUN operation remains unverified.
