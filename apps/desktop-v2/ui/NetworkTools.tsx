import { HelpLabel, Help } from "./Help";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { NativeError, Preferences } from "./types";
import {
  mergeConnections,
  matchesConnection,
  matchesProcess,
  type HistoryRow,
} from "./playgroundState";
import { RefreshCw, Play, Square } from "lucide-react";
const failure = (e: unknown): NativeError =>
  typeof e === "object" && e !== null && "message" in e
    ? (e as NativeError)
    : { code: "NetworkTools", stage: "network", message: String(e) };
export function ProfileNetwork({
  draft: p,
  change,
  lang,
}: {
  draft: Preferences;
  change: (p: Preferences) => void;
  lang: string;
}) {
  const ru = lang === "ru";
  const v = p.tls_defaults ?? {
    sni_mode: "inherit",
    server_name: "",
    http_host: null,
    fingerprint: null,
  };
  const set = (patch: Partial<typeof v>) =>
    change({ ...p, tls_defaults: { ...v, ...patch } });
  return (
    <section className="card">
      <h3>{ru ? "Общие SNI и TLS" : "Global SNI and TLS"}</h3>
      <p>
        {ru
          ? "Применяются ко всем совместимым серверам после сохранения настроек. Проверка сертификатов остаётся включённой. Настройки сервера из подписки используются, если не задано переопределение."
          : "Apply to all compatible servers after saving. Certificate validation stays enabled. Original server settings apply unless overridden."}
      </p>
      <div className="form-grid">
        <HelpLabel topic="sniMode">
          SNI
          <select
            value={v.sni_mode}
            onChange={(e) => set({ sni_mode: e.target.value })}
          >
            <option value="inherit">
              {ru ? "Из конфигурации сервера" : "Server configuration"}
            </option>
            <option value="auto">
              {ru ? "По адресу сервера" : "Server address"}
            </option>
            <option value="custom">{ru ? "Своё имя" : "Custom name"}</option>
            <option value="disabled">
              {ru
                ? "Не отправлять SNI (sing-box без Reality)"
                : "Omit SNI (sing-box without Reality)"}
            </option>
          </select>
        </HelpLabel>
        {["custom", "disabled"].includes(v.sni_mode) && (
          <HelpLabel topic="certName">
            {ru ? "Имя для проверки сертификата" : "Certificate name"}
            <input
              value={v.server_name}
              placeholder="vpn.example.org"
              onChange={(e) => set({ server_name: e.target.value })}
            />
          </HelpLabel>
        )}
        <HelpLabel topic="httpHost">
          HTTP Host
          <input
            value={v.http_host ?? ""}
            placeholder={
              ru ? "Из конфигурации сервера" : "Server configuration"
            }
            onChange={(e) => set({ http_host: e.target.value || null })}
          />
        </HelpLabel>
        <HelpLabel topic="utls">
          uTLS
          <select
            value={v.fingerprint ?? "inherit"}
            onChange={(e) =>
              set({
                fingerprint:
                  e.target.value === "inherit" ? null : e.target.value,
              })
            }
          >
            <option value="inherit">
              {ru ? "Из конфигурации сервера" : "Server configuration"}
            </option>
            <option value="disabled">
              {ru ? "Стандартный TLS ядра" : "Standard core TLS"}
            </option>
            {[
              "chrome",
              "firefox",
              "edge",
              "safari",
              "ios",
              "android",
              "randomized",
            ].map((f) => (
              <option key={f}>{f}</option>
            ))}
          </select>
        </HelpLabel>
      </div>
      <p>
        {ru
          ? "HTTP Host применяется к WebSocket, XHTTP и HTTPUpgrade; uTLS — к TCP TLS, кроме Hysteria2. Reality требует SNI и uTLS: несовместимые настройки будут отклонены до переподключения. Изменение общего имени может нарушить доступ к серверам с другими сертификатами."
          : "HTTP Host applies to WebSocket, XHTTP and HTTPUpgrade; uTLS to TCP TLS, excluding Hysteria2. Reality requires SNI and uTLS. Incompatible settings are rejected before reconnection. A shared name can break servers with different certificates."}
      </p>
      <button onClick={() => change({ ...p, tls_defaults: null })}>
        {ru ? "Использовать настройки серверов" : "Use server settings"}
      </button>
    </section>
  );
}
export function TransportSettings({
  draft: p,
  change,
  lang,
}: {
  draft: Preferences;
  change: (p: Preferences) => void;
  lang: string;
}) {
  const ru = lang === "ru",
    f = p.fragmentation,
    m = p.multiplex;
  const fragment = (v: Partial<typeof f>) =>
    change({ ...p, fragmentation: { ...f, ...v } });
  const mux = (v: Partial<typeof m>) =>
    change({ ...p, multiplex: { ...m, ...v } });
  return (
    <section className="card">
      <h3>
        {ru ? "Фрагментация и мультиплексор" : "Fragmentation and multiplexing"}
      </h3>
      <p>
        {ru
          ? "Необязательные функции Xray. Применяются кнопкой «Сохранить». Несовместимые сочетания проверяются перед остановкой работающего соединения."
          : "Optional Xray features. Apply with Save. Unsupported combinations are rejected before stopping an active connection."}
      </p>
      <HelpLabel topic="fragment" className="check-label">
        <input
          type="checkbox"
          checked={f.enabled}
          onChange={(e) => fragment({ enabled: e.target.checked })}
        />
        {ru ? "Включить фрагментацию" : "Enable fragmentation"}
      </HelpLabel>
      <fieldset disabled={!f.enabled} className="network-fieldset">
        <div className="form-grid">
          <HelpLabel topic="packets">
            {ru ? "Пакеты" : "Packets"}
            <select
              value={f.packets}
              onChange={(e) =>
                fragment({ packets: e.target.value as typeof f.packets })
              }
            >
              <option value="tlshello">TLS ClientHello</option>
              <option value="1-3">
                {ru ? "Первые 3 записи TCP" : "First 3 TCP writes"}
              </option>
            </select>
          </HelpLabel>
          <HelpLabel topic="length">
            {ru ? "Длина, байт (мин-макс)" : "Length, bytes (min-max)"}
            <input
              value={f.length}
              onChange={(e) => fragment({ length: e.target.value })}
            />
          </HelpLabel>
          <HelpLabel topic="interval">
            {ru ? "Интервал, мс (мин-макс)" : "Interval, ms (min-max)"}
            <input
              value={f.interval}
              onChange={(e) => fragment({ interval: e.target.value })}
            />
          </HelpLabel>
          <HelpLabel topic="split">
            maxSplit ({ru ? "мин-макс" : "min-max"})
            <input
              value={f.max_split}
              onChange={(e) => fragment({ max_split: e.target.value })}
            />
          </HelpLabel>
        </div>
      </fieldset>
      <p>
        {ru
          ? "Xray выбирает значения внутри диапазонов. Фрагментируется внешний TCP-транспорт к VPN-серверу; Hysteria2 / QUIC не поддерживаются. Большая задержка может снизить скорость."
          : "Xray samples values within these ranges. Fragments the outer TCP VPN transport, not Hysteria2 / QUIC. Delays can reduce throughput."}
      </p>
      <HelpLabel topic="mux" className="check-label">
        <input
          type="checkbox"
          checked={m.enabled}
          onChange={(e) => mux({ enabled: e.target.checked })}
        />
        {ru ? "Включить Mux / XUDP" : "Enable Mux / XUDP"}
      </HelpLabel>
      <fieldset disabled={!m.enabled} className="network-fieldset">
        <div className="form-grid">
          <HelpLabel topic="tcpMux">
            TCP (-1…128)
            <input
              type="number"
              min={-1}
              max={128}
              value={m.concurrency}
              onChange={(e) => mux({ concurrency: Number(e.target.value) })}
            />
          </HelpLabel>
          <HelpLabel topic="xudp">
            XUDP (-1…1024)
            <input
              type="number"
              min={-1}
              max={1024}
              value={m.xudp_concurrency}
              onChange={(e) =>
                mux({ xudp_concurrency: Number(e.target.value) })
              }
            />
          </HelpLabel>
          <HelpLabel topic="quic">
            QUIC / UDP 443
            <select
              value={m.udp_443}
              onChange={(e) =>
                mux({ udp_443: e.target.value as typeof m.udp_443 })
              }
            >
              <option value="skip">{ru ? "Вне Mux" : "Outside Mux"}</option>
              <option value="allow">
                {ru ? "Разрешить внутри Mux" : "Allow in Mux"}
              </option>
              <option value="reject">{ru ? "Отклонять" : "Reject"}</option>
            </select>
          </HelpLabel>
        </div>
      </fieldset>
      <p>
        {ru
          ? "Mux: VLESS / Trojan TCP и WebSocket без Vision. gRPC и XHTTP используют собственный транспорт. Mux не ускоряет большие загрузки и может ухудшить их. -1 отключает соответствующий канал; 0 использует стандартное поведение ядра."
          : "Mux supports VLESS / Trojan TCP and WebSocket without Vision. gRPC / XHTTP use their own transport. Mux may reduce bulk download speeds. -1 disables that channel; 0 uses core defaults."}
      </p>
    </section>
  );
}
type Process = { pid: number; name: string; path: string };
export type ObservedConnection = {
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
type Analysis = {
  connections: ObservedConnection[];
  process_alive: boolean;
  errors: string[];
  core_available: boolean;
  truncated: boolean;
};
export function Playground({
  lang,
  visible,
  smoke,
}: {
  lang: string;
  visible: boolean;
  smoke?: boolean;
}) {
  const ru = lang === "ru";
  const [processes, setProcesses] = useState<Process[]>([]),
    [selected, setSelected] = useState(""),
    [filter, setFilter] = useState(""),
    [processFilter, setProcessFilter] = useState("");
  const [running, setRunning] = useState(false),
    [loading, setLoading] = useState(false),
    [analysis, setAnalysis] = useState<Analysis | null>(null),
    [error, setError] = useState("");
  const [refreshRate, setRefreshRate] = useState(0);
  const [refreshTick, setRefreshTick] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const inFlight = useRef(false);
  const [history, setHistory] = useState<HistoryRow[]>([]);
  const chosen = processes.find((p) => String(p.pid) === selected);
  const mount = useRef(true);
  useEffect(() => {
    mount.current = true;
    return () => {
      mount.current = false;
    };
  }, []);
  const scan = async () => {
    setLoading(true);
    try {
      const v = await invoke<Process[]>("playground_processes");
      if (mount.current) setProcesses(v);
    } catch (e) {
      if (mount.current) setError(failure(e).message);
    } finally {
      if (mount.current) setLoading(false);
    }
  };
  const scanned = useRef(false);
  useEffect(() => {
    if (visible && !scanned.current && !smoke) {
      scanned.current = true;
      void scan();
    }
  }, [visible, smoke]);
  const foundProcesses = processes.filter((process) =>
    matchesProcess(process, processFilter),
  );
  const shownConnections = history.filter((connection) =>
    matchesConnection(connection, filter),
  );
  useEffect(() => {
    if (!visible) setRunning(false);
  }, [visible]);
  useEffect(() => {
    if (!running || !visible) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (inFlight.current) {
        timer = setTimeout(poll, 50);
        return;
      }
      inFlight.current = true;
      setRefreshing(true);
      try {
        const result = await invoke<Analysis>("connection_analysis", {
          pid: chosen?.pid ?? null,
          path: chosen?.path ?? "",
        });
        if (!alive) return;
        setAnalysis(result);
        setError("");
        setHistory((old) =>
          mergeConnections(old, result.connections, Date.now()),
        );
        if (chosen && !result.process_alive) {
          setRunning(false);
          return;
        }
      } catch (e) {
        if (alive) setError(failure(e).message);
      } finally {
        inFlight.current = false;
        if (mount.current) setRefreshing(false);
        if (alive && refreshRate > 0) timer = setTimeout(poll, refreshRate);
      }
    };
    void poll();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [running, visible, selected, refreshRate, refreshTick]);
  return (
    <section className="card playground">
      <div className="section-top">
        <h3>Playground · {ru ? "Анализ соединений" : "Connection analysis"}</h3>
        <button disabled={loading || running} onClick={scan}>
          <RefreshCw size={15} />
          {loading
            ? ru
              ? "Чтение…"
              : "Reading…"
            : ru
              ? "Найти процессы"
              : "Find processes"}
        </button>
      </div>
      <p>
        {ru
          ? "Выберите запущенный процесс. Windows показывает TCP-состояния и локальные UDP-сокеты даже без VPN. Ядро дополняет захваченные соединения адресами, маршрутом и байтами. Приложение не запускается и его настройки не меняются."
          : "Select a running process. Windows provides TCP states and UDP bindings even without VPN. The core adds destination, route and byte counters for captured traffic. This does not launch or modify the app."}
      </p>
      <div className="form-grid playground-searches">
        <div>
          <HelpLabel topic="processSearch">
            {ru ? "Найти процесс" : "Find process"}
            <input
              type="search"
              value={processFilter}
              disabled={running}
              placeholder={
                ru ? "Имя EXE, путь или PID" : "EXE name, path or PID"
              }
              onChange={(e) => setProcessFilter(e.target.value)}
            />
          </HelpLabel>
          <HelpLabel topic="process">
            {ru ? "Процесс для наблюдения" : "Process to observe"}
            <select
              disabled={running}
              value={selected}
              onChange={(e) => {
                setSelected(e.target.value);
                setHistory([]);
                setAnalysis(null);
              }}
            >
              <option value="">
                {ru ? "Все соединения ядра" : "All core connections"}
              </option>
              {processes
                .filter(
                  (process) =>
                    String(process.pid) === selected ||
                    matchesProcess(process, processFilter),
                )
                .map((process) => (
                  <option key={process.pid} value={String(process.pid)}>
                    {process.name} · PID {process.pid}
                  </option>
                ))}
            </select>
          </HelpLabel>
          <p className="small muted" role="status">
            {loading
              ? ru
                ? "Чтение процессов…"
                : "Reading processes…"
              : (ru ? "Найдено процессов: " : "Matching processes: ") +
                foundProcesses.length +
                " / " +
                processes.length}
          </p>
        </div>
        <div>
          <HelpLabel topic="connectionSearch">
            {ru ? "Фильтр соединений" : "Filter connections"}
            <input
              type="search"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder={
                ru
                  ? "IP, домен, EXE, PID, TCP/UDP, маршрут"
                  : "IP, domain, EXE, PID, TCP/UDP, route"
              }
            />
          </HelpLabel>
          <p className="small muted">
            {ru
              ? "Фильтрует собранные строки сразу, включая историю после остановки наблюдения."
              : "Filters collected rows immediately, including history after observation stops."}
          </p>
        </div>
      </div>
      {chosen && <p className="selectable">{chosen.path}</p>}
      <div className="row playground-controls">
        <button
          className={running ? "" : "primary"}
          disabled={!visible || (!!selected && !chosen)}
          onClick={() => {
            setRunning((v) => !v);
            setError("");
          }}
        >
          {running ? <Square size={15} /> : <Play size={15} />}
          {running
            ? ru
              ? "Остановить"
              : "Stop"
            : ru
              ? "Наблюдать"
              : "Observe"}
        </button>
        <button
          disabled={!visible || refreshing || (!!selected && !chosen)}
          onClick={() => {
            setRunning(true);
            setRefreshTick((v) => v + 1);
          }}
        >
          <RefreshCw size={15} />
          {ru ? "Обновить" : "Refresh"}
        </button>
        <HelpLabel topic="autoRefresh" className="check-label">
          <input
            type="checkbox"
            checked={refreshRate > 0}
            onChange={(e) => setRefreshRate(e.target.checked ? 5000 : 0)}
          />
          {ru ? "Автообновление" : "Auto refresh"}
        </HelpLabel>
        <HelpLabel
          topic="refreshRate"
          className="inline-setting playground-interval"
        >
          {ru ? "Интервал" : "Interval"}
          <select
            disabled={refreshRate === 0}
            value={refreshRate || 5000}
            onChange={(e) => setRefreshRate(Number(e.target.value))}
          >
            {[1000, 5000, 10000].map((ms) => (
              <option key={ms} value={ms}>
                {ms / 1000} {ru ? "сек." : "sec"}
              </option>
            ))}
          </select>
        </HelpLabel>
        <button
          onClick={() => {
            setHistory([]);
            setAnalysis(null);
          }}
        >
          {ru ? "Очистить" : "Clear"}
        </button>
        <span role="status">
          {running
            ? ru
              ? refreshing
                ? "Чтение…"
                : refreshRate
                  ? "Автообновление включено"
                  : "Ручное обновление"
              : refreshing
                ? "Reading…"
                : refreshRate
                  ? "Auto refresh enabled"
                  : "Manual refresh"
            : ru
              ? "Наблюдение остановлено"
              : "Observation stopped"}
        </span>
      </div>
      {error && <p role="alert">{error}</p>}
      {analysis && !analysis.process_alive && (
        <p>
          {ru
            ? "Процесс завершился. Выберите его заново после запуска."
            : "Process exited. Select it again after restarting."}
        </p>
      )}
      {analysis && !analysis.core_available && (
        <p>
          {ru
            ? "Данные ядра недоступны: доступны только таблицы Windows. Для трафика и маршрутов подключите VPN."
            : "Core metadata unavailable. Windows socket tables remain available; connect VPN for routes and traffic counters."}
        </p>
      )}
      <div className="table-scroll">
        <table>
          <thead>
            <tr>
              {[
                ru ? "Источник / приложение" : "Source / app",
                ru ? "Тип / состояние" : "Type / state",
                ru ? "Локальный адрес" : "Local address",
                ru ? "Назначение" : "Destination",
                ru ? "Маршрут" : "Route",
                "↑ / ↓",
              ].map((v) => (
                <th key={v}>{v}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {shownConnections.map((v) => (
              <tr
                key={`${v.source}:${v.id}`}
                className={v.active ? "" : "muted"}
              >
                <td>
                  {v.source} · {v.process.split(/[\\/]/).pop() || "—"}
                </td>
                <td>
                  {v.network} ·{" "}
                  {v.active
                    ? v.state
                    : ru
                      ? "завершено / не наблюдается"
                      : "closed / unobserved"}
                </td>
                <td className="selectable">{v.local}</td>
                <td className="selectable">{v.destination || "—"}</td>
                <td>{v.route || "—"}</td>
                <td>
                  {v.upload ?? "—"} / {v.download ?? "—"} B
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {history.length > 0 && (
        <p className="small muted" role="status">
          {ru ? "Показано соединений: " : "Connections shown: "}
          {shownConnections.length} / {history.length}
        </p>
      )}
      {history.length > 0 && shownConnections.length === 0 && (
        <p>
          {ru
            ? "Нет соединений по этому запросу. Измените или очистите фильтр."
            : "No connections match. Change or clear the filter."}
        </p>
      )}
      {history.length === 0 && (
        <p>
          {ru
            ? "Соединений пока нет. Запустите наблюдение и выполните действие в выбранном приложении."
            : "No connections yet. Start observing and use the selected app."}
        </p>
      )}
      <details>
        <summary>
          {ru
            ? "Последние предупреждения ядра (могут относиться к другим приложениям)"
            : "Recent core warnings (may concern other apps)"}
        </summary>
        <pre className="selectable log-output">
          {analysis?.errors.join("\n") || "—"}
        </pre>
      </details>
      <p>
        {ru
          ? "До 300 записей в памяти; без базы данных и фонового сканирования после остановки. Наблюдение прекращается при уходе из вкладки. Данные пакетов и HTTPS-содержимое не читаются. Windows не предоставляет здесь трафик, адрес назначения UDP и внутренние ошибки сторонней игры; отсутствие записи не означает блокировку."
          : "Up to 300 in-memory entries; no database or background polling after stopping or leaving this tab. Does not inspect payloads or HTTPS contents. Windows rows do not provide byte counters, UDP destinations or internal game errors; a missing row does not mean blocking."}
      </p>
    </section>
  );
}
