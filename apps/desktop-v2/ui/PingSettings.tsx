import { HelpLabel, Help } from "./Help";
import type { Preferences } from "./types";
export const pingMethods = [
  ["proxy_https_rtt", "via Proxy GET"],
  ["proxy_https_head_rtt", "via Proxy HEAD"],
  ["tcp_connect_rtt", "TCP"],
  ["icmp_rtt", "ICMP"],
] as const;
export function PingSettings(p: {
  draft: Preferences;
  change: <K extends keyof Preferences>(key: K, value: Preferences[K]) => void;
  lang: string;
}) {
  const ru = p.lang === "ru";
  return (
    <section className="card ping-settings">
      <h3>{ru ? "Пинг" : "Ping"}</h3>
      <p>{ru ? "Тип проверки" : "Test method"}</p>
      <div className="ping-methods">
        {pingMethods.map(([value, label]) => (
          <HelpLabel topic={value} className="check-label" key={value}>
            <input
              type="radio"
              name="ping-method"
              value={value}
              checked={p.draft.ping_method === value}
              onChange={() => p.change("ping_method", value)}
            />
            {label}
          </HelpLabel>
        ))}
      </div>
      <div className="form-grid">
        <HelpLabel topic="pingMode">
          {ru ? "Режим (via Proxy)" : "Proxy mode"}
          <select
            value={p.draft.ping_proxy_mode}
            onChange={(e) =>
              p.change(
                "ping_proxy_mode",
                e.target.value as Preferences["ping_proxy_mode"],
              )
            }
          >
            <option value="double">double</option>
            <option value="keepalive">keepalive</option>
          </select>
        </HelpLabel>
        <HelpLabel topic="probe">
          {ru ? "Тестовый URL (via Proxy)" : "Test URL (via Proxy)"}
          <input
            type="url"
            value={p.draft.probe_url}
            onChange={(e) => p.change("probe_url", e.target.value)}
          />
        </HelpLabel>
      </div>
      <p className="small">
        {ru
          ? "Результат — лучший из двух замеров. double: два отдельных HTTPS-соединения через один профиль. keepalive: повторный запрос использует установленное соединение, если сервер его сохраняет."
          : "Displays the best of two measurements. double uses separate HTTPS connections; keepalive reuses a connection when supported."}
      </p>
      <p className="small muted">
        {ru
          ? "TCP измеряет соединение до сервера вне TUN NetPulse. ICMP использует текущие маршруты Windows. Эти методы не проверяют VPN; HY2 использует UDP и может не отвечать на TCP. GET/HEAD проверяют реальный запрос через профиль, TLS и авторизацию."
          : "TCP connects outside NetPulse TUN. ICMP uses current Windows routing. Neither verifies VPN; HY2 uses UDP and may not answer TCP. GET/HEAD traverse the profile with TLS and authentication."}
      </p>
    </section>
  );
}
