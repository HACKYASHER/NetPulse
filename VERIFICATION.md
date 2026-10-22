# Verification — NetPulse 1.0.1 beta

Validated on Windows x64 on October 23, 2026, with Qt 6.8.3 and the bundled official sing-box and Xray cores.

| Area | Result |
|---|---|
| Core tests, including actual engines | 112 distinct cases passed (94 regular, 18 engine-backed) |
| Settings persistence and real configuration validation | 87 variants passed |
| Invalid values / incompatible combinations | 16 / 2 rejected |
| Packaged controller integration | 14 grouped checks, including 59 form variants |
| UI interaction tests | 6 scenarios plus setup/cleanup passed |
| Packaged UI rendering | 8 pages and 18 page/tab/dialog frames, RU/dark, RU/light, EN/light, EN/dark; zero QML issues |
| Scoped installer lifecycle | Install, repair of a damaged controller, installed UI and uninstall passed; data preserved |

Core tests use owned loopback endpoints. They exercise repeated sing-box ↔ Xray switches with real HTTPS requests, failed-switch rollback, profile switching, route validation, close-flow accounting, bounded sustained speed tests, latency persistence and history eviction. One Windows test fixture inherited nonblocking sockets; the fixture was corrected and the affected test passed on rerun. All other 111 cases passed in the combined run. No production exception was introduced to make the test pass.

The settings matrix saves each variant, reopens SQLite and runs actual engine configuration validators. It covers DNS and routes, MTU values and presets, independent TUN/proxy preferences, all engine choices, Sniffing protocols/timeouts, SNI, TLS fingerprints, HTTP Host, fragmentation, multiplexing, application/domain rules, languages, themes and launch preferences. It does not create a real TUN adapter.

UI interaction checks click the engine picker, select both connection modes, append console entries while preserving scroll and selection, move both console scrollbars, test nested Routing scrolling, and check light-theme menus and hidden protected actions. Rendering uses synthetic loopback profiles in separate data folders with only packaged Qt dependencies.

Installer lifecycle tests use separate folders, shortcut/registry names and disabled service hooks. The final check can use faster compression; application payloads and maintenance behavior remain the same. Real service installation and the user's installed client are not changed by these tests.

## Limits

A live privileged TUN connection, MTU range scan over the user's VPN server, public-WAN speed tests, and production update installation were not exercised. The MTU algorithm was checked with synthetic samples and ownership/recovery tests. Its probes compare latency, loss and jitter; they do not measure download throughput. Self-steal awaits clarification of the intended feature.

See [machine-readable verification](VERIFICATION.json). Download hashes are supplied separately in `checksums.sha256`.
