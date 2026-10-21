import { HelpLabel, Help } from "./Help";
import type { Preferences } from "./types";
import { networkPreset } from "./networkPresets";
export function NetworkSettings({
  draft: p,
  change,
  lang,
}: {
  draft: Preferences;
  change: (p: Preferences) => void;
  lang: string;
}) {
  const ru = lang === "ru";
  const field = (patch: Partial<Preferences>) =>
    change({ ...p, ...patch, network_preset: "custom" });
  return (
    <section className="card">
      <h3>{ru ? "MTU и пресеты" : "MTU and presets"}</h3>
      <div className="form-grid">
        <HelpLabel topic="preset">
          {ru ? "Пресет" : "Preset"}
          <select
            value={p.network_preset}
            onChange={(e) => {
              const v = e.target.value as Preferences["network_preset"];
              change(networkPreset(p, v));
            }}
          >
            <option value="balanced">
              {ru ? "Сбалансированный" : "Balanced"}
            </option>
            <option value="compatibility">
              {ru ? "Совместимость" : "Compatibility"}
            </option>
            <option value="games">{ru ? "Игры — TUN" : "Games — TUN"}</option>
            <option value="custom">{ru ? "Свой" : "Custom"}</option>
          </select>
        </HelpLabel>
        <HelpLabel topic="mtu">
          MTU (1280–9000)
          <input
            type="number"
            min={1280}
            max={9000}
            value={p.mtu}
            onChange={(e) => field({ mtu: Number(e.target.value) })}
          />
        </HelpLabel>
      </div>
      {p.network_preset === "games" && (
        <>
          <p>
            {ru
              ? "После сохранения включается TUN и защищённый DNS Cloudflare (1.1.1.1) через VPN. System Proxy, MTU и правила маршрутизации сохраняют ваш выбор. В разделе «Маршрутизация» можно выбрать приложения, которые пойдут через VPN. Сервер должен поддерживать UDP."
              : "After saving, enable TUN and Cloudflare encrypted DNS (1.1.1.1) through VPN. Keep the selected System Proxy mode, MTU and routing rules. Choose apps to tunnel in Routing. The server must support UDP."}
          </p>
          <p>
            {ru
              ? "После применения заново запустите игру и её загрузчик: старые соединения и DNS-кеш могли сохраниться. Это настройка совместимости, а не гарантия исправления ошибок сервера или игры."
              : "Restart the game and launcher after applying; old connections and DNS caches may persist. This preset cannot guarantee resolution of server or game errors."}
          </p>
        </>
      )}
      <p>
        {ru
          ? "MTU применяется только к адаптеру NetPulse. Сбалансированный: MTU 1500, DoH через VPN. Совместимость: MTU 1400, DoH напрямую. Системный DNS и начальное разрешение имени DNS-сервера используют Windows."
          : "MTU applies only to NetPulse. Balanced: 1500, DoH through VPN. Compatibility: 1400, direct DoH. System DNS and DNS-server hostname bootstrap use Windows."}
      </p>
    </section>
  );
}

export function DnsSettings({
  draft: p,
  change,
  lang,
}: {
  draft: Preferences;
  change: (p: Preferences) => void;
  lang: string;
}) {
  const ru = lang === "ru";
  const field = (patch: Partial<Preferences>) =>
    change({ ...p, ...patch, network_preset: "custom" });
  return (
    <section className="card">
      <h3>DNS</h3>
      <div className="form-grid">
        <HelpLabel topic="dns">
          DNS
          <select
            value={p.dns.mode}
            onChange={(e) =>
              field({
                dns:
                  e.target.value === "system"
                    ? { mode: "system" }
                    : e.target.value === "https"
                      ? { mode: "https", server: "1.1.1.1", path: "/dns-query" }
                      : {
                          mode: e.target.value as "udp" | "tls",
                          server: "1.1.1.1",
                        },
                ...(e.target.value === "system"
                  ? { dns_route: "direct" as const }
                  : {}),
              })
            }
          >
            <option value="system">{ru ? "Системный" : "System"}</option>
            <option value="udp">UDP</option>
            <option value="https">DNS over HTTPS</option>
            <option value="tls">DNS over TLS</option>
          </select>
        </HelpLabel>
        <HelpLabel topic="dnsRoute">
          {ru ? "Маршрут DNS" : "DNS route"}
          <select
            value={p.dns_route}
            disabled={p.dns.mode === "system"}
            onChange={(e) =>
              field({ dns_route: e.target.value as "vpn" | "direct" })
            }
          >
            <option value="vpn">{ru ? "Через VPN" : "Through VPN"}</option>
            <option value="direct">{ru ? "Напрямую" : "Direct"}</option>
          </select>
        </HelpLabel>
        {p.dns.mode !== "system" && (
          <HelpLabel topic="dnsServer">
            {ru ? "Адрес DNS" : "DNS server"}
            <input
              value={p.dns.server}
              onChange={(e) =>
                field({
                  dns: {
                    ...p.dns,
                    server: e.target.value,
                  } as Preferences["dns"],
                })
              }
            />
          </HelpLabel>
        )}
        {p.dns.mode === "https" && (
          <HelpLabel topic="dnsPath">
            Path
            <input
              value={p.dns.path}
              onChange={(e) =>
                field({
                  dns: { ...p.dns, path: e.target.value } as Preferences["dns"],
                })
              }
            />
          </HelpLabel>
        )}
      </div>{" "}
      <HelpLabel topic="fake" className="check-label">
        <input
          type="checkbox"
          checked={p.fake_ip}
          onChange={(e) => change({ ...p, fake_ip: e.target.checked })}
        />
        Fake IP (TUN)
      </HelpLabel>
      <p>
        {ru
          ? "Включается только вместе с TUN. DNS возвращает адрес из 198.18.0.0/15 или fc00::/18; ядро хранит соответствие имени в памяти. Для доменных исключений сохраняются реальные ответы. При переключении Fake IP переподключите приложения: у них мог остаться DNS-кеш. Собственный DoH приложения и DNS вне захвата не перехватываются."
          : "Applies only with TUN. DNS maps names to 198.18.0.0/15 or fc00::/18 in memory; bypass domains keep real responses. Restart app connections after changing Fake IP to clear cached addresses. App-private DoH or DNS outside capture is not intercepted."}
      </p>
      <p>
        {ru
          ? "DNS TCP/UDP 53 уже направляется во встроенный DNS. Для зашифрованного upstream выберите DNS over HTTPS или TLS в этом разделе; системный DNS не является зашифрованным DNS ядра."
          : "TCP/UDP DNS on port 53 is redirected to the internal resolver. Choose DNS over HTTPS or TLS in this section for encrypted upstream; system DNS does not provide that guarantee."}
      </p>
    </section>
  );
}
