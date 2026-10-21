import { test } from "node:test";
import assert from "node:assert/strict";
import { parseLog, readAutoTail, startLogPolling } from "./fileLog.ts";
test("old interaction audit is hidden while real interface failures remain", () => {
  const event = {
    timestamp: 1,
    level: "DEBUG",
    subsystem: "Interface",
    code: "Action",
    message: "button",
  };
  assert.deepEqual(parseLog(JSON.stringify(event), "en"), []);
  assert.equal(
    parseLog(
      JSON.stringify({ ...event, level: "ERROR", code: "UiRuntimeFailure" }),
      "en",
    )[0].level,
    "error",
  );
});
const flush = async () => {
  await Promise.resolve();
  await Promise.resolve();
};
function clock() {
  let task: (() => void) | undefined;
  return {
    after: (fn: () => void, ms: number) => {
      assert.equal(ms, 2000);
      task = fn;
      return 1 as unknown as ReturnType<typeof setTimeout>;
    },
    clear: () => {
      task = undefined;
    },
    get task() {
      return task;
    },
    fire: () => {
      const fn = task;
      task = undefined;
      fn?.();
    },
  };
}
test("new installs follow logs; saved opt-out and unavailable storage are respected", () => {
  assert.equal(readAutoTail({ getItem: () => null }), true);
  assert.equal(readAutoTail({ getItem: () => "false" }), false);
  assert.equal(
    readAutoTail({
      getItem: () => {
        throw Error("denied");
      },
    }),
    true,
  );
});
test("log parsing colours levels, retains malformed and UTF-8 lines, and treats HTML as text", () => {
  const event = {
    timestamp: 1,
    level: "ERROR",
    subsystem: "Core",
    code: "Test",
    message: "Ошибка <img onerror=alert(1)>",
  };
  const rows = parseLog(
    JSON.stringify(event) +
      "\nnot-json\n" +
      JSON.stringify({ ...event, level: "WARNING" }),
    "ru",
  );
  assert.equal(rows[0].level, "error");
  assert.ok(rows[0].text.includes(event.message));
  assert.deepEqual(rows[1], { level: "plain", text: "not-json" });
  assert.equal(rows[2].level, "warning");
  assert.equal(
    parseLog("null\n{}", "en").every((r) => r.level === "plain"),
    true,
  );
});
test("log polling waits for completion, never overlaps, and stops after leaving Activity", async () => {
  const time = clock();
  let calls = 0,
    complete = () => {};
  const stop = startLogPolling(
    () => {
      calls++;
      return new Promise<void>((r) => {
        complete = r;
      });
    },
    true,
    time,
  );
  assert.equal(calls, 1);
  assert.equal(time.task, undefined);
  complete();
  await flush();
  assert.ok(time.task);
  time.fire();
  assert.equal(calls, 2);
  assert.equal(time.task, undefined);
  stop();
  complete();
  await flush();
  assert.equal(time.task, undefined);
});
test("disabled follow performs one refresh; failed refresh can recover at next poll", async () => {
  const time = clock();
  let calls = 0;
  const once = startLogPolling(
    async () => {
      calls++;
    },
    false,
    time,
  );
  await flush();
  assert.equal(calls, 1);
  assert.equal(time.task, undefined);
  once();
  const stop = startLogPolling(
    async () => {
      if (++calls === 2) throw Error("offline");
    },
    true,
    time,
  );
  await flush();
  assert.ok(time.task);
  time.fire();
  await flush();
  assert.equal(calls, 3);
  stop();
  assert.equal(time.task, undefined);
});

test("legacy routine command entries are hidden without hiding command failures", () => {
  const e = {
    id: "fixture",
    timestamp: 1,
    level: "DEBUG",
    subsystem: "Command",
    stage: "snapshot",
    code: "Started",
    message: "Native command started",
  };
  const text = [
    e,
    { ...e, code: "Succeeded" },
    {
      ...e,
      level: "ERROR",
      code: "ServiceTimeout",
      message: "service timeout",
    },
  ]
    .map((v) => JSON.stringify(v))
    .join("\n");
  const rows = parseLog(text, "ru");
  assert.equal(rows.length, 1);
  assert.match(rows[0].text, /ServiceTimeout/);
});
