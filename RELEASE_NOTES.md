# NetPulse 1.0.3 beta

NetPulse fixes missing core connection logs and automatic IPv6 behavior in split tunneling.

- Both sing-box and Xray now receive the selected logging level. INFO includes ordinary connection records; the journal has a visible level selector. Active level changes use the existing verified restart and rollback path.
- Category switching resets horizontal scroll and selection, so shorter core lines cannot remain outside the visible area. Date/model loading settles before reading, and loading or read errors are shown explicitly.
- The journal reads the newest 500 lines across managed rotation files within one 256 KiB budget. An empty current file no longer hides existing rotated core records.
- TUN IPv6 has Auto, IPv4 only and IPv4/IPv6 modes. In Auto, a whitelist/direct split tunnel without a physical IPv6 default uses IPv4 instead of advertising unusable IPv6 to other applications. Global VPN keeps server-side IPv6. Explicit dual stack remains available.
- Service compatibility is checked before tearing down a working connection, so an outdated TUN service does not cause a destructive failed switch.

The supplied log contained 91 errors: 66 direct-connection timeouts, 23 unreachable IPv6 destinations and 2 interrupted DNS HTTP/2 requests. Timeouts to unavailable remote/local peers are not hidden or converted into successes. TLS verification and application routing choices remain enabled.

Automated checks use owned local endpoints and synthetic data. No live TUN, DNS, MTU or Windows proxy settings were changed during verification. Install the current TUN service when upgrading (normal Windows UAC); older service binaries cannot accept newer settings.
