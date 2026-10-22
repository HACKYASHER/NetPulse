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
the parent QProcess. `backend/commands.json` lists 64 typed commands adapted
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

TUN service management is explicit and uses standard UAC. The service retains
typed requests, SID/PID checks, pipe ACLs, a protected directory and independent
engine hash validation. Separate identities prevent replacing the old service:

- Service: `NetPulseCoreQt`.
- Pipe: `NetPulse.CoreQt.<SID>`.
- Protected files: `Program Files/NetPulse Core Qt`.
- User data: `%LOCALAPPDATA%/NetPulseQt`.

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
the React screens. Advanced options remain available through a JSON editor;
some specialized editors and charts are deferred. The old client update channel
is disconnected so it cannot replace the new edition with a legacy installer.
