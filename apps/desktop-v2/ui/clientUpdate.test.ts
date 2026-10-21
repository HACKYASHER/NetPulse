import { test } from "node:test";
import assert from "node:assert/strict";
import {
  pollClientUpdates,
  INITIAL_UPDATE_DELAY,
  UPDATE_CHECK_INTERVAL,
} from "./clientUpdate.ts";
function clock() {
  const tasks = new Map<string, { fn: () => void; ms: number }>();
  return {
    tasks,
    after: (fn: () => void, ms: number) => {
      tasks.set("initial", { fn, ms });
      return "initial";
    },
    every: (fn: () => void, ms: number) => {
      tasks.set("recurring", { fn, ms });
      return "recurring";
    },
    clearAfter: (id: unknown) => {
      tasks.delete(String(id));
    },
    clearEvery: (id: unknown) => {
      tasks.delete(String(id));
    },
  };
}
const flush = async () => {
  await Promise.resolve();
  await Promise.resolve();
};
test("startup check is delayed and later checks recur", async () => {
  const time = clock();
  let calls = 0;
  const received: unknown[] = [];
  const stop = pollClientUpdates(
    async () => {
      calls++;
      return null;
    },
    (r) => received.push(r),
    time,
  );
  assert.equal(calls, 0);
  assert.equal(time.tasks.get("initial")!.ms, INITIAL_UPDATE_DELAY);
  assert.equal(time.tasks.get("recurring")!.ms, UPDATE_CHECK_INTERVAL);
  time.tasks.get("initial")!.fn();
  await flush();
  time.tasks.get("recurring")!.fn();
  await flush();
  assert.equal(calls, 2);
  assert.deepEqual(received, [null, null]);
  stop();
  assert.equal(time.tasks.size, 0);
});
test("slow checks never overlap; closing ignores late completion", async () => {
  const time = clock();
  let calls = 0;
  let complete: (v: null) => void = () => {};
  const received: unknown[] = [];
  const stop = pollClientUpdates(
    () => {
      calls++;
      return new Promise((r) => {
        complete = r;
      });
    },
    (r) => received.push(r),
    time,
  );
  time.tasks.get("initial")!.fn();
  time.tasks.get("recurring")!.fn();
  assert.equal(calls, 1);
  stop();
  complete(null);
  await flush();
  assert.deepEqual(received, []);
});
test("offline failures do not disable later checks", async () => {
  const time = clock();
  let calls = 0;
  let results = 0;
  const stop = pollClientUpdates(
    async () => {
      if (++calls === 1) throw new Error("offline");
      return null;
    },
    () => results++,
    time,
  );
  time.tasks.get("initial")!.fn();
  await flush();
  assert.equal(results, 0);
  time.tasks.get("recurring")!.fn();
  await flush();
  assert.equal(results, 1);
  stop();
});
