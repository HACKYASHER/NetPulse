# NetPulse 1.0.1 beta

NetPulse 1.0.1 beta fixes connection controls, engine switching, persistent latency results, and scrolling across the native Windows client.

- TUN and System Proxy can be selected independently or together, with an explicit confirmation before connecting. Overview adds a labelled connection button, manual ping refresh, transfer totals and active connections.
- Explicit engine choices are applied and retained. Unsupported combinations produce an error instead of silently selecting another core. Failed switches preserve or restore the previous verified connection.
- The last latency result is stored separately for each server and method, so measurement-history cleanup cannot remove it. Subscription headers include a latency test for their servers; protected copy/configuration actions are hidden.
- Sniffing controls are grouped in Network and apply to TUN metadata. System Proxy continues to use the target provided by the application without waiting for its first data.
- MTU comparison tests each value in a chosen range (1440–1472 by default), with up to five probes per value, a repeated candidate and baseline controls. Results describe latency, loss and jitter, restore the original MTU, and require a manual Apply action. These probes do not measure download throughput. The old physical-path automatic MTU selector has been removed from the UI.
- Gaming enables TUN and Cloudflare secure DNS through VPN while preserving MTU, System Proxy and routing rules. TCP is the default latency method on new installations. Speed Test repeats transfers for the selected duration or until its explicit traffic limit is reached.
- The activity console appends updates without replacing the whole document, preserves selection, and has separate working horizontal and vertical scrollbars. Application-list scrolling no longer scrolls the containing Routing page.
- Sidebar alignment, light/dark hover contrast and Russian log labels are corrected. The installer supports installation, update, repair and removal under the NetPulse name.

Download **NetPulse-Setup.exe** for installation or **NetPulse-Portable.zip** for a portable copy. Source and dependency archives, SHA-256 checksums and verification results are included.

This is a beta release. Automated checks use owned local endpoints and synthetic profiles. Live TUN operation and the MTU scan on a real VPN were not exercised on the user's active connection. Self-steal is awaiting clarification of the intended feature and is not represented by an unrelated setting.
