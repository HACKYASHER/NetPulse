import { useEffect, useState, useReducer, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  Gauge,
  FileJson,
  Square,
  Trash2,
  Star,
  ChevronDown,
  MoreHorizontal,
  RefreshCw,
  Pencil,
  Clipboard,
} from "lucide-react";
import type { Profile, Subscription, NativeError } from "./types";
import { CountryFlag, serverName } from "./CountryFlag";
import { ExplorerView, displayView, type ExplorerMode } from "./ExplorerView";
import { pingMethods } from "./PingSettings";
import { ProfileJson } from "./ProfileJson";
import { subscriptionUsage } from "./subscriptionUsage";
import { SubscriptionSettings } from "./SubscriptionSettings";
import { initialLatency, latencyReducer } from "./latencyJob";
type Latency = {
  profile_id: string;
  method: string;
  timestamp: number;
  avg_ms: number | null;
  min_ms: number | null;
  successes: number;
  timeouts: number;
  failures: number;
  error: NativeError | null;
};
type Job = {
  id: string;
  kind: string;
  running: boolean;
  cancelled: boolean;
  completed: number;
  total: number;
  results: Latency[];
  error: NativeError | null;
};
export function ServerList(p: {
  profiles: Profile[];
  subscriptions: Subscription[];
  sourceFilter: string;
  query: string;
  subscriptionAction: (
    command:
      | "update_subscription"
      | "copy_subscription_url"
      | "delete_subscription",
    id: string,
  ) => void;
  selected: string;
  favorites: string[];
  toggleFavorite: (id: string) => void;
  pingMethod: import("./types").Preferences["ping_method"];
  view: ExplorerMode;
  changeView: (v: ExplorerMode) => void;
  choose: (id: string) => Promise<void>;
  remove: (v: Profile) => unknown;
  activeId?: string;
  busy: boolean;
  lang: string;
  error: (e: NativeError) => void;
  refresh: () => Promise<unknown>;
}) {
  const ru = p.lang === "ru";

  const [tab, setTab] = useState("all"),
    [job, setJob] = useState<Job | null>(null),
    [history, setHistory] = useState<Latency[]>([]);
  const [state, dispatch] = useReducer(latencyReducer, initialLatency);
  const [collapsed, setCollapsed] = useState<string[]>([]);
  const [jsonProfile, setJsonProfile] = useState<Profile | null>(null);
  const [editing, setEditing] = useState<Subscription | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const starting = useRef(false);
  const [pollVersion, setPollVersion] = useState(0);
  const load = () =>
    invoke<Latency[]>("diagnostic_history", { kind: "latency" }).then(
      setHistory,
    );
  const method = p.pingMethod;
  useEffect(() => {
    let disposed = false;
    void load().catch(p.error);
    void invoke<Job | null>("current_diagnostic")
      .then((v) => {
        if (!disposed && v?.kind === "latency" && v.running) {
          setJob(v);
          dispatch({ type: "accepted", id: v.id });
        }
      })
      .catch(p.error);
    return () => {
      disposed = true;
    };
  }, []);
  useEffect(() => {
    if (!state.id) return;
    let disposed = false;
    let failures = 0;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const v = await invoke<Job>("diagnostic_status", { id: state.id });
        if (disposed) return;
        failures = 0;
        setJob(v);
        if (v.running) timer = setTimeout(poll, 500);
        else {
          dispatch({ type: "finished" });
          if (v.error) p.error(v.error);
          await load();
          await p.refresh();
        }
      } catch (error) {
        if (disposed) return;
        failures++;
        if (failures < 3) timer = setTimeout(poll, 1000);
        else {
          void invoke("cancel_diagnostic", { id: state.id }).catch(() => {});
          setJob(null);
          dispatch({ type: "failed" });
          p.error(error as NativeError);
        }
      }
    };
    void poll();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [state.id, pollVersion]);
  const running = state.phase !== "idle";
  const start = async (ids: string[]) => {
    if (starting.current || running || p.busy) return;
    starting.current = true;
    dispatch({ type: "start" });
    try {
      const existing = await invoke<Job | null>("current_diagnostic");
      if (existing?.running) {
        if (existing.kind !== "latency")
          throw {
            code: "DiagnosticInProgress",
            stage: "latency-start",
            message: ru
              ? "Сначала завершите тест скорости."
              : "Finish the speed test first.",
          };
        setJob(existing);
        dispatch({ type: "accepted", id: existing.id });
        setPollVersion((v) => v + 1);
        return;
      }
      const id = await invoke<string>("start_latency", { ids, method });
      setJob({
        id,
        kind: "latency",
        running: true,
        cancelled: false,
        completed: 0,
        total: ids.length,
        results: [],
        error: null,
      });
      dispatch({ type: "accepted", id });
    } catch (error) {
      setJob(null);
      dispatch({ type: "failed" });
      p.error(error as NativeError);
    } finally {
      starting.current = false;
    }
  };
  const cancel = async () => {
    if (!state.id) return;
    dispatch({ type: "cancel" });
    try {
      await invoke("cancel_diagnostic", { id: state.id });
    } catch (error) {
      dispatch({ type: "accepted", id: state.id });
      p.error(error as NativeError);
    }
  };
  const items = p.profiles.filter(
    (v) =>
      tab === "all" ||
      (tab === "selected" ? v.id === p.selected : p.favorites.includes(v.id)),
  );
  const groups = [
    ...new Set([
      ...items.map((v) => v.subscription_id || "manual"),
      ...(tab === "all" && !p.query.trim()
        ? p.subscriptions
            .filter((s) => !p.sourceFilter || p.sourceFilter === s.id)
            .map((s) => s.id)
        : []),
    ]),
  ].map((id) => ({
    id,
    subscription: p.subscriptions.find((s) => s.id === id),
    name:
      p.subscriptions.find((s) => s.id === id)?.name ||
      (ru ? "Мои серверы" : "My servers"),
    items: items.filter((v) => (v.subscription_id || "manual") === id),
  }));
  const measurements = [...(job?.results || []), ...history]
    .filter((v) => v.method === method)
    .sort((a, b) => b.timestamp - a.timestamp);
  return (
    <section className="server-browser">
      <div className="data-actions collection-toolbar">
        <div className="segments">
          {[
            ["all", "Все", "All"],
            ["selected", "Выбранный", "Selected"],
            ["favorites", "Избранные", "Favorites"],
          ].map(([v, r, e]) => (
            <button key={v} aria-pressed={tab === v} onClick={() => setTab(v)}>
              {ru ? r : e}
            </button>
          ))}
        </div>

        <button
          disabled={running || p.busy || !p.selected}
          onClick={() => start([p.selected])}
        >
          <Gauge size={14} />
          {ru ? "Проверить выбранный" : "Test selected"}
        </button>
        <button
          disabled={running || p.busy || !items.length}
          onClick={() => start(items.map((v) => v.id))}
        >
          {ru ? "Проверить список" : "Test list"} ({items.length})
        </button>
        {running && (
          <button
            disabled={state.phase === "cancelling" || !state.id}
            onClick={() => void cancel()}
          >
            <Square size={14} />
            {state.phase === "cancelling"
              ? ru
                ? "Отменяем…"
                : "Cancelling…"
              : ru
                ? "Отменить"
                : "Cancel"}
          </button>
        )}
        <ExplorerView value={p.view} change={p.changeView} lang={p.lang} />
      </div>
      <p className="small muted">
        {method.startsWith("proxy_https")
          ? ru
            ? "Через профиль • лучший из двух запросов"
            : "Through profile • best of two requests"
          : ru
            ? method === "tcp_connect_rtt"
              ? "До сервера вне TUN NetPulse • не проверяет VPN"
              : "ICMP до сервера • маршруты Windows • не проверяет VPN"
            : "Server reachability • does not verify VPN"}{" "}
        · {pingMethods.find((v) => v[0] === method)?.[1]}{" "}
        {running && (ru ? "• Проверка выполняется" : "• Running")}{" "}
        {job && `${job.completed}/${job.total}`}
      </p>
      <div className="server-groups">
        {groups.map((group) => (
          <section className="server-group" key={group.id}>
            <div className="subscription-heading">
              <button
                className="group-toggle"
                aria-expanded={!collapsed.includes(group.id)}
                onClick={() =>
                  setCollapsed((v) =>
                    v.includes(group.id)
                      ? v.filter((id) => id !== group.id)
                      : [...v, group.id],
                  )
                }
              >
                <ChevronDown
                  size={16}
                  className={collapsed.includes(group.id) ? "collapsed" : ""}
                />
                <span>
                  <strong>{group.name}</strong>
                  {group.subscription && (
                    <small>
                      {group.subscription.updated_at
                        ? new Date(
                            group.subscription.updated_at * 1000,
                          ).toLocaleString(p.lang)
                        : ru
                          ? "Ещё не обновлена"
                          : "Not refreshed"}{" "}
                      · {ru ? "Автообновление" : "Refresh"}:{" "}
                      {group.subscription.interval_hours} {ru ? "ч" : "h"}
                    </small>
                  )}
                </span>
                <span className="count">
                  {group.subscription?.count ?? group.items.length}
                </span>
              </button>
              {group.subscription && (
                <button
                  className="icon"
                  disabled={p.busy || running}
                  title={ru ? "Обновить подписку" : "Refresh subscription"}
                  aria-label={(ru ? "Обновить " : "Refresh ") + group.name}
                  onClick={() =>
                    p.subscriptionAction("update_subscription", group.id)
                  }
                >
                  <RefreshCw size={16} />
                </button>
              )}
              <button
                className="icon"
                disabled={p.busy || running || !group.items.length}
                title={ru ? "Проверить пинг группы" : "Test group latency"}
                aria-label={
                  (ru ? "Проверить пинг " : "Test latency ") + group.name
                }
                onClick={() => void start(group.items.map((v) => v.id))}
              >
                <Gauge size={15} />
              </button>
              {group.subscription && (
                <details className="subscription-menu">
                  <summary
                    aria-label={
                      (ru ? "Действия подписки " : "Subscription actions ") +
                      group.name
                    }
                  >
                    <MoreHorizontal size={18} />
                  </summary>
                  <div
                    className="subscription-menu-items"
                    onClick={(e) =>
                      e.currentTarget.parentElement?.removeAttribute("open")
                    }
                  >
                    <button
                      disabled={p.busy}
                      onClick={() => setEditing(group.subscription!)}
                    >
                      <Pencil size={15} />
                      {ru ? "Редактировать" : "Edit"}
                    </button>
                    <button
                      disabled={p.busy || group.subscription.url_protected}
                      title={
                        group.subscription.url_protected
                          ? ru
                            ? "Разрешите копирование в настройках подписки"
                            : "Allow copying in subscription settings"
                          : undefined
                      }
                      onClick={() =>
                        p.subscriptionAction("copy_subscription_url", group.id)
                      }
                    >
                      <Clipboard size={15} />
                      {ru ? "Копировать URL" : "Copy URL"}
                    </button>
                    <button
                      disabled={
                        p.busy ||
                        p.profiles.some(
                          (v) =>
                            v.id === p.activeId &&
                            v.subscription_id === group.id,
                        )
                      }
                      onClick={() => setDeleting(group.id)}
                    >
                      <Trash2 size={15} />
                      {ru ? "Удалить подписку" : "Delete subscription"}
                    </button>
                  </div>
                </details>
              )}
            </div>
            {group.subscription && (
              <div className="subscription-usage">
                {subscriptionUsage(group.subscription, p.lang)}
              </div>
            )}
            {deleting === group.id && (
              <div className="banner">
                <span className="grow">
                  {ru
                    ? "Удалить подписку вместе с её серверами?"
                    : "Delete this subscription and its servers?"}
                </span>
                <button
                  disabled={p.busy}
                  onClick={() => {
                    p.subscriptionAction("delete_subscription", group.id);
                    setDeleting(null);
                  }}
                >
                  {ru ? "Удалить" : "Delete"}
                </button>
                <button onClick={() => setDeleting(null)}>
                  {ru ? "Отмена" : "Cancel"}
                </button>
              </div>
            )}
            <div
              hidden={collapsed.includes(group.id)}
              className={"server-catalog " + displayView(p.view)}
            >
              {group.items.map((v) => {
                const m = measurements.find((a) => a.profile_id === v.id);
                const fallback = v.latency_method === method;
                const success = m
                  ? m.successes > 0
                  : fallback && v.latency_status === "success";
                const ms = m ? m.min_ms : fallback ? v.latency_ms : null;
                const label = m
                  ? m.successes
                    ? `${ms == null ? "—" : ms < 1 ? "<1" : ms.toFixed(0)} ms`
                    : m.timeouts
                      ? "Timeout"
                      : ru
                        ? "Ошибка"
                        : "Error"
                  : fallback && v.latency_status === "failed"
                    ? ru
                      ? "Ошибка"
                      : "Error"
                    : ms != null
                      ? `${ms < 1 ? "<1" : ms.toFixed(0)} ms`
                      : "—";
                return (
                  <article
                    key={v.id}
                    className={`server-tile ${v.id === p.selected ? "chosen" : ""}`}
                  >
                    <button
                      className="server-tile-main"
                      disabled={p.busy}
                      onClick={() => p.choose(v.id)}
                      title={`${v.name}\n${v.host}:${v.port}`}
                    >
                      <span className="server-tile-name">
                        <CountryFlag name={v.name} />
                        <strong>{serverName(v.name)}</strong>
                        {v.id === p.selected && <Check size={14} />}
                      </span>
                      <span className="server-tile-meta">
                        {v.protocol.toUpperCase()} ·{" "}
                        {p.subscriptions.find((s) => s.id === v.subscription_id)
                          ?.name || (ru ? "Вручную" : "Manual")}
                      </span>
                      <span className="server-tile-address">
                        {v.host}:{v.port}
                      </span>
                      <span
                        className={`server-latency ${success ? "ok" : label === "—" ? "" : "failed"}`}
                        title={
                          (m
                            ? m.error?.message ||
                              `${pingMethods.find((v) => v[0] === method)?.[1]}: ${m.successes} ✓ / ${m.timeouts} timeout / ${m.failures} error`
                            : null) ||
                          (m?.timestamp || v.latency_at
                            ? new Date(
                                (m?.timestamp || v.latency_at || 0) * 1000,
                              ).toLocaleString()
                            : "")
                        }
                      >
                        {label}
                        {m && m.successes > 0 && m.timeouts + m.failures > 0
                          ? " *"
                          : ""}
                      </span>
                    </button>
                    <button
                      className="icon server-json"
                      disabled={p.busy || !!group.subscription?.config_locked}
                      aria-label={ru ? "Редактировать JSON" : "Edit JSON"}
                      title={
                        group.subscription?.config_locked
                          ? ru
                            ? "Редактирование запрещено для этой подписки"
                            : "Editing is locked for this subscription"
                          : ru
                            ? "Конфигурация сервера (JSON)"
                            : "Server configuration (JSON)"
                      }
                      onClick={() => setJsonProfile(v)}
                    >
                      <FileJson size={14} />
                    </button>
                    <button
                      className="icon server-favorite"
                      disabled={p.busy}
                      aria-pressed={p.favorites.includes(v.id)}
                      aria-label={`${ru ? "Избранное" : "Favorite"} ${v.name}`}
                      onClick={() => p.toggleFavorite(v.id)}
                    >
                      <Star
                        size={14}
                        fill={
                          p.favorites.includes(v.id) ? "currentColor" : "none"
                        }
                      />
                    </button>
                    <button
                      className="icon server-remove"
                      disabled={p.busy || p.activeId === v.id}
                      aria-label={`${ru ? "Удалить" : "Remove"} ${v.name}`}
                      onClick={() => p.remove(v)}
                    >
                      <Trash2 size={13} />
                    </button>
                  </article>
                );
              })}
            </div>
          </section>
        ))}
      </div>
      {!groups.length && (
        <p className="empty-small">
          {ru ? "Нет серверов в этой вкладке" : "No servers in this tab"}
        </p>
      )}
      {jsonProfile && (
        <ProfileJson
          profile={jsonProfile}
          lang={p.lang}
          close={() => setJsonProfile(null)}
          refresh={p.refresh}
        />
      )}
      {editing && (
        <SubscriptionSettings
          subscription={editing}
          lang={p.lang}
          close={() => setEditing(null)}
          refresh={p.refresh}
          error={p.error}
        />
      )}
    </section>
  );
}
