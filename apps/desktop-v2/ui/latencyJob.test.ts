import { test } from "node:test";
import assert from "node:assert/strict";
import { initialLatency, latencyReducer } from "./latencyJob.ts";
test("job remains busy from accepted start through cancellation, then becomes idle", () => {
  let s = latencyReducer(initialLatency, { type: "start" });
  assert.equal(s.phase, "starting");
  s = latencyReducer(s, { type: "accepted", id: "fixture" });
  assert.equal(s.phase, "running");
  s = latencyReducer(s, { type: "cancel" });
  assert.equal(s.id, "fixture");
  assert.equal(s.phase, "cancelling");
  assert.deepEqual(latencyReducer(s, { type: "finished" }), initialLatency);
});
test("failed start or polling releases controls and permits another job", () => {
  for (const phase of [
    "starting",
    "running",
    "cancelling",
    "recovering",
  ] as const) {
    const next = latencyReducer({ phase, id: "previous" }, { type: "failed" });
    assert.deepEqual(next, initialLatency);
    assert.equal(latencyReducer(next, { type: "start" }).phase, "starting");
  }
});
