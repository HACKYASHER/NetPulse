# Verification — NetPulse 1.0.2 beta

Validated on Windows x64 on October 25, 2026 with Qt 6.8.3 and the bundled official sing-box and Xray cores. Exact results are in VERIFICATION.json.

| Area | Result |
|---|---|
| Core tests, including real engines | 112 passed in one combined run; zero failures |
| Settings persistence and actual configuration validation | 87 variants passed; 16 invalid values and 2 incompatible combinations rejected |
| Packaged controller integration | 14 grouped checks, including settings and ping persistence |
| UI interaction tests | 10 scenarios plus setup/cleanup passed |
| Packaged UI | 8 pages and 19 frames in each RU/dark, RU/light, EN/light, EN/dark run; zero QML issues |
| Scoped installer lifecycle | Install, update/repair, installed UI, uninstall; user data preserved |

Journal checks drag selections past both horizontal edges, append during selection, test Ctrl+C and normal-text copying, and move both scrollbars. Other interactions cover engine selection, simultaneous modes, nested application scrolling, light-theme menus, searchable server selection, quota/expiry rendering, and network error explanations.

The core tests carry real HTTPS requests after repeated sing-box/Xray changes. Failed candidates and occupied ports keep the current process alive. Saving Sniffing without TUN preserves the process. Generated TUN rules validate the corrected AND condition for excluded processes with the actual bundled core.

The same repeated switch/rollback scenario took 88.06 seconds before and 44.13 seconds after in a debug build on owned loopback endpoints. This describes the whole test scenario and is not a latency guarantee for an individual VPN server.

Installer tests use scoped registry/shortcut/folder names and disabled service hooks. Their faster compression changes neither the application payload nor the maintenance behavior. Production archives include checksums.

## Limits

No live privileged TUN connection, real VPN MTU scan, public-WAN speed measurement or update installation over the user's running client was performed. Network tests use synthetic profiles and owned local endpoints. The reviewed logs contain external DNS/TLS interruptions, absent IPv6 routes and remote timeouts; errors remain visible with explanations. Self-steal awaits clarification.
