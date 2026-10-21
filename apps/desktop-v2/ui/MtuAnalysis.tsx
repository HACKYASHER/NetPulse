import { HelpLabel, Help } from "./Help";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity, Square } from "lucide-react";
import type { NativeError } from "./types";
type Report = {
  adapters: {
    name: string;
    kind: string;
    up: boolean;
    mtu: number;
    addresses: string[];
    selected: boolean;
  }[];
  target: string;
  path: {
    confirmed_mtu: number | null;
    largest_confirmed: number | null;
    probes: number;
    note: string;
  };
  suggested_mtu: number | null;
  recommendation_source: string;
  base_mtu: number | null;
  reserve: number;
  tunnel_active: boolean;
  tunnel_https_ok: boolean | null;
  tunnel_note: string;
};
type Job = {
  id: string;
  kind: string;
  running: boolean;
  cancelled: boolean;
  error: NativeError | null;
  results: Report[];
};
export function MtuAnalysis(p: {
  lang: string;
  busy: boolean;
  apply: (mtu: number) => void;
  error: (e: NativeError) => void;
}) {
  const ru = p.lang === "ru";
  const [target, setTarget] = useState("1.1.1.1"),
    [job, setJob] = useState<Job | null>(null),
    [starting, setStarting] = useState(false);
  useEffect(() => {
    invoke<Job | null>("current_diagnostic")
      .then((j) => {
        if (j?.kind === "mtu") setJob(j);
      })
      .catch(p.error);
  }, []);
  useEffect(() => {
    if (!job?.running) return;
    let disposed = false;
    let failures = 0;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const j = await invoke<Job>("diagnostic_status", { id: job.id });
        if (disposed) return;
        setJob(j);
        if (j.running) timer = setTimeout(poll, 700);
        else if (j.error && !j.cancelled) p.error(j.error);
      } catch (e) {
        if (disposed) return;
        if (++failures < 3) timer = setTimeout(poll, 1200);
        else {
          setJob(null);
          p.error(e as NativeError);
        }
      }
    };
    void poll();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [job?.id, job?.running]);
  const [showUnused, setShowUnused] = useState(false);
  const report = job?.results[0];
  const used = (a: Report["adapters"][number]) =>
    a.up &&
    a.mtu > 0 &&
    a.addresses.some((ip) => ip !== "0.0.0.0" && !ip.startsWith("169.254."));

  return (
    <section className="card">
      <div className="section-top">
        <div>
          <h3>{ru ? "Подбор MTU" : "MTU analysis"}</h3>
          <p>
            {ru
              ? "Проверка пути IPv4 с повторным подтверждением границы MTU и отдельной проверкой доступности VPN."
              : "Experimental adapter, Don't Fragment path and VPN health analysis."}
          </p>
        </div>
      </div>
      <div className="data-actions">
        <HelpLabel topic="mtuTarget">
          {ru ? "Узел для IPv4 DF-пакетов" : "IPv4 DF target"}
          <input
            value={target}
            onChange={(e) => setTarget(e.target.value)}
            disabled={starting || !!job?.running}
          />
        </HelpLabel>
        <button
          disabled={p.busy || starting || !!job?.running}
          onClick={async () => {
            if (starting || job?.running) return;
            setStarting(true);
            try {
              const id = await invoke<string>("start_mtu", { target });
              setJob({
                id,
                kind: "mtu",
                running: true,
                cancelled: false,
                error: null,
                results: [],
              });
            } catch (e) {
              p.error(e as NativeError);
            } finally {
              setStarting(false);
            }
          }}
        >
          <Activity size={16} />
          {job?.running
            ? ru
              ? "Анализ…"
              : "Analyzing…"
            : ru
              ? "Проверить MTU"
              : "Analyze MTU"}
        </button>
        {job?.running && (
          <button
            onClick={() =>
              invoke("cancel_diagnostic", { id: job.id }).catch(p.error)
            }
          >
            <Square size={15} />
            {ru ? "Отменить" : "Cancel"}
          </button>
        )}
      </div>
      <p className="small muted">
        {ru
          ? "Запросы отправляются указанному узлу; HTTPS проверка идёт к тестовому URL из настроек. Адаптеры и маршруты Windows не изменяются. Результат не гарантирует максимальную скорость."
          : "Probes contact the target and configured HTTPS probe URL. Windows adapters and routes are not changed. No maximum-speed guarantee."}
      </p>
      {job?.cancelled && (
        <p>{ru ? "Проверка отменена" : "Analysis cancelled"}</p>
      )}
      {report && (
        <div className="mtu-result">
          <button onClick={() => setShowUnused((v) => !v)}>
            {showUnused
              ? ru
                ? "Скрыть неиспользуемые адаптеры"
                : "Hide unused adapters"
              : ru
                ? "Показать неиспользуемые адаптеры"
                : "Show unused adapters"}
          </button>
          <table>
            <thead>
              <tr>
                <th>{ru ? "Адаптер" : "Adapter"}</th>
                <th>{ru ? "Тип" : "Type"}</th>
                <th>MTU</th>
                <th>IPv4</th>
              </tr>
            </thead>
            <tbody>
              {report.adapters
                .filter((a) => showUnused || used(a))
                .map((a) => (
                  <tr key={a.name}>
                    <td>
                      {a.selected ? "✓ " : ""}
                      {a.name}
                      {!a.up ? " · off" : ""}
                    </td>
                    <td>{a.kind}</td>
                    <td>{a.mtu || "—"}</td>
                    <td className="selectable">
                      {a.addresses.join(", ") || "—"}
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
          <p>
            {ru ? "MTU пути" : "Path MTU"}:{" "}
            <strong>
              {report.path.confirmed_mtu ??
                (ru ? "Не определён" : "Inconclusive")}
            </strong>{" "}
            · {report.path.probes} {ru ? "проб" : "probes"}
          </p>
          <p className="small muted">{report.path.note}</p>
          <p>
            VPN HTTPS:{" "}
            {report.tunnel_https_ok === null
              ? "—"
              : report.tunnel_https_ok
                ? ru
                  ? "Отвечает"
                  : "Responding"
                : ru
                  ? "Нет ответа"
                  : "No response"}
          </p>
          <p className="small muted">{report.tunnel_note}</p>
          {report.suggested_mtu && (
            <>
              <p>
                {ru ? "Значение-кандидат для TUN" : "Candidate TUN value"}:{" "}
                <strong>{report.suggested_mtu}</strong>.{" "}
                {ru
                  ? "Расчётная рекомендация: " +
                    (report.recommendation_source === "measured_path"
                      ? "измеренный MTU пути"
                      : "MTU активного физического адаптера; ICMP-измерение недоступно") +
                    " (" +
                    report.base_mtu +
                    "), минус " +
                    report.reserve +
                    " байт, с ограничением 1280–1420. Это начальное значение для проверки: накладные расходы зависят от протокола и сервера. Для измерения внешнего пути отключите TUN и укажите IPv4 VPN-сервера."
                  : "Estimate with an 80-byte reserve, not a measured optimum. Disconnect TUN to measure the outer DF path."}
              </p>
              <button
                disabled={p.busy || !!job?.running}
                onClick={() => p.apply(report.suggested_mtu!)}
              >
                {ru ? "Внести MTU в настройки" : "Use in settings"}
              </button>
            </>
          )}
        </div>
      )}
    </section>
  );
}
