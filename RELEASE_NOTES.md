# NetPulse 1.0.2 beta

NetPulse improves journal selection, server selection, switching and subscription details.

- Drag a journal selection beyond either horizontal edge to scroll continuously. New entries do not interrupt a selection; Ctrl+C and the context menu copy ordinary text. Manual reading pauses following the journal.
- Overview has a server button with the selected flag/name and a searchable picker. Repeated clicks cannot queue competing switches.
- Switching validates the final configuration and hashes once before probing the candidate, while keeping the current session alive. Failed candidates and occupied local ports preserve the working connection. HTTPS and certificate checks remain enabled.
- Sniffing exclusions now invert only the process condition and always remain restricted to TUN. Saving Sniffing without TUN no longer restarts the runtime.
- Subscription counts show the complete subscription independently of search. The traffic bar retains expiry date and time, including when collapsed. Missing quota or expiry information is shown explicitly.
- ComboBox arrows are centred geometrically. Known network errors gain Russian/English explanations without hiding the original error or changing its severity.

The repeated switch/rollback test on owned local SOCKS/TLS endpoints took 88.06 seconds before and 44.13 seconds after the change in a debug build. This is a whole-scenario comparison, not a promise for every VPN server or physical network.

Download **NetPulse-Setup.exe** or **NetPulse-Portable.zip**. Installation, update/repair and removal preserve user data. Older releases remain available.

Live TUN switching on the user's VPN is not part of the automated checks. The reviewed logs also contain external DNS failures, TLS DNS interruptions, timeouts and IPv6 destinations without a route; those failures remain visible. Self-steal is still awaiting the intended feature description.
