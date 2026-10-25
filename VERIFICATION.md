# Verification — NetPulse 1.0.3 beta

Validated on Windows x64 on October 25, 2026 with Qt 6.8.3 and the bundled official cores. Machine-readable results are in VERIFICATION.json.

| Area | Result |
|---|---|
| Core tests | 116 passed (97 regular, 19 engine-backed), zero failures |
| Real INFO output | A separate owned-loopback switch test verifies an actual INFO connection record |
| Settings | 92 persisted and real-core-validated variants; 17 invalid and 2 incompatible values rejected |
| Controller integration | 14 groups, including 62 settings variants |
| QML interactions | 12 scenarios plus setup/cleanup passed |
| Packaged UI | 20 frames across 8 pages, RU/EN and dark/light; non-empty real core category and zero QML issues in every run |
| Installer | Scoped install, repair/update, installed core journal, uninstall and data preservation passed |

Tests cover all log categories with following paused, source scroll reset, edge selection/copying, rotation reading, logging levels for both cores, legacy preferences and automatic/forced IPv6 policy. The actual pinned engine validates IPv4 TUN with and without FakeIP. The current runtime remains alive when a candidate or local port fails. Service version is checked before teardown.

No live TUN was established and the user's Windows routes, proxy, DNS and MTU were not modified. Native routes and saved settings were inspected read-only for diagnosis. IPv6 capability combinations are injected into policy tests; privileged service operation is outside this automated run. External direct-connection failures remain visible.
