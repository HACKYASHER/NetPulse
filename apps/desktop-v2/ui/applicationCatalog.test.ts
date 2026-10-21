import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mergeCatalog,
  savedApplications,
  readCatalog,
  writeCatalog,
  clearCatalog,
  catalogRows,
  normalizedPath,
} from "./applicationCatalog.ts";
test("catalog folds known app versions but preserves unrelated same-name runtimes", () => {
  const app = (path: string) => ({ path, name: "codex", source: "manual" });
  const rows = catalogRows([
    app("C:/OpenAI/Codex/bin/6257d9e8f63c0f2b/codex.exe"),
    app("C:/OpenAI/Codex/bin/896a0166b774037a/codex.exe"),
    app("C:/Other/node.exe"),
    app("C:/OpenAI/node.exe"),
    app("c:\\OTHER\\NODE.exe"),
  ]);
  assert.equal(rows.length, 3);
  assert.equal(rows.find((r) => r.path.endsWith("codex.exe"))?.paths.length, 2);
  assert.equal(
    normalizedPath('"\\\\?\\C:\\Other\\NODE.exe"'),
    "c:\\other\\node.exe",
  );
});
const storage = () => {
  const data = new Map<string, string>();
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => {
      data.set(k, v);
    },
    removeItem: (k: string) => {
      data.delete(k);
    },
  };
};
test("deselected saved paths remain available after remount and scan", () => {
  const store = storage();
  const paths = [String.raw`C:\Discord\app-1.0\Discord.exe`];
  let catalog = mergeCatalog(savedApplications(paths));
  writeCatalog(store, catalog);
  catalog = mergeCatalog(savedApplications([]), readCatalog(store));
  const scanned = {
    path: String.raw`C:\Other\Tool.exe`,
    name: "Tool",
    source: "running_process",
  };
  catalog = mergeCatalog([scanned], catalog);
  assert.equal(catalog.length, 2);
  assert.equal(catalog[1].path, paths[0]);
  writeCatalog(store, catalog);
  assert.equal(readCatalog(store).length, 2);
  clearCatalog(store);
  assert.deepEqual(readCatalog(store), []);
});
test("scan metadata replaces fallback without duplicate paths or selection changes", () => {
  const paths = [String.raw`C:\Apps\Tool.exe`];
  const app = {
    path: paths[0].toLowerCase(),
    name: "Friendly name",
    source: "manual",
  };
  const merged = mergeCatalog(
    savedApplications(paths),
    [app],
    savedApplications(paths),
  );
  assert.deepEqual(merged, [app]);
  assert.equal(paths.length, 1);
});
test("invalid and unavailable storage cannot crash routing", () => {
  const store = storage();
  store.setItem("netpulse.application-catalog.v1", '{"bad":true}');
  assert.deepEqual(readCatalog(store), []);
  store.setItem(
    "netpulse.application-catalog.v1",
    JSON.stringify([
      null,
      { path: "bad" },
      { path: "C:\\Apps\\ok.exe", name: "OK", source: "saved" },
    ]),
  );
  assert.equal(readCatalog(store).length, 1);
  const blocked = {
    getItem: () => {
      throw new Error("blocked");
    },
    setItem: () => {
      throw new Error("full");
    },
    removeItem: () => {
      throw new Error("blocked");
    },
  };
  assert.deepEqual(readCatalog(blocked), []);
  assert.doesNotThrow(() => writeCatalog(blocked, []));
});
