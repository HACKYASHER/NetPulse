import { HelpLabel, Help } from "./Help";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { parseLog, readAutoTail, startLogPolling } from "./fileLog";
const categories = [
  ["main", "Основной", "Main"],
  ["core", "Ядро", "Core"],
  ["tunnel", "Туннель", "Tunnel"],
  ["subscriptions", "Подписки", "Subscriptions"],
  ["service", "Служба", "Service"],
];
export function FileLogs({ lang }: { lang: string }) {
  const ru = lang === "ru";
  const [days, setDays] = useState<string[]>([]),
    [day, setDay] = useState("");
  const [category, setCategory] = useState("main"),
    [text, setText] = useState("");
  const [error, setError] = useState(""),
    [revision, setRevision] = useState(0);
  const [autoTail, setAutoTail] = useState(readAutoTail),
    [clearing, setClearing] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false),
    [copied, setCopied] = useState(false);
  const output = useRef<HTMLPreElement>(null);
  const latestDay = useRef("");
  const context = useRef("");
  useEffect(() => {
    let alive = true;
    const load = async () => {
      try {
        const dates = await invoke<string[]>("log_days");
        if (!alive) return;
        setDays(dates);
        const newest = dates[0] || "";
        if (!day || (autoTail && day === latestDay.current && newest !== day)) {
          latestDay.current = newest;
          if (day !== newest) {
            setDay(newest);
            return;
          }
        }
        latestDay.current = newest;
        const value = day
          ? await invoke<string>("log_read", { day, category })
          : "";
        if (alive) {
          setText(value);
          setError("");
        }
      } catch (e) {
        if (alive) setError(String((e as { message?: string })?.message || e));
      }
    };
    if (context.current !== day + ":" + category) {
      context.current = day + ":" + category;
      setText("");
    }
    const stop = startLogPolling(load, autoTail);
    return () => {
      alive = false;
      stop();
    };
  }, [day, category, revision, autoTail]);
  useEffect(() => {
    if (autoTail && output.current)
      output.current.scrollTop = output.current.scrollHeight;
  }, [text, autoTail, category]);
  const rows = parseLog(text, lang);
  return (
    <section className="card file-logs">
      <div className="section-top">
        <h3>{ru ? "Файловый журнал" : "File logs"}</h3>
        <div className="row log-controls">
          <HelpLabel topic="tail" className="check-label">
            <input
              type="checkbox"
              checked={autoTail}
              onChange={(e) => {
                setAutoTail(e.target.checked);
                try {
                  localStorage.setItem(
                    "netpulse.logAutoTail",
                    String(e.target.checked),
                  );
                } catch {
                  /* storage unavailable */
                }
              }}
            />
            {ru ? "Автопрокрутка" : "Follow log"}
          </HelpLabel>
          <span className="control-with-help">
            <select
              aria-label={ru ? "Дата журнала" : "Log date"}
              value={day}
              onChange={(e) => setDay(e.target.value)}
            >
              {!days.length && <option value="">—</option>}
              {days.map((d) => (
                <option key={d}>{d}</option>
              ))}
            </select>
            <Help topic="day" />
          </span>
          <button disabled={clearing} onClick={() => setRevision((v) => v + 1)}>
            {ru ? "Обновить" : "Refresh"}
          </button>
          <button
            onClick={() =>
              void invoke("open_log_folder").catch((e) =>
                setError(String(e?.message || e)),
              )
            }
          >
            {ru ? "Папка" : "Folder"}
          </button>
          <button
            disabled={!text}
            onClick={() =>
              void invoke("copy_log_text", {
                text: rows.map((r) => r.text).join("\n"),
              })
                .then(() => {
                  setCopied(true);
                  setTimeout(() => setCopied(false), 2000);
                })
                .catch((e) => setError(String(e?.message || e)))
            }
          >
            {copied
              ? ru
                ? "Скопировано"
                : "Copied"
              : ru
                ? "Копировать"
                : "Copy"}
          </button>
          <button disabled={clearing} onClick={() => setConfirmClear(true)}>
            {ru ? "Очистить все журналы" : "Clear all logs"}
          </button>
        </div>
      </div>
      {confirmClear && (
        <div className="banner" role="alert">
          <span className="grow">
            {ru
              ? "Очистить все разделы за все даты? Новые действия продолжат записываться."
              : "Clear all categories and dates? New actions will continue to be recorded."}
          </span>
          <button
            disabled={clearing}
            onClick={async () => {
              setClearing(true);
              try {
                await invoke("clear_file_logs");
                setText("");
                setDay("");
                latestDay.current = "";
                setRevision((v) => v + 1);
                setConfirmClear(false);
              } catch (e) {
                setError(String((e as { message?: string })?.message || e));
              } finally {
                setClearing(false);
              }
            }}
          >
            {ru ? "Очистить" : "Clear"}
          </button>
          <button disabled={clearing} onClick={() => setConfirmClear(false)}>
            {ru ? "Отмена" : "Cancel"}
          </button>
        </div>
      )}
      <div
        className="settings-tabs log-tabs"
        role="tablist"
        aria-label={ru ? "Журналы" : "Logs"}
      >
        {categories.map(([v, r, e]) => (
          <button
            key={v}
            role="tab"
            aria-selected={v === category}
            onClick={() => setCategory(v)}
          >
            {ru ? r : e}
          </button>
        ))}
      </div>
      {error && (
        <p role="alert" className="selectable">
          {error}
        </p>
      )}
      <pre
        ref={output}
        className="log-output selectable"
        tabIndex={0}
        aria-label={ru ? "Содержимое журнала" : "Log contents"}
      >
        {rows.length
          ? rows.map((r, i) => (
              <span className={"log-line log-" + r.level} key={i}>
                {r.text}
                {"\n"}
              </span>
            ))
          : ru
            ? "В этом разделе за выбранную дату записей пока нет."
            : "No entries for this category and date."}
      </pre>
      <p>
        {ru
          ? "Папки: logs/ГГГГ-ММ-ДД/. Последние 500 строк, до 256 КиБ. При включённой автопрокрутке журнал обновляется каждые 2 секунды. Основной журнал включает все разделы. Лимит архива — 128 МиБ."
          : "Folders: logs/YYYY-MM-DD/. Last 500 lines, up to 256 KiB. Follow log refreshes every 2 seconds. Main includes all categories. Archive limit: 128 MiB."}
      </p>
    </section>
  );
}
