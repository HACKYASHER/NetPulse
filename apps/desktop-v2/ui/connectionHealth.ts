import type { NativeError, Snapshot } from "./types";

export function serviceMonitorError(
  error: NativeError | null | undefined,
): boolean {
  return (
    !!error &&
    ([
      "ServiceTimeout",
      "IpcTimeout",
      "ServiceBusy",
      "ServiceMonitorUnavailable",
    ].includes(error.code) ||
      (error.code === "IoError" && /service|ipc/.test(error.stage)))
  );
}

export function connectionUnconfirmed(
  snapshot: Snapshot | null | undefined,
): boolean {
  return !!snapshot?.engine_pid && serviceMonitorError(snapshot.error);
}

// A healthy observation dismisses only a stale service notice, never an unrelated failure.
export function recoveredNotice(
  current: NativeError | null,
  snapshot: Snapshot,
): NativeError | null {
  return serviceMonitorError(current) && !serviceMonitorError(snapshot.error)
    ? null
    : current;
}
