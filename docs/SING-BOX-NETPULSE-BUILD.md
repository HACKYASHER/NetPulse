> Историческая документация прежней отдельной сборки. В текущем комплекте используется неизменённый официальный sing-box 1.14.4.

# sing-box 1.14.4-netpulse.1

This is a separately marked NetPulse build of upstream sing-box v1.14.4,
not an official SagerNet release. Original branding and licenses are retained.
The application still bundles the official Xray 26.10.21 executable.

The exact dependency replacements are in
`resources/cores/netpulse-build/go.mod` and `go.sum`. They update Chi to 5.3.0,
JOSE to 4.1.4, DHCP to c76316d4aa82, x/crypto to 0.58.0, x/net to 0.62.0,
x/mod to 0.41.0, x/text to 0.43.0 and gRPC to 1.83.2, with their required
transitive versions. Only dependencies and build selection change; upstream
sing-box application source files are unchanged.

Enabled features are gVisor, QUIC, uTLS and the authenticated Clash statistics
API. These cover NetPulse's supported protocols, system/mixed TUN stack, DNS,
Sniffing and routing. Unused optional protocols/services are not enabled.
The companion official libcronet DLL is retained for users explicitly selecting
an official full engine through the updater; this build does not load it.

Build requirements and executable SHA-256 are recorded in
`resources/cores/netpulse-build/manifest.json`. Use **Go 1.27.3**:

```powershell
python scripts/build-sing-box.py --go C:\Path\To\Go\bin\go.exe
python scripts/fetch-engine-sources.py
```

The script verifies the original source archive and dependency override hashes,
uses `-mod=readonly`, checks the exact toolchain and verifies the final executable
against the published hash. Two independent source extractions produced identical
executables during release verification. The complete modified source archive,
original dependency sources with verified h1 sums, and original licenses are
included in `NetPulse-Engine-Sources.zip`.

The final binary scan no longer reports the original HTTP/2/gRPC, Chi, JOSE,
DHCP, SSH, sumdb and PRECIS findings. The scanner still emits the module-wide
[OpenPGP advisory](https://pkg.go.dev/vuln/GO-2026-5932) for x/crypto.
`go list -deps` with the exact build tags confirms that no
`golang.org/x/crypto/openpgp` package is compiled. This finding is documented,
not silently removed from the raw scanner output.

Selecting official sing-box 1.14.4 manually does not include these patches.
The client labels that distinction, and a later startup upgrades known older
bundles to the patched default. Use “Restore bundled engines” to return to the
NetPulse build immediately. Missing/quarantined executables are not silently
recreated on startup.
