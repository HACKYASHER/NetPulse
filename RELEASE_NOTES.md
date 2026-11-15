# NetPulse 1.0.10

[<kbd>Русский</kbd>](docs/ru/releases/v1.0.10.md)

The refreshed 1.0.10 installer bundles official [Xray 26.11.9](https://github.com/XTLS/Xray-core/releases/tag/v26.11.9). The theme selector is translated, and Interface animations now uses the same switch as the other settings. If you already have 1.0.10, run the updated installer and choose **Update**. The updater only offers newer version numbers.

- Diagnostics checks Windows after a connection error or a failed periodic VPN HTTPS probe. The report covers system proxy, routes, the TUN adapter and MTU, the time service, estimated clock drift, and differences between saved network settings and the running session.
- The file audit names missing or unreadable client, TUN service and engine files. Repair buttons restore working engines from the bundled copies or run TUN maintenance. Use the official setup to repair the client or bundled files.
- An animated NetPulse indicator appears while connecting, running diagnostics or downloading. Navigation, buttons, switches, menus and dialogs have transitions. Turn off **Interface animations** in Settings to disable the effects.
- The controller stops updating the tray menu once the menu is destroyed during shutdown.
- Activity keeps log prefixes left aligned and resets horizontal scroll when jumping to the latest entry. If the log file changes while loading history, Activity reloads from the newest entries. Failed date reads can be retried, and responses for a closed view are discarded.

[Release verification](VERIFICATION.md) · [Compare with 1.0.9](https://github.com/HACKYASHER/NetPulse/compare/v1.0.9...v1.0.10)
