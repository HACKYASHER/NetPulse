import type { Application } from "./types";

const KEY = "netpulse.application-catalog.v1";
const MAX = 5000;
type Storage = Pick<globalThis.Storage, "getItem" | "setItem" | "removeItem">;
export const savedApplications = (paths: string[]): Application[] =>
  paths.map((path) => ({
    path,
    name: path.split(/[\\/]/).pop() || path,
    source: "saved",
  }));
export function mergeCatalog(...lists: Application[][]): Application[] {
  const result = new Map<string, Application>();
  for (const app of lists.flat()) {
    const key = normalizedPath(app.path);
    const previous = result.get(key);
    if (
      (!previous && result.size < MAX) ||
      (previous && app.source !== "saved")
    )
      result.set(key, app);
  }
  return [...result.values()];
}
export function readCatalog(storage?: Storage): Application[] {
  try {
    const raw = (storage ?? globalThis.localStorage).getItem(KEY);
    if (!raw || raw.length > 2_000_000) return [];
    const entries: unknown = JSON.parse(raw);
    if (!Array.isArray(entries)) return [];
    return mergeCatalog(
      entries
        .slice(0, MAX)
        .filter(
          (v): v is Application =>
            !!v &&
            typeof v === "object" &&
            typeof v.path === "string" &&
            v.path.length <= 4096 &&
            /^(?:[a-z]:[\\/]|\\\\).+\.exe$/i.test(v.path) &&
            !/[\x00-\x1f]/.test(v.path) &&
            typeof v.name === "string" &&
            v.name.length <= 512 &&
            typeof v.source === "string" &&
            v.source.length <= 128,
        ),
    );
  } catch {
    return [];
  }
}
export function writeCatalog(
  storage: Storage | undefined,
  apps: Application[],
) {
  try {
    (storage ?? globalThis.localStorage).setItem(
      KEY,
      JSON.stringify(apps.slice(0, MAX)),
    );
  } catch {
    /* Keep the in-memory list if storage is unavailable. */
  }
}
export function clearCatalog(storage?: Storage) {
  try {
    (storage ?? globalThis.localStorage).removeItem(KEY);
  } catch {
    /* Storage may be unavailable. */
  }
}

export function normalizedPath(path: string): string {
  let value = path.trim().replace(/^"|"$/g, "").replaceAll("/", "\\");
  if (value.startsWith("\\\\?\\UNC\\")) value = "\\\\" + value.slice(8);
  else if (value.startsWith("\\\\?\\")) value = value.slice(4);
  return value.toLowerCase();
}
// Fold version directories only inside known installation layouts. Unrelated
// executables with the same basename (e.g. Node runtimes) remain separate.
export function applicationIdentity(path: string): string {
  return normalizedPath(path)
    .replace(/(\\codex\\bin\\)[a-f0-9]{8,}(\\[^\\]+\.exe)$/i, "$1{version}$2")
    .replace(
      /(\\discord(?:ptb|canary)?\\)app-[0-9.]+(\\.*\.exe)$/i,
      "$1{version}$2",
    )
    .replace(
      /(\\roblox\\versions\\)version-[a-f0-9]+(\\[^\\]+\.exe)$/i,
      "$1{version}$2",
    );
}
export type ApplicationRow = Application & { paths: string[] };
export function catalogRows(apps: Application[]): ApplicationRow[] {
  const groups = new Map<string, ApplicationRow>();
  for (const app of mergeCatalog(apps)) {
    const id = applicationIdentity(app.path),
      old = groups.get(id);
    if (old) old.paths.push(app.path);
    else groups.set(id, { ...app, paths: [app.path] });
  }
  return [...groups.values()].sort(
    (a, b) => a.name.localeCompare(b.name) || a.path.localeCompare(b.path),
  );
}
