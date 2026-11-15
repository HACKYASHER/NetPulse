# NetPulse 1.0.10

[<kbd>Русский</kbd>](docs/ru/releases/v1.0.10.md)

- Diagnostics checks Windows after a connection error or a failed periodic VPN HTTPS probe. The report covers system proxy, routes, the TUN adapter and MTU, the time service, estimated clock drift, and differences between saved network settings and the running session.
- The file audit names missing or unreadable client, TUN service and engine files. Repair buttons restore working engines from the bundled copies or run TUN maintenance. Use the official setup to repair the client or bundled files.
- An animated NetPulse indicator appears while connecting, running diagnostics or downloading. Navigation, buttons, switches, menus and dialogs have transitions. Clear **Interface animations** in Settings to disable the effects.
- The controller stops updating the tray menu once the menu is destroyed during shutdown.

[Release verification](VERIFICATION.md) · [Compare with 1.0.9](https://github.com/HACKYASHER/NetPulse/compare/v1.0.9...v1.0.10)
