import { HelpLabel, Help } from "./Help";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { NativeError, Profile } from "./types";
export function ProfileJson(p: {
  profile: Profile;
  lang: string;
  close: () => void;
  refresh: () => Promise<unknown>;
}) {
  const ru = p.lang === "ru";
  const [text, setText] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let alive = true;
    invoke<string>("profile_json", { id: p.profile.id })
      .then((v) => {
        if (alive) {
          setText(v);
          setLoaded(true);
        }
      })
      .catch((e) => alive && setError((e as NativeError).message));
    return () => {
      alive = false;
    };
  }, [p.profile.id]);
  return (
    <div className="overlay">
      <form
        className="modal json-editor"
        role="dialog"
        aria-modal="true"
        aria-labelledby="profile-json-title"
        onSubmit={async (e) => {
          e.preventDefault();
          if (!loaded || saving) return;
          setSaving(true);
          setError("");
          try {
            JSON.parse(text);
            await invoke("save_profile_json", { id: p.profile.id, json: text });
            await p.refresh();
            p.close();
          } catch (e) {
            setError((e as NativeError).message || String(e));
          } finally {
            setSaving(false);
          }
        }}
      >
        <h2 id="profile-json-title">JSON · {p.profile.name}</h2>
        <p>
          {ru
            ? "Конфигурация исходящего соединения NetPulse / sing-box. Содержит секреты ключа. Изменения сохраняются локально и переживают обновление неизменившегося ключа в подписке. Активный сервер будет переподключён после проверки."
            : "NetPulse / sing-box outbound configuration. Contains key secrets. Local edits survive refresh while the original provider key stays unchanged. An active server reconnects after validation."}
        </p>
        <HelpLabel topic="json">
          {ru ? "Конфигурация сервера" : "Server configuration"}
          <textarea
            className="selectable"
            spellCheck={false}
            value={text}
            disabled={!loaded || saving}
            maxLength={65536}
            onChange={(e) => setText(e.target.value)}
          />
        </HelpLabel>
        {error && (
          <p role="alert" className="selectable">
            {error}
          </p>
        )}
        <div className="data-actions">
          <button className="primary" disabled={!loaded || saving}>
            {saving
              ? ru
                ? "Проверка…"
                : "Validating…"
              : ru
                ? "Проверить и сохранить"
                : "Validate and save"}
          </button>
          <button type="button" disabled={saving} onClick={p.close}>
            {ru ? "Отмена" : "Cancel"}
          </button>
        </div>
      </form>
    </div>
  );
}
