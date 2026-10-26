# NetPulse 1.0.4

The user-provided Kaspersky context-menu scan reported no threats among 3,381 objects. Its exact file scope and hashes were not shown. Service-maintenance compatibility remains unverified and the service is unsigned.

## Incident and limits of diagnosis

The supplied Kaspersky report identifies PDM:Trojan.Win32.Generic on the
per-user NetPulseService.exe. The subsequent entries for the protected service,
sing-box, Xray, libcronet and their staging/backup copies say that an activity
rollback was performed. Read-only inspection confirmed that all four live
service components and the per-user maintenance helper were absent and SCM
reported the service as stopped. The released 1.0.3 helper is unsigned.

This establishes the sequence of removal, not that the detection is false or
which exact operation triggered it. No protection settings, quarantine items,
real service registration or user VPN settings were changed during this fix.
No automatic reinstallation of the removed service is attempted. Code signing
and a vendor review remain necessary before claiming antivirus compatibility.

## Corrections

- TUN readiness and periodic health checks derive their required address
  families from the final runtime configuration. IPv4-only no longer requires
  an IPv6 route. Both families must belong to the same live captured interface
  when dual stack is configured.
- Maintenance checks its complete source before disconnecting. Service startup
  checks the protected executable path and readable, nonempty components before
  asking SCM to start. Missing files and Win32 access/security errors have
  specific messages rather than a generic Some(5).
- Updates stage only changed components, validate the complete transaction
  before commit, move originals to a protected backup, and verify final files.
  Successful updates remove their staging files. Validation failures restore
  originals only if all affected artifacts still match this transaction.
- External deletion or modification stops recovery and retains original backups
  for investigation. No loop recreates a quarantined component.
- Ordinary upgrade and installer stop wait for graceful SCM shutdown. They do
  not terminate the service process forcibly.
- Activity uses the actual remaining page viewport, including the space consumed
  by the error banner. The console fills the rest and keeps its own scrollbars.

## Vendor report preparation

The incident concerns the published 1.0.3 binary. Its file hash and unsigned
status can be recorded alongside the screenshots and public source commit
55410e39ca37c78d8b7f75a21e173183919bafeb. Sending a file or support message is a
separate external action; this change does not submit anything automatically.

Kaspersky describes activity rollback here:
https://support.kaspersky.com/help/Kaspersky/Win21.7/en-US/68174.htm
Official file-analysis portal: https://opentip.kaspersky.com/
