import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mergeConnections,
  matchesProcess,
  matchesConnection,
  type ConnectionRow,
} from "./playgroundState.ts";
test("process search works before observation by name, path and PID", () => {
  const process = {
    name: "Codex.exe",
    path: "C:\\OpenAI\\Codex\\codex.exe",
    pid: 1234,
  };
  assert.equal(matchesProcess(process, "  CODEX 1234 "), true);
  assert.equal(matchesProcess(process, "c:/openai/codex"), true);
  assert.equal(matchesProcess(process, "discord"), false);
  assert.equal(matchesProcess(process, "  "), true);
});
test("connection search trims clipboard whitespace and matches all displayed metadata", () => {
  const connection = { ...row("one"), pid: 1234 };
  assert.equal(
    matchesConnection(connection, "  FIXTURE.TEST:443   tcp  "),
    true,
  );
  assert.equal(matchesConnection(connection, "https://fixture.test:443"), true);
  assert.equal(matchesConnection(connection, "1234 proxy active"), true);
  assert.equal(matchesConnection(connection, "127.0.0.1:1234"), true);
  assert.equal(matchesConnection(connection, "fixture direct"), false);
});
const row = (id: string, source = "core"): ConnectionRow => ({
  id,
  source,
  process: "fixture.exe",
  pid: null,
  network: "tcp",
  local: "127.0.0.1:1234",
  destination: "fixture.test:443",
  state: "active",
  upload: 10,
  download: 20,
  route: "proxy",
});
test("observation retains closed rows, refreshes counters and separates OS/core identities", () => {
  const first = mergeConnections(
    [],
    [row("one"), row("one", "windows"), row("old")],
    10,
  );
  const next = mergeConnections(first, [{ ...row("one"), download: 100 }], 20);
  assert.equal(next.length, 3);
  assert.equal(next[0].download, 100);
  assert.equal(next.filter((r) => r.active).length, 1);
  assert.equal(next.find((r) => r.source === "windows")?.active, false);
  assert.equal(
    first.find((r) => r.id === "one" && r.source === "core")?.download,
    20,
  );
});
test("observation remains bounded and evicts oldest entries before current connections", () => {
  const old = mergeConnections(
    [],
    Array.from({ length: 300 }, (_, i) => row(`old${i}`)),
    1,
  );
  const next = mergeConnections(
    old,
    Array.from({ length: 40 }, (_, i) => row(`new${i}`)),
    2,
  );
  assert.equal(next.length, 300);
  assert.equal(next.filter((v) => v.id.startsWith("new")).length, 40);
  assert.equal(new Set(next.map((v) => `${v.source}:${v.id}`)).size, 300);
});
