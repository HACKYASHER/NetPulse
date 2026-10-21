import { HelpLabel, Help } from "./Help";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Sparkles, Square } from "lucide-react";
import type { NativeError } from "./types";
type Job = {
  id: string;
  kind: string;
  running: boolean;
  completed: number;
  total: number;
  cancelled: boolean;
  error: NativeError | null;
};
export function BestServer(p: {
  lang: string;
  busy: boolean;
  count: number;
  enabled: boolean;
  samples: number;
  change: (
    key: "auto_failover" | "selection_samples",
    value: boolean | number,
  ) => void;
  refresh: () => Promise<unknown>;
  error: (e: NativeError) => void;
}) {
  const ru = p.lang === "ru";
  const [job, setJob] = useState<Job | null>(null),
    [starting, setStarting] = useState(false);
  const guard = useRef(false);
  useEffect(() => {
    const load = () =>
      invoke<Job | null>("current_diagnostic")
        .then((j) => {
          if (j?.kind === "best_server") setJob(j);
        })
        .catch(p.error);
    void load();
    const off = listen("automatic-selection", load);
    return () => {
      void off.then((f) => f());
    };
  }, []);
  useEffect(() => {
    if (!job?.running) return;
    let stopped = false,
      failures = 0;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const value = await invoke<Job>("diagnostic_status", { id: job.id });
        if (stopped) return;
        failures = 0;
        setJob(value);
        if (value.running) timer = setTimeout(poll, 700);
        else {
          if (value.error) p.error(value.error);
          await p.refresh();
        }
      } catch (e) {
        if (stopped) return;
        if (++failures < 3) timer = setTimeout(poll, 1500);
        else {
          setJob(null);
          p.error(e as NativeError);
        }
      }
    };
    void poll();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [job?.id, job?.running]);
  return (
    <section className="card best-server">
      <div className="section-top">
        <div>
          <h3>{ru ? "Лучший сервер" : "Best server"}</h3>
          <p>
            {ru
              ? "Проверка HTTPS через каждый профиль. Выбор по потерям, медиане пинга и стабильности."
              : "HTTPS through each profile, ranked by loss, median latency and stability."}
          </p>
          <p className="small warning">
            {ru
              ? "Экспериментальная оценка: выбор может быть неточным и работать с недочётами."
              : "Experimental estimate: selection may be inaccurate and have limitations."}
          </p>
        </div>
        <button
          className="primary"
          disabled={p.busy || starting || !!job?.running || !p.count}
          onClick={async () => {
            if (guard.current) return;
            guard.current = true;
            setStarting(true);
            try {
              const id = await invoke<string>("start_best_server", {
                samples: p.samples,
              });
              setJob({
                id,
                kind: "best_server",
                running: true,
                completed: 0,
                total: p.count,
                cancelled: false,
                error: null,
              });
            } catch (e) {
              p.error(e as NativeError);
            } finally {
              setStarting(false);
              guard.current = false;
            }
          }}
        >
          <Sparkles size={16} />
          {ru ? "Выбрать лучший" : "Select best"}
        </button>
      </div>
      <div className="data-actions">
        <HelpLabel topic="samples">
          {ru ? "Проверок на сервер" : "Samples per server"}
          <select
            value={p.samples}
            disabled={starting || !!job?.running}
            onChange={(e) =>
              p.change("selection_samples", Number(e.target.value))
            }
          >
            {[5, 10, 15].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </HelpLabel>
        <HelpLabel topic="failover" className="check-label">
          <input
            type="checkbox"
            checked={p.enabled}
            onChange={(e) => p.change("auto_failover", e.target.checked)}
          />
          {ru
            ? "Автоматически менять недоступный сервер"
            : "Switch automatically if the server is unavailable"}
        </HelpLabel>
        {job?.running && (
          <button
            onClick={() =>
              invoke("cancel_diagnostic", { id: job.id }).catch(p.error)
            }
          >
            <Square size={14} />
            {ru ? "Отменить" : "Cancel"}
          </button>
        )}
      </div>
      <p className="small muted">
        {job
          ? `${job.running ? (ru ? "Проверено" : "Tested") : job.cancelled ? (ru ? "Отменено" : "Cancelled") : ru ? "Завершено" : "Finished"}: ${job.completed}/${job.total} · `
          : ""}
        {ru
          ? "Без VPN выбирается профиль для следующего подключения. При работающем VPN происходит переключение. Автопереключение: после 3 неудачных HTTPS-проверок с интервалом 10 секунд; настройки нужно сохранить. Переключение может прервать текущие TCP- и игровые сессии; бесшовное сохранение матча не гарантируется."
          : "When disconnected, selection prepares the next connection. While connected, it switches servers. Automatic failover follows 3 failed HTTPS checks, 10 seconds apart. Save settings to enable it. Switching may interrupt existing TCP and game sessions; seamless migration is not guaranteed."}
      </p>
    </section>
  );
}
