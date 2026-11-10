# NetPulse 1.0.9

[<kbd>Русский</kbd>](docs/ru/releases/v1.0.9.md)

- Activity is now one continuous journal: scroll upward to load earlier entries, without page buttons. Daily selection and five-day retention remain. Daily log files are not split at 8 MiB.
- New entries are highlighted, and **Latest ↓** returns to the live tail. Loading earlier entries preserves your selection and scroll position. Bursts of new entries are fetched across multiple batches without gaps.
- Activity controls fit narrower windows, including both themes and interface languages.
- Routine background polling failures no longer open the global error banner. Settings blocked by temporary controller or engine availability are retained and retried.
- A crashed controller restarts automatically, with at most three attempts and an increasing delay. Partial responses are discarded, and quitting cancels recovery.
- A command panic returns an error response instead of leaving the UI waiting indefinitely. Shutdown still attempts cleanup after a disconnect error or a poisoned controller lock.
- Temporary or missing application paths display a local fallback icon instead of producing QML image errors.

The bundled official engines remain **sing-box 1.14.4** and **Xray 26.10.31**. This release changes the client and journal, not the transport engines.

Validation covers native/QML regressions, audit storage, controller/service unit tests, isolated packaged startup and recovery, interface rendering, and the installer lifecycle. Real provider connectivity and live TUN installation are not included in this validation; see the [verification report](VERIFICATION.md) for scope.

[Compare with 1.0.8](https://github.com/HACKYASHER/NetPulse/compare/v1.0.8...v1.0.9)
