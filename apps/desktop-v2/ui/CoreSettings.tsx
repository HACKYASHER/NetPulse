import { CoreSelector } from "./CoreSelector";
import { HelpLabel, Help } from "./Help";
import { useEffect, useState } from "react";
import { ExternalLink, Download, RefreshCw } from "lucide-react";
import type { Preferences } from "./types";
type Release = {
  engine: string;
  tag: string;
  prerelease: boolean;
  page: string;
  asset: string;
  url: string;
  archive_sha256: string;
};
export function CoreSettings(p: {
  busy: boolean;
  active: boolean;
  lang: string;
  transport: Preferences["transport_engine"];
  changeTransport: (value: Preferences["transport_engine"]) => void;
  singboxVersion: string;
  xrayVersion: string;
  action: <T>(
    command: string,
    args?: Record<string, unknown>,
    refresh?: boolean,
  ) => Promise<T | undefined>;
}) {
  const ru = p.lang === "ru";
  const [engine, setEngine] = useState("xray"),
    [releases, setReleases] = useState<Record<string, Release[]>>({}),
    [chosen, setChosen] = useState<Record<string, string>>({}),
    [done, setDone] = useState("");
  useEffect(() => {
    if (!done) return;
    const timer = setTimeout(() => setDone(""), 6000);
    return () => clearTimeout(timer);
  }, [done]);
  const items = releases[engine] || [];
  const selected = items.find((r) => r.tag === chosen[engine]) || items[0];
  const open = (url: string) => p.action("open_core_page", { url }, false);
  return (
    <section className="card core-settings">
      <h3>{ru ? "Ядра и обновления" : "Cores and updates"}</h3>
      <CoreSelector
        lang={p.lang}
        value={p.transport}
        disabled={p.busy}
        change={p.changeTransport}
      />
      <p className="small muted">
        {ru
          ? "Xray используется для поддерживаемых VLESS, Trojan, Shadowsocks, SOCKS и HTTP. Hysteria2 использует sing-box; TUN и правила приложений также обслуживает sing-box. Выбор применяется кнопкой «Сохранить»."
          : "Xray handles supported transports; Hysteria2 and TUN use sing-box. Save to apply your choice."}
      </p>
      <div className="data-actions collection-toolbar">
        <div className="segments">
          {["xray", "sing-box"].map((v) => (
            <button
              key={v}
              aria-pressed={engine === v}
              className={engine === v ? "selected" : ""}
              disabled={p.busy}
              onClick={() => setEngine(v)}
            >
              {v === "xray" ? "Xray" : "sing-box"} ·{" "}
              {v === "xray" ? p.xrayVersion : p.singboxVersion}
            </button>
          ))}
        </div>
        <button
          disabled={p.busy}
          onClick={async () => {
            const list = await p.action<Release[]>(
              "list_core_releases",
              { engine },
              false,
            );
            if (list) {
              setReleases((v) => ({ ...v, [engine]: list }));
              setChosen((v) => ({ ...v, [engine]: list[0]?.tag || "" }));
            }
          }}
        >
          <RefreshCw size={15} />
          {p.busy
            ? ru
              ? `Проверка / обновление ${engine}…`
              : `Checking / updating ${engine}…`
            : ru
              ? `Проверить ${engine}`
              : `Check ${engine}`}
        </button>
      </div>
      <div className="data-actions">
        <button
          className="link-button"
          onClick={() => open("https://github.com/XTLS/Xray-core")}
        >
          <ExternalLink size={14} />
          GitHub · XTLS/Xray-core
        </button>
        <button
          className="link-button"
          onClick={() => open("https://github.com/SagerNet/sing-box")}
        >
          <ExternalLink size={14} />
          GitHub · SagerNet/sing-box
        </button>
      </div>
      <p className="small muted">
        {ru
          ? "Показаны стабильные и предварительные релизы. Перед заменой проверяются официальный SHA-256 и конфигурации; предыдущий комплект сохраняется."
          : "Stable and prerelease versions are included. Official checksums and configurations are checked before replacement."}
      </p>
      {!!items.length && (
        <>
          <HelpLabel topic="release">
            {ru ? "Версия для установки" : "Version to install"}
            <select
              disabled={p.busy}
              value={selected?.tag || ""}
              onChange={(e) =>
                setChosen((v) => ({ ...v, [engine]: e.target.value }))
              }
            >
              {items.map((r) => (
                <option key={r.tag} value={r.tag}>
                  {r.tag}
                  {r.prerelease ? " · pre-release" : ""}
                </option>
              ))}
            </select>
          </HelpLabel>
          {selected && (
            <div className="core-source">
              <button
                className="link-button"
                onClick={() => open(selected.page)}
              >
                <ExternalLink size={14} />
                {ru ? "Страница выбранного релиза" : "Selected release page"}
              </button>
              <button
                className="link-button"
                onClick={() => open(selected.url)}
              >
                {selected.asset}
              </button>
              <small>
                SHA-256: <code>{selected.archive_sha256}</code>
              </small>
              <button
                className="primary"
                disabled={p.busy || p.active}
                onClick={async () => {
                  setDone("");
                  const result = await p.action<{
                    engine: string;
                    previous: string;
                    installed: string;
                    cache_clean: boolean;
                  }>("update_core", {
                    release: selected,
                  });
                  if (result) {
                    setDone(
                      `${result.engine}: ${result.previous} → ${result.installed}. ${ru ? "Версия файла и SHA-256 проверены." : "Binary version and SHA-256 verified."} ${result.cache_clean ? (ru ? "ZIP удалён." : "ZIP removed.") : ru ? "Не удалось очистить кеш ZIP." : "ZIP cache cleanup incomplete."} ${ru ? "Для TUN обновите службу в разделе «Сеть»." : "Update the TUN service in Network settings."}`,
                    );
                  }
                }}
              >
                <Download size={15} />
                {ru ? "Скачать и установить" : "Download and install"}
              </button>
            </div>
          )}
        </>
      )}
      {done && (
        <p role="status" className="success">
          {done}
        </p>
      )}
      <div className="data-actions">
        <button
          disabled={p.busy || p.active}
          onClick={() => p.action("activate_core_bundle", { rollback: true })}
        >
          {ru ? "Откатить комплект" : "Roll back bundle"}
        </button>
        <button
          disabled={p.busy || p.active}
          onClick={() => p.action("activate_core_bundle", { rollback: false })}
        >
          {ru ? "Восстановить встроенные ядра" : "Restore bundled cores"}
        </button>
      </div>
      <small>
        {ru
          ? "Обновление и откат доступны после отключения VPN. Защищённые копии службы обновляются отдельно с обычным UAC; официальность архива проверяется повторно."
          : "Disconnect before updates or rollback. Service copies require a separate UAC update with independent archive verification."}
      </small>
    </section>
  );
}
