import { HelpLabel, Help } from "./Help";
import { Download, Upload } from "lucide-react";
import type { Preferences } from "./types";
import { mergeRouting, routingDocument, type RoutingDocument } from "./routing";
export function RoutingSettings(p: {
  draft: Preferences;
  lang: string;
  busy: boolean;
  change: (next: Preferences) => void;
  action: <T>(
    command: string,
    args?: Record<string, unknown>,
    refresh?: boolean,
  ) => Promise<T | undefined>;
}) {
  const ru = p.lang === "ru";
  return (
    <section className="card routing-custom">
      <div className="section-top">
        <div>
          <h3>{ru ? "Кастомные маршруты" : "Custom routes"}</h3>
          <p>
            {ru
              ? "Исключения идут напрямую во всех режимах, в том числе для приложений белого списка."
              : "Exceptions bypass VPN in every mode, including whitelisted applications."}
          </p>
        </div>
        <div className="data-actions">
          <button
            disabled={p.busy}
            onClick={async () => {
              const value = await p.action<RoutingDocument | null>(
                "import_routing",
                {},
                false,
              );
              if (value) p.change(mergeRouting(p.draft, value));
            }}
          >
            <Upload size={16} />
            {ru ? "Импорт" : "Import"}
          </button>
          <button
            disabled={p.busy}
            onClick={() =>
              p.action(
                "export_routing",
                { routing: routingDocument(p.draft) },
                false,
              )
            }
          >
            <Download size={16} />
            {ru ? "Экспорт" : "Export"}
          </button>
        </div>
      </div>
      <HelpLabel topic="bypass">
        {ru ? "Домены в обход VPN" : "Domains bypassing VPN"}
        <textarea
          spellCheck={false}
          placeholder={"example.com\n*.example.org"}
          value={p.draft.bypass_domains.join("\n")}
          onChange={(e) =>
            p.change({ ...p.draft, bypass_domains: e.target.value.split("\n") })
          }
        />
      </HelpLabel>
      <p className="small muted">
        {ru
          ? "По одному домену в строке, без https:// и путей. example.com включает его поддомены. До 1000 правил. Импорт заменяет режим, приложения, группы и исключения в черновике — проверьте их и нажмите «Сохранить»."
          : "One domain per line, without https:// or paths. example.com includes subdomains. Up to 1000 rules. Import replaces the routing draft; review it and Save."}
      </p>
      <p className="small muted">
        {ru
          ? "Для TUN домен определяется по DNS и открытым данным TLS/HTTP/QUIC. Если приложение скрывает имя через собственный DNS и ECH, доменное исключение может не сработать."
          : "TUN learns names from DNS and visible TLS/HTTP/QUIC metadata. Apps hiding names through private DNS and ECH may not match domain exceptions."}
      </p>
    </section>
  );
}
