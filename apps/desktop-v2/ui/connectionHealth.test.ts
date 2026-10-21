import { test } from "node:test";
import assert from "node:assert/strict";
import { connectionUnconfirmed, recoveredNotice } from "./connectionHealth.ts";
import type { Snapshot, NativeError } from "./types.ts";

const timeout: NativeError = {
  code: "ServiceTimeout",
  stage: "service-ipc",
  message: "timeout",
};
const snapshot = (error: NativeError | null, pid: number | null = 123) =>
  ({ error, engine_pid: pid, state: "connected" }) as Snapshot;
test("live engine with failed service monitoring is unconfirmed until a healthy observation", () => {
  assert.equal(connectionUnconfirmed(snapshot(timeout)), true);
  assert.equal(connectionUnconfirmed(snapshot(null)), false);
  assert.equal(connectionUnconfirmed(snapshot(timeout, null)), false);
  assert.equal(recoveredNotice(timeout, snapshot(null)), null);
  assert.equal(recoveredNotice(timeout, snapshot(timeout)), timeout);
});
test("recovery preserves unrelated errors and candidate failure does not mark active session unhealthy", () => {
  const candidate = {
    code: "ProbeTimeout",
    stage: "connection-probe",
    message: "candidate failed",
  };
  assert.equal(recoveredNotice(candidate, snapshot(null)), candidate);
  assert.equal(connectionUnconfirmed(snapshot(candidate)), false);
  const disk = { code: "IoError", stage: "storage", message: "read failed" };
  assert.equal(connectionUnconfirmed(snapshot(disk)), false);
  assert.equal(recoveredNotice(disk, snapshot(null)), disk);
});
