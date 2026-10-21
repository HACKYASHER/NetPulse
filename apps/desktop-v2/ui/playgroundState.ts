export type ConnectionRow = {
  id: string;
  source: string;
  process: string;
  pid: number | null;
  network: string;
  local: string;
  destination: string;
  state: string;
  upload: number | null;
  download: number | null;
  route: string;
};
export type HistoryRow = ConnectionRow & { seen: number; active: boolean };
function searchable(text: string): string {
  return text
    .normalize("NFKC")
    .toLowerCase()
    .replace(/https?:\/\//g, "")
    .replaceAll("/", "\\");
}
function matches(text: string, query: string): boolean {
  const value = searchable(text);
  return searchable(query)
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .every((term) => value.includes(term));
}
export function matchesProcess(
  process: { name: string; path: string; pid: number },
  query: string,
): boolean {
  return matches(
    process.name + " " + process.path + " PID " + process.pid,
    query,
  );
}
export function matchesConnection(row: ConnectionRow, query: string): boolean {
  return matches(
    [
      row.process,
      row.pid ?? "",
      row.source,
      row.network,
      row.local,
      row.destination,
      row.state,
      row.route,
    ].join(" "),
    query,
  );
}
export function mergeConnections(
  old: HistoryRow[],
  current: ConnectionRow[],
  now: number,
): HistoryRow[] {
  const rows = new Map(
    old.map((row) => [`${row.source}:${row.id}`, { ...row, active: false }]),
  );
  for (const row of current)
    rows.set(`${row.source}:${row.id}`, { ...row, seen: now, active: true });
  return [...rows.values()].sort((a, b) => b.seen - a.seen).slice(0, 300);
}
