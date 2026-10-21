import { HelpLabel, Help } from "./Help";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { NativeError, Subscription } from "./types";
export function SubscriptionSettings(p: {
  subscription: Subscription;
  lang: string;
  close: () => void;
  refresh: () => Promise<unknown>;
  error: (e: NativeError) => void;
}) {
  const ru = p.lang === "ru",
    s = p.subscription;
  const [name, setName] = useState(s.name),
    [interval, setInterval] = useState(s.interval_hours);
  const [protectedUrl, setProtected] = useState(s.url_protected),
    [saving, setSaving] = useState(false);
  const [configLocked, setConfigLocked] = useState(s.config_locked);
  const [error, setError] = useState("");
  useEffect(() => {
    const close = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !saving) p.close();
    };
    document.addEventListener("keydown", close);
    return () => document.removeEventListener("keydown", close);
  }, [saving, p.close]);
  return (
    <div
      className="overlay"
      onMouseDown={(e) => {
        if (e.currentTarget === e.target && !saving) p.close();
      }}
    >
      <form
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="subscription-settings-title"
        onSubmit={async (e) => {
          e.preventDefault();
          if (saving) return;
          setSaving(true);
          setError("");
          try {
            await invoke("save_subscription", {
              id: s.id,
              name: name.trim(),
              intervalHours: interval,
              protected: protectedUrl,
              lockConfiguration: configLocked,
            });
            await p.refresh();
            p.close();
          } catch (e) {
            setError(String((e as NativeError)?.message || e));
          } finally {
            setSaving(false);
          }
        }}
      >
        <h2 id="subscription-settings-title">
          {ru ? "Настройки подписки" : "Subscription settings"}
        </h2>
        <HelpLabel topic="name">
          {ru ? "Название" : "Name"}
          <input
            autoFocus
            required
            maxLength={256}
            value={name}
            onChange={(e) => setName(e.target.value)}
            disabled={saving}
          />
        </HelpLabel>
        <HelpLabel topic="subInterval">
          {ru ? "Автообновление, часов" : "Refresh interval, hours"}
          <input
            required
            type="number"
            min={1}
            max={168}
            step={1}
            value={interval}
            onChange={(e) => setInterval(Number(e.target.value))}
            disabled={saving}
          />
        </HelpLabel>
        <HelpLabel topic="protect" className="check-label">
          <input
            type="checkbox"
            checked={protectedUrl}
            onChange={(e) => setProtected(e.target.checked)}
            disabled={saving}
          />
          {ru ? "Запретить копирование ссылки" : "Protect link from copying"}
        </HelpLabel>
        <p>
          {ru
            ? "Ссылка всегда хранится зашифрованной средствами Windows. Снятая галочка разрешает копировать её в буфер обмена. Изменение названия и интервала сохраняет серверы и активное подключение."
            : "Windows always encrypts the stored link. Uncheck to allow clipboard copying. Renaming or changing the interval preserves servers and the active connection."}
        </p>
        <HelpLabel topic="lock" className="check-label">
          <input
            type="checkbox"
            checked={configLocked}
            disabled={saving || s.config_locked}
            onChange={(e) => setConfigLocked(e.target.checked)}
          />
          {ru
            ? "Навсегда запретить редактирование конфигураций серверов"
            : "Permanently disable server configuration editing"}
        </HelpLabel>
        <p>
          {ru
            ? "После сохранения запрет нельзя снять в этом клиенте, включая восстановление старой резервной копии. Обновления от провайдера продолжают работать. Общие сетевые настройки остаются доступны."
            : "Once saved, this restriction cannot be removed in this client, including by restoring an older backup. Provider updates continue. Global network settings remain available."}
        </p>
        {error && (
          <p role="alert" className="selectable">
            {error}
          </p>
        )}
        <div className="data-actions">
          <button className="primary" disabled={saving || !name.trim()}>
            {saving
              ? ru
                ? "Сохранение…"
                : "Saving…"
              : ru
                ? "Сохранить"
                : "Save"}
          </button>
          <button type="button" disabled={saving} onClick={p.close}>
            {ru ? "Отмена" : "Cancel"}
          </button>
        </div>
      </form>
    </div>
  );
}
