# NetPulse 1.0.8

[<kbd>Русский</kbd>](docs/ru/releases/v1.0.8.md)

- VLESS imports retain REALITY, gRPC, and XHTTP settings. XHTTP `packet-up` accepts GET, header and padding options, legacy session field names, and whole numbers written as decimals such as `1000.0`.
- Hysteria2 links with `insecure=0` import with certificate verification enabled.
- Subscription URL copying can be locked permanently, independently of server configuration. The lock survives restoring an older backup, and subscriptions can still refresh.
- The list refreshes immediately after deletion. A late provider response can no longer restore a deleted subscription. If a server from that subscription is active, the client disconnects it after confirmation.
- Country flags are matched by English and Russian names, country codes, and emoji. Lithuania, Romania, and the United States now display their flags.

[Release verification](VERIFICATION.md) · [Compare with 1.0.7](https://github.com/HACKYASHER/NetPulse/compare/v1.0.7...v1.0.8)
