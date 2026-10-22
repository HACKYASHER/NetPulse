# NetPulse 1.0.0 beta

Native Windows VPN client with a Qt Quick desktop, Rust controller, sing-box
and Xray. Supply your own server or subscription.

- Unified servers and subscriptions, pinned profiles, context-menu actions and
  subscription refresh on every startup and by interval.
- Fixed country flags, connection-mode selection, copyable direct/VPN IPs,
  engine selection and automatic active-connection latency.
- EXE-name application routing, icons, dedicated routing JSON, import/export
  and custom domain exceptions.
- Network tabs with DNS, MTU, SNI/TLS, fragmentation, multiplexing, TUN service
  and verified engine update/rollback controls, with inline explanations.
- MTU path analysis and optional bounded comparison of candidate values using
  real throughput and latency measurements, repetition and baseline control.
- Fixed measurement history, added speed-test endpoints, grouped process network
  activity and application connection events.
- Copyable coloured journals, working translated category selection and follow
  controls; improved light/dark controls and a themed tray with connection actions.
- Native client update checks on startup/every six hours; automatic cleanup of
  temporary downloads while keeping the previous engine bundle for rollback.
- Installer update, repair and uninstall; default TUN service installation uses
  standard Windows elevation. Settings and subscriptions are preserved.

Downloads: **NetPulse-Setup.exe** or the complete **NetPulse-Portable.zip**
(Windows x64). Sources, notices and SHA-256 checksums are included.

This is a beta. Executables are unsigned. Physical elevated TUN operation and
the MTU comparison on a real remote VPN remain unverified in this environment;
see the included verification report. MTU comparison reconnects VPN repeatedly
and consumes up to 36 MiB of payload plus protocol overhead. It does not claim
a universal best MTU or improved game UDP latency.

**NetPulse 3.0.4** remains an archived fallback pre-release.
