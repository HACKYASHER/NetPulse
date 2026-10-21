import { HelpLabel, Help } from "./Help";
import React, { useEffect, useRef, useState } from "react";
import {
  AlertCircle,
  Clipboard,
  FileImage,
  FolderOpen,
  LoaderCircle,
  Plus,
  X,
} from "lucide-react";
import type { Label } from "./i18n";
import type { NativeError } from "./types";

export function ImportModal({
  t,
  busy,
  error,
  close,
  imported,
}: {
  t: (key: Label) => string;
  busy: boolean;
  error: NativeError | null;
  close: () => void;
  imported: (
    command: string,
    args?: Record<string, unknown>,
  ) => Promise<boolean>;
}) {
  const dialog = useRef<HTMLElement>(null);
  const [tab, setTab] = useState<"key" | "subscription" | "form" | "photo">(
    "key",
  );
  const [text, setText] = useState(""),
    [name, setName] = useState(""),
    [url, setUrl] = useState("");
  const [protocol, setProtocol] = useState("vless"),
    [host, setHost] = useState(""),
    [port, setPort] = useState(443);
  const [secret, setSecret] = useState(""),
    [username, setUsername] = useState(""),
    [sni, setSni] = useState("");
  const [cipher, setCipher] = useState("chacha20-ietf-poly1305");
  const [protectedUrl, setProtectedUrl] = useState(true);
  const proxy = ["socks5", "http", "https"].includes(protocol);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialog.current
      ?.querySelector<HTMLElement>("button,input,textarea")
      ?.focus();
    return () => previous?.focus();
  }, []);
  function keydown(e: React.KeyboardEvent) {
    if (e.key === "Escape" && !busy) close();
    if (e.key === "Tab") {
      const focusable = dialog.current?.querySelectorAll<HTMLElement>(
        "button:not(:disabled),input:not(:disabled),textarea:not(:disabled),select:not(:disabled)",
      );
      if (!focusable?.length) return;
      const first = focusable[0],
        last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }
  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (tab === "photo") return;
    if (tab === "subscription") {
      if (
        await imported("add_subscription", {
          name,
          url,
          protected: protectedUrl,
        })
      )
        setUrl("");
      return;
    }
    if (tab === "key") {
      if (await imported("import_text", { text })) setText("");
      return;
    }
    const address =
      host.includes(":") && !host.startsWith("[") ? `[${host}]` : host;
    const auth = proxy
      ? username
        ? `${encodeURIComponent(username)}:${encodeURIComponent(secret)}@`
        : ""
      : protocol === "ss"
        ? `${cipher}:${encodeURIComponent(secret)}@`
        : `${encodeURIComponent(secret)}@`;
    const query = ["vless", "trojan", "hy2"].includes(protocol)
      ? `?security=tls${sni ? `&sni=${encodeURIComponent(sni)}` : ""}`
      : "";
    // Native parsing owns validation; malformed input cannot throw a browser URL error.
    const uri = `${protocol}://${auth}${address}:${port}${query}#${encodeURIComponent(name || host)}`;
    if (await imported("import_text", { text: uri })) setSecret("");
  }

  return (
    <div className="overlay">
      <section
        ref={dialog}
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-title"
        onKeyDown={keydown}
      >
        <div className="section-top">
          <h2 id="import-title">{t("importTitle")}</h2>
          <button
            className="icon"
            disabled={busy}
            onClick={close}
            aria-label={t("close")}
          >
            <X />
          </button>
        </div>
        <div className="segments">
          {(["key", "subscription", "photo", "form"] as const).map((v) => (
            <button
              key={v}
              disabled={busy}
              className={tab === v ? "selected" : ""}
              onClick={() => setTab(v)}
            >
              {v === "photo" ? t("qr") : t(v)}
            </button>
          ))}
        </div>
        {error && (
          <div className="banner error" role="alert">
            <AlertCircle size={18} />
            <div>
              <strong>
                {error.code} · {error.stage}
              </strong>
              <p>{error.message}</p>
            </div>
          </div>
        )}
        <form onSubmit={submit}>
          {tab === "photo" ? (
            <div className="empty-small">
              <FileImage size={38} />
              <p>
                {t("qr")} — PNG, JPEG, WebP. HTTPS → subscription; VLESS / HY2 /
                Trojan / SS → profile.
              </p>
              <button
                type="button"
                disabled={busy}
                onClick={() => imported("import_file", { qr: true })}
              >
                <FolderOpen size={16} />
                {t("importFile")}
              </button>
            </div>
          ) : tab === "key" ? (
            <>
              <p>{t("keyHint")}</p>
              <HelpLabel topic="key" className="import-key-field">
                {t("key")}
                <textarea
                  spellCheck={false}
                  autoComplete="off"
                  value={text}
                  onChange={(e) => setText(e.target.value)}
                  placeholder={t("keyPlaceholder")}
                  aria-label={t("key")}
                />
              </HelpLabel>
              <div className="data-actions">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => imported("import_clipboard")}
                >
                  <Clipboard size={16} />
                  {t("paste")}
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => imported("import_file", { qr: false })}
                >
                  <FolderOpen size={16} />
                  {t("importFile")}
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => imported("import_file", { qr: true })}
                >
                  <FileImage size={16} />
                  {t("qr")}
                </button>
              </div>
            </>
          ) : tab === "subscription" ? (
            <>
              <HelpLabel topic="name">
                {t("name")}
                <input
                  required
                  value={name}
                  maxLength={256}
                  onChange={(e) => setName(e.target.value)}
                />
              </HelpLabel>
              <HelpLabel topic="url">
                {t("url")}
                <input
                  required
                  type="url"
                  value={url}
                  autoComplete="off"
                  spellCheck={false}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder="https://…"
                />
              </HelpLabel>
              <p>{t("subscriptionNote")}</p>
              <HelpLabel topic="protect" className="check-label">
                <input
                  type="checkbox"
                  checked={protectedUrl}
                  onChange={(e) => setProtectedUrl(e.target.checked)}
                />
                {t("protectSubscription")}
              </HelpLabel>
              <p className="small muted">{t("protectSubscriptionHelp")}</p>
            </>
          ) : (
            <>
              <div className="form-grid">
                <HelpLabel topic="name">
                  {t("name")}
                  <input
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                  />
                </HelpLabel>
                <HelpLabel topic="protocol">
                  {t("protocol")}
                  <select
                    value={protocol}
                    onChange={(e) => setProtocol(e.target.value)}
                  >
                    {[
                      "vless",
                      "trojan",
                      "hy2",
                      "ss",
                      "socks5",
                      "http",
                      "https",
                    ].map((p) => (
                      <option key={p} value={p}>
                        {p.toUpperCase()}
                      </option>
                    ))}
                  </select>
                </HelpLabel>
                <HelpLabel topic="host">
                  {t("host")}
                  <input
                    required
                    value={host}
                    onChange={(e) => setHost(e.target.value)}
                  />
                </HelpLabel>
                <HelpLabel topic="port">
                  {t("port")}
                  <input
                    required
                    type="number"
                    min="1"
                    max="65535"
                    value={port}
                    onChange={(e) => setPort(Number(e.target.value))}
                  />
                </HelpLabel>
                {proxy && (
                  <HelpLabel topic="username">
                    {t("username")}
                    <input
                      value={username}
                      autoComplete="off"
                      required={!!secret}
                      onChange={(e) => setUsername(e.target.value)}
                    />
                  </HelpLabel>
                )}
                <HelpLabel topic="secret">
                  {t("password")}
                  <input
                    required={!proxy}
                    type="password"
                    autoComplete="new-password"
                    value={secret}
                    onChange={(e) => setSecret(e.target.value)}
                  />
                </HelpLabel>
                {protocol === "ss" ? (
                  <HelpLabel topic="cipher">
                    {t("cipher")}
                    <select
                      value={cipher}
                      onChange={(e) => setCipher(e.target.value)}
                    >
                      {[
                        "chacha20-ietf-poly1305",
                        "aes-128-gcm",
                        "aes-256-gcm",
                        "2022-blake3-aes-128-gcm",
                        "2022-blake3-aes-256-gcm",
                        "2022-blake3-chacha20-poly1305",
                      ].map((v) => (
                        <option key={v}>{v}</option>
                      ))}
                    </select>
                  </HelpLabel>
                ) : (
                  !proxy && (
                    <HelpLabel topic="sni">
                      {t("sni")}
                      <input
                        value={sni}
                        onChange={(e) => setSni(e.target.value)}
                      />
                    </HelpLabel>
                  )
                )}
              </div>
              <p>{t("manualNote")}</p>
            </>
          )}
          <div className="modal-footer">
            <button type="button" disabled={busy} onClick={close}>
              {t("cancel")}
            </button>
            <button
              className="primary"
              type="submit"
              hidden={tab === "photo"}
              disabled={
                busy || tab === "photo" || (tab === "key" && !text.trim())
              }
            >
              {busy ? (
                <LoaderCircle size={17} className="spin" />
              ) : (
                <Plus size={17} />
              )}{" "}
              {t("import")}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
