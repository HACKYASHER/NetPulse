# Computer and network consistency checks

Diagnostics includes a read-only computer check. Opening Diagnostics runs a local check if no report exists. A connection error triggers a full check with a 90-second cooldown. While connected, a periodic HTTPS probe can trigger the same report even when the engine process is still running. The report shows when the evidence was collected.

The report distinguishes observed OS state, matching configuration, mismatches, and unavailable evidence. It compares 20 network preference fields with the active session, reads Windows proxy/PAC settings, adapters and routes, the IPv4 TUN MTU, TUN service installation status and Windows Time service state. Configuration equality is not a functional test of each application's routing, DNS, Sniffing or TLS settings.

Full checks send two bounded UDP NTP requests to Cloudflare's documented IPv4 servers. When an active VPN probe endpoint is available, the check also sends an HTTPS request to Google's generate_204 endpoint through the VPN. NTP replies must match the request nonce, server mode/version and synchronization state. Slow replies, disagreements and wall-clock changes during sampling return unverified. UDP NTP provides an advisory estimate. Clock changes go through Windows Date & Time.

Four seconds of clock drift is reported for investigation, not declared the cause of every TLS failure. REALITY's server-side `maxTimeDiff` and certificate validity can impose different requirements. The client's certificate validation is never disabled.

The file audit looks for essential client and Qt files, bundled and working engines, and the protected TUN service components in use. It reports missing, empty, unreadable or redirected files. The engine and TUN buttons use the existing verified repair workflows and check again after success. If the client or its bundled repair source is damaged, use the official setup and select Repair.

Other actions open Windows time or proxy settings and NetPulse service/engine settings. Reconnection requires confirmation and uses the existing serialized connection operation, preserving the selected server and configured connection modes.

References: [Cloudflare NTP addresses](https://developers.cloudflare.com/time-services/ntp/usage/), [Windows Time tools](https://learn.microsoft.com/en-us/windows-server/networking/windows-time-service/Windows-Time-Service-Tools-and-Settings), [REALITY configuration](https://xtls.github.io/en/config/transports/reality.html).
