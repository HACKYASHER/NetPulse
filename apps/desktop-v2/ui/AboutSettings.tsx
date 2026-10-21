import { HelpLabel, Help } from "./Help";
import { useEffect, useState } from "react";
import { Download, ExternalLink, FolderOpen, RefreshCw } from "lucide-react";
import type { Preferences, ClientRelease } from "./types";
export function AboutSettings(p: {
  draft: Preferences;
  change: <K extends keyof Preferences>(key: K, value: Preferences[K]) => void;
  lang: string;
  version: string;
  busy: boolean;
  dirty: boolean;
  release: ClientRelease | null;
  receiveRelease: (release: ClientRelease | null) => void;
  action: <T>(
    command: string,
    args?: Record<string, unknown>,
    refresh?: boolean,
  ) => Promise<T | undefined>;
}) {
  const ru = p.lang === "ru";
  const release = p.release;
  const [downloaded, setDownloaded] = useState(false);
  const [notice, setNotice] = useState("");
  useEffect(() => setDownloaded(false), [release?.tag, release?.sha256]);
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(""), 6000);
    return () => clearTimeout(timer);
  }, [notice]);
  return (
    <>
      <section className="card">
        <h3>
          {ru ? "О NetPulse" : "About NetPulse"} · {p.version}
        </h3>
        <p>
          {ru
            ? "Windows-клиент для ваших серверов. Разработано HACKYASHER совместно с OpenAI Codex."
            : "A Windows client for your servers. Developed by HACKYASHER with OpenAI Codex."}
        </p>
        <button
          onClick={() =>
            p.action(
              "open_core_page",
              { url: "https://github.com/HACKYASHER/NetPulse" },
              false,
            )
          }
        >
          <ExternalLink size={16} />
          GitHub · HACKYASHER/NetPulse
        </button>
        <h4>{ru ? "Поведение клиента" : "Client behavior"}</h4>
        {(
          [
            [
              "autostart",
              "Запускать при входе в Windows",
              "Start with Windows",
            ],
            ["close_to_tray", "Закрывать окно в трей", "Close to tray"],
            ["minimize_to_tray", "Сворачивать в трей", "Minimize to tray"],
            [
              "auto_reconnect",
              "Подключаться при запуске и восстанавливать соединение",
              "Connect on startup and reconnect",
            ],
          ] as const
        ).map(([key, r, en]) => (
          <HelpLabel topic={key} className="check-label" key={key}>
            <input
              type="checkbox"
              checked={p.draft[key]}
              onChange={(e) => p.change(key, e.target.checked)}
            />
            {ru ? r : en}
          </HelpLabel>
        ))}
        <p className="small muted">
          {ru
            ? "Сворачивание в трей выключено по умолчанию при новой установке. Уже выбранные настройки сохраняются."
            : "Minimize to tray is off by default for new installations. Existing preferences are preserved."}
        </p>
      </section>
      <section className="card">
        <h3>{ru ? "Обновление клиента" : "Client update"}</h3>
        <p>
          {ru
            ? "Клиент проверяет официальные релизы GitHub после запуска и каждые 6 часов, включая предварительные. При выходе новой версии появляется уведомление. Загрузка начинается по вашей команде; проверяются источник и SHA-256. Установка завершит клиент и отключит VPN."
            : "Official GitHub releases are checked after startup and every 6 hours, including prereleases. You are notified when an update is available. Downloads are manual, with source and SHA-256 verification. Installation disconnects VPN and exits the client."}
        </p>
        <button
          disabled={p.busy}
          onClick={async () => {
            const result = await p.action<ClientRelease | null>(
              "check_client_update",
              { force: true },
              false,
            );
            if (result !== undefined) {
              p.receiveRelease(result);
              setDownloaded(false);
              setNotice(
                result
                  ? ru
                    ? "Найдена новая версия"
                    : "Update available"
                  : ru
                    ? "Установлена актуальная версия"
                    : "You are up to date",
              );
            }
          }}
        >
          <RefreshCw size={16} />
          {ru ? "Проверить обновления" : "Check for updates"}
        </button>
        {notice && <p role="status">{notice}</p>}
        {release && (
          <div className="core-source">
            <strong>
              {release.tag}
              {release.prerelease ? " · pre-release" : ""}
            </strong>
            <button
              className="link-button"
              onClick={() =>
                p.action("open_core_page", { url: release.page }, false)
              }
            >
              <ExternalLink size={14} />
              {ru ? "Страница релиза" : "Release page"}
            </button>
            <p className="selectable small">{release.url}</p>
            <small className="selectable">SHA-256: {release.sha256}</small>
            <button
              disabled={p.busy}
              onClick={async () => {
                if (
                  await p.action<boolean>(
                    "download_client_update",
                    { release },
                    false,
                  )
                ) {
                  setDownloaded(true);
                  setNotice(
                    ru
                      ? "Установщик загружен и проверен"
                      : "Installer downloaded and verified",
                  );
                }
              }}
            >
              <Download size={16} />
              {ru ? "Скачать установщик" : "Download installer"} ·{" "}
              {(release.size / 1048576).toFixed(1)} MB
            </button>
            {downloaded && (
              <button
                className="primary"
                disabled={p.busy || p.dirty}
                onClick={() =>
                  p.action("install_client_update", { release }, false)
                }
              >
                {ru
                  ? "Отключить VPN и установить обновление"
                  : "Disconnect VPN and install update"}
              </button>
            )}
            {p.dirty && downloaded && (
              <small>
                {ru
                  ? "Сначала сохраните или отмените изменения настроек."
                  : "Save or discard settings changes first."}
              </small>
            )}
          </div>
        )}
      </section>
      <section className="card">
        <h3>{ru ? "Журнал действий" : "Action log"}</h3>
        <p>
          {ru
            ? "Действия интерфейса, команды, состояния и ошибки сохраняются по датам в logs/ГГГГ-ММ-ДД/. Основной журнал содержит все разделы; отдельно доступны ядро, туннель, подписки и служба. Секреты и содержимое полей исключаются. Общий лимит — 128 МиБ. В «Активности» можно скопировать записи или очистить все разделы за все даты."
            : "Actions, states and errors are written to dated logs/YYYY-MM-DD/ folders. Main includes all categories; core, tunnel, subscriptions and service are separate. Secrets and field contents are excluded. Archive limit: 128 MiB. Activity supports copying and clearing all categories and dates."}
        </p>
        <button
          disabled={p.busy}
          onClick={() => p.action("open_log_folder", {}, false)}
        >
          <FolderOpen size={16} />
          {ru ? "Открыть папку журнала" : "Open log folder"}
        </button>
      </section>
    </>
  );
}
