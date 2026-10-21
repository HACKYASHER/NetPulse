import { HelpLabel, Help } from "./Help";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity, Play, Square } from "lucide-react";
import type { Profile, Subscription, NativeError } from "./types";
type Latency = {
  profile_id: string;
  method: string;
  timestamp: number;
  min_ms: number | null;
  avg_ms: number | null;
  max_ms: number | null;
  successes: number;
  timeouts: number;
  failures: number;
  note: string;
  error: NativeError | null;
};
type Speed = {
  idle_https_ms: number | null;
  loaded_latency_ms: number | null;
  jitter_ms: number | null;
  mode: string;
  timestamp: number;
  route: string;
  download_mbps: number | null;
  upload_mbps: number | null;
  download_bytes: number;
  upload_bytes: number;
  cancelled: boolean;
  error: NativeError | null;
  samples: { elapsed_ms: number; bytes: number; direction: string }[];
};
type Job = {
  id: string;
  kind: string;
  running: boolean;
  cancelled: boolean;
  completed: number;
  total: number;
  results: (Latency | Speed)[];
  error: NativeError | null;
  progress: { bytes: number; elapsed_ms: number; direction: string } | null;
};
const number = (v: number | null) => (v == null ? "—" : v.toFixed(1));
export function DiagnosticsPanel(p: {
  speedOnly?: boolean;
  profiles: Profile[];
  subscriptions: Subscription[];
  selected: string;
  active: boolean;
  lang: string;
  jobId: string;
  setJobId: (v: string) => void;
  error: (v: NativeError) => void;
}) {
  const ru = p.lang === "ru";
  const [method, setMethod] = useState("proxy_https_rtt"),
    [source, setSource] = useState(""),
    [job, setJob] = useState<Job | null>(null),
    [pending, setPending] = useState(false),
    [latency, setLatency] = useState<Latency[]>([]),
    [speed, setSpeed] = useState<Speed[]>([]);
  const [mode, setMode] = useState("direct"),
    [download, setDownload] = useState(""),
    [upload, setUpload] = useState(""),
    [confirmed, setConfirmed] = useState(false),
    [duration, setDuration] = useState(10),
    [limit, setLimit] = useState(20),
    [traffic, setTraffic] = useState(false);
  const refresh = async () => {
    const [a, b] = await Promise.all([
      invoke<Latency[]>("diagnostic_history", { kind: "latency" }),
      invoke<Speed[]>("diagnostic_history", { kind: "speed" }),
    ]);
    setLatency(a);
    setSpeed(b);
  };
  useEffect(() => {
    refresh().catch(p.error);
  }, []);
  useEffect(() => {
    if (!p.jobId) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const state = await invoke<Job>("diagnostic_status", { id: p.jobId });
        if (disposed) return;
        setJob(state);
        if (state.running) timer = setTimeout(poll, 500);
        else await refresh();
      } catch (e) {
        if (!disposed) p.error(e as NativeError);
      }
    };
    poll();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [p.jobId]);
  const start = async (command: string, args: Record<string, unknown>) => {
    setPending(true);
    try {
      const id = await invoke<string>(command, args);
      setJob(null);
      p.setJobId(id);
    } catch (e) {
      p.error(e as NativeError);
    } finally {
      setPending(false);
    }
  };
  const running = pending || !!job?.running;
  const ids = p.profiles
    .filter((v) => !source || v.subscription_id === source)
    .map((v) => v.id);
  const last = speed[0];
  return (
    <section className="card diagnostics">
      <div className="section-top">
        <div>
          <h3>
            <Activity size={18} />{" "}
            {ru ? "Диагностика соединения" : "Connection diagnostics"}
          </h3>
          <p>
            {ru
              ? "Отдельные измерения по запросу. Счётчики текущего трафика не являются тестом скорости."
              : "Explicit measurements. Live traffic counters are separate from speed tests."}
          </p>
        </div>
        {running && (
          <button
            onClick={() =>
              invoke("cancel_diagnostic", { id: p.jobId }).catch(p.error)
            }
          >
            <Square size={16} />
            {ru ? "Отменить" : "Cancel"}
          </button>
        )}
      </div>
      {job && (
        <p role="status">
          {job.running
            ? ru
              ? "Выполняется"
              : "Running"
            : job.cancelled
              ? ru
                ? "Отменено"
                : "Cancelled"
              : ru
                ? "Завершено"
                : "Finished"}{" "}
          · {job.completed}/{job.total}
        </p>
      )}
      {job?.running && job.progress && (
        <p>
          {job.progress.direction} ·{" "}
          {(job.progress.bytes / 1024 / 1024).toFixed(2)} MiB ·{" "}
          {(job.progress.elapsed_ms / 1000).toFixed(1)} s
        </p>
      )}
      {!p.speedOnly && (
        <>
          <h4>{ru ? "Задержка" : "Latency"}</h4>
          <div className="data-actions">
            <span className="control-with-help">
              <select
                aria-label="Latency method"
                value={method}
                onChange={(e) => setMethod(e.target.value)}
              >
                <option value="proxy_https_rtt">Proxy HTTPS RTT</option>
                <option value="tcp_connect_rtt">TCP Connect RTT</option>
              </select>
              <Help topic="method" />
            </span>
            <span className="control-with-help">
              <select
                aria-label="Source for latency test"
                value={source}
                onChange={(e) => setSource(e.target.value)}
              >
                <option value="">{ru ? "Все подписки" : "All sources"}</option>
                {p.subscriptions.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
              <Help topic="source" />
            </span>
            <button
              disabled={running || !p.selected}
              onClick={() =>
                start("start_latency", { ids: [p.selected], method })
              }
            >
              <Play size={15} />
              {ru ? "Выбранный" : "Selected"}
            </button>
            <button
              disabled={running || !ids.length}
              onClick={() => start("start_latency", { ids, method })}
            >
              {ru ? "Все в списке" : "All in list"} ({ids.length})
            </button>
          </div>
          <p className="small muted">
            {ru
              ? "TCP показывает только установление TCP-соединения и не проверяет Hysteria2/UDP. HTTPS проверяет выбранный профиль, включая TLS и авторизацию. Одновременно проверяются не более двух серверов."
              : "TCP proves only a TCP handshake, not Hysteria2/UDP. HTTPS traverses the selected profile with TLS and authentication. At most two profiles run concurrently."}
          </p>
          {!!latency.length && (
            <div className="diagnostic-table">
              <table>
                <thead>
                  <tr>
                    {[
                      ru ? "Сервер" : "Server",
                      "Method",
                      "min / avg / max, ms",
                      "✓ / timeout / error",
                      ru ? "Время" : "Time",
                    ].map((h) => (
                      <th key={h}>{h}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {latency.slice(0, 8).map((v, i) => (
                    <tr key={i} title={v.error?.message || v.note}>
                      <td>
                        {p.profiles.find((a) => a.id === v.profile_id)?.name ||
                          "—"}
                      </td>
                      <td>
                        {v.method === "proxy_https_rtt" ? "HTTPS" : "TCP"}
                      </td>
                      <td>
                        {number(v.min_ms)} / {number(v.avg_ms)} /{" "}
                        {number(v.max_ms)}
                      </td>
                      <td>
                        {v.successes} / {v.timeouts} / {v.failures}
                      </td>
                      <td>{new Date(v.timestamp * 1000).toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
      <h4>Speed Test</h4>
      <div className="form-grid">
        <HelpLabel topic="testRoute">
          {ru ? "Маршрут" : "Route"}
          <select value={mode} onChange={(e) => setMode(e.target.value)}>
            <option value="direct">Direct</option>
            <option value="vpn" disabled={!p.active}>
              VPN
            </option>
            <option value="compare" disabled={!p.active}>
              Compare
            </option>
          </select>
        </HelpLabel>
        <HelpLabel topic="duration">
          {ru ? "Длительность фазы, сек." : "Phase duration, sec."}
          <input
            type="number"
            min={2}
            max={30}
            value={duration}
            onChange={(e) => setDuration(Number(e.target.value))}
          />
        </HelpLabel>
        <HelpLabel topic="cap">
          {ru ? "Общий лимит данных, MiB" : "Total payload cap, MiB"}
          <input
            type="number"
            min={1}
            max={100}
            value={limit}
            onChange={(e) => setLimit(Number(e.target.value))}
          />
        </HelpLabel>
        <HelpLabel topic="download">
          {ru
            ? "Тестовый сервер — HTTPS download URL"
            : "Test server — HTTPS download URL"}
          <input
            type="url"
            value={download}
            placeholder="https://your-test-server/download"
            onChange={(e) => setDownload(e.target.value)}
          />
        </HelpLabel>
        <HelpLabel topic="upload">
          {ru
            ? "HTTPS upload URL (необязательно)"
            : "HTTPS upload URL (optional)"}
          <input
            type="url"
            value={upload}
            onChange={(e) => setUpload(e.target.value)}
          />
        </HelpLabel>
      </div>
      {upload && (
        <HelpLabel topic="post" className="check-label">
          <input
            type="checkbox"
            checked={confirmed}
            onChange={(e) => setConfirmed(e.target.checked)}
          />
          {ru
            ? "Этот сервер специально принимает тестовые POST-загрузки."
            : "This endpoint is designed to accept test POST uploads."}
        </HelpLabel>
      )}
      <HelpLabel topic="traffic" className="check-label">
        <input
          type="checkbox"
          checked={traffic}
          onChange={(e) => setTraffic(e.target.checked)}
        />
        {ru
          ? `Разрешаю тесту передать до ${limit} MiB данных. Служебный трафик TLS/HTTP добавляется сверх этого лимита.`
          : `Allow up to ${limit} MiB payload. TLS/HTTP overhead is additional.`}
      </HelpLabel>
      <p className="small muted">
        {ru
          ? "Compare делит лимит между Direct и VPN. Загрузка на сервер ограничена 4 MiB. При активном NetPulse оба маршрута закреплены отдельными диагностическими входами. Без подключения Direct использует маршрут Windows и не обходит сторонние VPN. Отмена проверяется между операциями; ожидание сети ограничено длительностью фазы."
          : "Compare splits the cap across Direct and VPN. Upload is limited to 4 MiB. With NetPulse connected, diagnostic inbounds force the selected route. Disconnected Direct uses the Windows route and does not bypass external VPNs. Cancellation is checked between operations; pending requests have a phase timeout."}
      </p>
      <button
        className="primary"
        disabled={running || !traffic || !download || (!!upload && !confirmed)}
        onClick={() =>
          start("start_speed", {
            request: {
              mode,
              download_url: download,
              upload_url: upload || null,
              upload_endpoint_confirmed: confirmed,
              duration_seconds: duration,
              max_payload_mib: limit,
            },
          })
        }
      >
        <Play size={16} />
        {ru ? "Начать тест" : "Start test"}
      </button>
      {last && (
        <div className="speed-summary">
          <h4>
            {ru ? "Последний тест" : "Last speed test"} ·{" "}
            {last.mode.toUpperCase()}
          </h4>
          <p>
            {new Date(last.timestamp * 1000).toLocaleString()} · ↓{" "}
            {number(last.download_mbps)} Mbps · ↑ {number(last.upload_mbps)}{" "}
            Mbps
          </p>
          <p className="small muted">{last.route}</p>
          {last.error && (
            <p className="warning">
              {last.error.code}: {last.error.message}
            </p>
          )}
          {last.samples.length > 1 && (
            <svg
              viewBox="0 0 600 100"
              role="img"
              aria-label="Transferred payload over time"
            >
              {(["download", "upload"] as const).map((direction) => (
                <polyline
                  key={direction}
                  fill="none"
                  stroke={
                    direction === "download" ? "var(--accent)" : "#39CFA3"
                  }
                  strokeWidth="2"
                  points={last.samples
                    .filter((s) => s.direction === direction)
                    .map(
                      (s) =>
                        `${(s.elapsed_ms / Math.max(...last.samples.map((v) => v.elapsed_ms), 1)) * 600},${95 - (s.bytes / Math.max(...last.samples.map((v) => v.bytes), 1)) * 90}`,
                    )
                    .join(" ")}
                />
              ))}
            </svg>
          )}
          <p>
            HTTPS idle: {number(last.idle_https_ms)} ms · loaded:{" "}
            {number(last.loaded_latency_ms)} ms · RTT jitter:{" "}
            {number(last.jitter_ms)} ms
          </p>
          <p className="small muted">
            {ru
              ? "График: переданные данные по времени. Задержка — HTTPS HEAD; джиттер — средняя разница последовательных RTT. «—» означает, что сервер не поддержал измерение или данных недостаточно."
              : "Chart: payload transferred over time. Latency uses HTTPS HEAD; jitter is mean consecutive RTT variation. Unavailable measurements appear as —."}
          </p>
        </div>
      )}
      {speed.slice(1, 5).map((v, i) => (
        <p key={i} className="small muted">
          {new Date(v.timestamp * 1000).toLocaleString()} · {v.mode} · ↓{" "}
          {number(v.download_mbps)} / ↑ {number(v.upload_mbps)} Mbps{" "}
          {v.error ? `· ${v.error.code}` : ""}
        </p>
      ))}
    </section>
  );
}
