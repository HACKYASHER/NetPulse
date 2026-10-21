import { HelpLabel, Help } from "./Help";
import { invoke } from "@tauri-apps/api/core";
import type { Preferences } from "./types";
export function AdvancedRules({
  draft: p,
  change,
  lang,
}: {
  draft: Preferences;
  change: (p: Preferences) => void;
  lang: string;
}) {
  const ru = lang === "ru";
  const rule = (i: number, v: Partial<Preferences["routing_rules"][number]>) =>
    change({
      ...p,
      routing_rules: p.routing_rules.map((r, n) =>
        n === i ? { ...r, ...v } : r,
      ),
    });
  return (
    <section className="card">
      <div className="section-top">
        <h3>{ru ? "Правила назначения" : "Destination rules"}</h3>
        <button
          disabled={p.routing_rules.length >= 64}
          onClick={() =>
            change({
              ...p,
              routing_rules: [
                ...p.routing_rules,
                { enabled: true, kind: "domain", value: "", action: "direct" },
              ],
            })
          }
        >
          {ru ? "Добавить правило" : "Add rule"}
        </button>
      </div>
      <p>
        {ru
          ? "Порядок: перехват DNS → доменные исключения из маршрутизации → правила ниже сверху вниз → общий список приложений. Совпадение первого правила определяет маршрут. IP/CIDR и GeoIP проверяют адрес назначения после разрешения имени."
          : "Order: DNS interception → routing domain exceptions → rules below, top to bottom → app list defaults. First matching rule wins. IP/CIDR and GeoIP use the resolved destination address."}
      </p>
      {p.routing_rules.map((r, i) => (
        <div className="route-rule" key={i}>
          <span className="control-with-help">
            <input
              type="checkbox"
              aria-label={ru ? "Правило включено" : "Rule enabled"}
              checked={r.enabled}
              onChange={(e) => rule(i, { enabled: e.target.checked })}
            />
            <Help topic="ruleEnabled" />
          </span>
          <span className="control-with-help">
            <select
              aria-label={ru ? "Тип правила" : "Rule type"}
              value={r.kind}
              onChange={(e) =>
                rule(i, { kind: e.target.value as typeof r.kind })
              }
            >
              {["domain", "cidr", "geoip", "geosite", "process"].map((v) => (
                <option key={v}>{v}</option>
              ))}
            </select>
            <Help topic="ruleKind" />
          </span>
          <span className="control-with-help">
            <input
              aria-label={ru ? "Значение правила" : "Rule value"}
              value={r.value}
              placeholder={
                r.kind === "domain"
                  ? "example.org"
                  : r.kind === "cidr"
                    ? "192.0.2.0/24"
                    : r.kind === "geoip"
                      ? "de"
                      : r.kind === "geosite"
                        ? "category-games"
                        : "C:\\Apps\\Game.exe"
              }
              onChange={(e) => rule(i, { value: e.target.value })}
            />
            <Help topic="ruleValue" />
          </span>
          <span className="control-with-help">
            <select
              aria-label={ru ? "Маршрут" : "Route"}
              value={r.action}
              onChange={(e) =>
                rule(i, { action: e.target.value as typeof r.action })
              }
            >
              <option value="proxy">VPN</option>
              <option value="direct">{ru ? "Напрямую" : "Direct"}</option>
              <option value="reject">{ru ? "Блокировать" : "Block"}</option>
            </select>
            <Help topic="ruleAction" />
          </span>
          <button
            disabled={i === 0}
            aria-label={ru ? "Выше" : "Move up"}
            onClick={() => {
              const rows = [...p.routing_rules];
              [rows[i - 1], rows[i]] = [rows[i], rows[i - 1]];
              change({ ...p, routing_rules: rows });
            }}
          >
            ↑
          </button>
          <button
            aria-label={ru ? "Удалить правило" : "Delete rule"}
            onClick={() =>
              change({
                ...p,
                routing_rules: p.routing_rules.filter((_, n) => n !== i),
              })
            }
          >
            ×
          </button>
        </div>
      ))}
      <p>
        {ru
          ? "GeoIP: код страны из двух букв. Geosite: имя набора SagerNet. Загружаются только выбранные наборы, через GitHub по HTTPS; ядро обновляет их раз в сутки во время работы. GeoIP — классификация адресов, точность страны и доступность набора не гарантируются. До 64 правил и 8 внешних наборов."
          : "GeoIP uses two-letter country codes; Geosite uses SagerNet set names. Only selected sets are fetched via GitHub HTTPS; the core refreshes them daily while running. Country classification and set availability are not guaranteed. Limit: 64 rules, 8 external sets."}
      </p>
      <div className="row">
        <button
          onClick={() =>
            void invoke("open_core_page", {
              url: "https://github.com/SagerNet/sing-geoip",
            }).catch(() => {})
          }
        >
          GitHub · GeoIP
        </button>
        <button
          onClick={() =>
            void invoke("open_core_page", {
              url: "https://github.com/SagerNet/sing-geosite",
            }).catch(() => {})
          }
        >
          GitHub · Geosite
        </button>
      </div>
      <HelpLabel topic="sniff">
        {ru
          ? "Не разбирать HTTP/TLS/QUIC этих EXE (по одному пути)"
          : "Skip HTTP/TLS/QUIC sniffing for these EXE paths (one per line)"}
        <textarea
          spellCheck={false}
          value={p.sniff_excluded_apps.join("\n")}
          onChange={(e) =>
            change({
              ...p,
              sniff_excluded_apps: e.target.value.split("\n").filter(Boolean),
            })
          }
          placeholder="C:\\Apps\\RobloxPlayerBeta.exe"
        />
      </HelpLabel>
      <p>
        {ru
          ? "Маршрут по процессу работает в TUN. Исключение сниффинга не меняет TLS игры и не отключает проверку сертификатов. Доменные правила для таких приложений зависят от DNS-сопоставления. Правила входят в импорт и экспорт маршрутизации."
          : "Process routing works in TUN. Sniff exclusions do not alter app TLS or certificate checks; domain rules then depend on DNS mapping. These rules are included in routing import/export."}
      </p>
    </section>
  );
}
