import type { Event } from "./types";
export function readAutoTail(storage?: Pick<Storage, "getItem">): boolean {
  try {
    return (
      (storage ?? globalThis.localStorage)?.getItem("netpulse.logAutoTail") !==
      "false"
    );
  } catch {
    return true;
  }
}
export function parseLog(text: string, lang: string) {
  return text
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      try {
        const e = JSON.parse(line) as Event;
        if (
          !Number.isFinite(e.timestamp) ||
          typeof e.message !== "string" ||
          typeof e.level !== "string"
        )
          throw Error();
        if (
          e.level === "DEBUG" &&
          (e.subsystem === "Interface" ||
            (e.subsystem === "Command" &&
              ["Started", "Succeeded"].includes(e.code)))
        )
          return null;
        const level = e.level.toLowerCase();
        return {
          level: [
            "error",
            "critical",
            "warning",
            "warn",
            "info",
            "debug",
            "trace",
          ].includes(level)
            ? level
            : "plain",
          text:
            "[" +
            new Date(e.timestamp * 1000).toLocaleTimeString(lang) +
            "] [" +
            e.level +
            "] " +
            e.subsystem +
            " · " +
            e.code +
            ": " +
            e.message,
        };
      } catch {
        return { level: "plain", text: line };
      }
    })
    .filter((v): v is NonNullable<typeof v> => v !== null);
}
export function startLogPolling(
  load: () => Promise<void>,
  recurring: boolean,
  clock = {
    after: (fn: () => void, ms: number) => setTimeout(fn, ms),
    clear: (id: ReturnType<typeof setTimeout>) => clearTimeout(id),
  },
) {
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const run = async () => {
    try {
      await load();
    } catch {
      /* A later poll may recover. */
    } finally {
      if (!stopped && recurring) timer = clock.after(() => void run(), 2000);
    }
  };
  void run();
  return () => {
    stopped = true;
    if (timer !== undefined) clock.clear(timer);
  };
}
