# Architecture

```text
NetPulse.exe — C++20 / Qt Quick / QML, normal user privileges
    │
    │ private inherited stdin/stdout; JSON with correlated request IDs
    ▼
netpulsed.exe — Rust NetworkCore, SQLite and DPAPI
    ├── sing-box / Xray: separately managed engine processes
    ├── System Proxy recovery: independent watchdog
    └── authenticated Windows named pipe
            ▼
       NetPulseService.exe — NetPulseCoreQt, elevated TUN operations
```

## Desktop boundary

The C++ client starts only the bundled controller using an absolute path. It
does not start a shell or local HTTP server. Inherited stdin/stdout belong to
the parent QProcess. `backend/commands.json` lists 68 typed commands adapted
from NetPulse 3.0.4. Frames are limited to 12 MiB and concurrent execution to
eight requests. Method names use an explicit allowlist, not shell interpretation.

The regular UI receives profile summaries without credentials. The explicit
JSON editor requests full profiles. Keys do not appear on the command line.
Imports clear the dialog on success; private stderr is not copied to the UI log.
Tauri and React are not runtime dependencies. A small independent `host` module
replaces their command transport while preserving core validation and rollback.

## Processes and ownership

A QLockFile prevents duplicate access to the client's data directory. Normal
exit cancels diagnostics, restores the owned System Proxy lease and stops
managed engines. Input EOF also requests cleanup. The retained independent
watchdog uses an encrypted journal; engine processes belong to a Windows Job.
Full physical adapter recovery still needs elevated Windows validation.

The installer offers update, repair and uninstall, preserving user data. It
attempts TUN service installation by default through standard UAC; portable
users can install or repair the service from Network. Service management uses
the same verified helper. The service retains
typed requests, SID/PID checks, pipe ACLs, a protected directory and independent
engine hash validation. Separate identities prevent replacing the old service:

- Service: `NetPulseCoreQt`.
- Pipe: `NetPulse.CoreQt.<SID>`.
- Installation root: user-selected local directory (administrator-owned payload).
- Protected service files: `<installation>/service`.
- User data: `<installation>/data/<Windows SID>` (private writable profile).
- Legacy paths are accepted only for validated migration; see [installation layout](docs/INSTALLATION.md).

## Engines and data

Qt DLLs are deployed next to the client. `cores/` supplies the verified engines;
the controller prepares the user runtime separately. The original sing-box/Xray
division, including authenticated local transport bridges, is retained.
No Happ binaries, source code or proprietary assets are included.

SQLite and DPAPI protect stored profile data. DPAPI entropy is preserved for
explicit import of a 3.0.4 backup under the same Windows account. The old
database is not opened automatically. Preferences pass through the original
validation and application logic.

## Migration scope

The application layer is C++/Qt; the retained network layer is Rust. QML replaces
the React screens. Network forms cover DNS, MTU, TLS/SNI, fragmentation and Mux;
the validated JSON editor remains available for less common options. Application
routes use EXE basenames, with discovery paths retained separately for icons.
`routing-apps.json` is a portable rules copy alongside the authoritative database.

Client updates require the native Qt channel marker and an official GitHub
installer digest, excluding the archived legacy release. Managed ZIP caches,
interrupted downloads and obsolete client installers are cleaned up; the previous
engine bundle remains available for rollback.

MTU comparison is user initiated and restricted to the active owned TUN session.
It benchmarks bounded HTTPS payloads, repeats the leading candidate, checks the
original baseline again, then restores MTU. A recovery journal handles interrupted
tests without replacing unrelated preferences. Applying a recommendation is
explicit; the measurements do not establish a universal or game-UDP optimum.
