import { HelpLabel, Help } from "./Help";
import { useEffect, useRef, useState } from "react";
import { Search, Star, X, RefreshCw, Check, Server } from "lucide-react";
import { ExplorerView, displayView, type ExplorerMode } from "./ExplorerView";
import { CountryFlag, serverName } from "./CountryFlag";
import type { Profile, Subscription } from "./types";
type Props = {
  view: ExplorerMode;
  changeView: (v: ExplorerMode) => void;
  profiles: Profile[];
  subscriptions: Subscription[];
  selected: string;
  favorites: string[];
  busy: boolean;
  lang: string;
  choose: (id: string) => Promise<void>;
  close: () => void;
  toggleFavorite: (id: string) => void;
  refresh: (id: string) => Promise<void>;
};
export function ServerSelector(p: Props) {
  const [search, setSearch] = useState(""),
    [group, setGroup] = useState(""),
    [protocol, setProtocol] = useState(""),
    [sort, setSort] = useState("name"),
    [favoritesOnly, setFavoritesOnly] = useState(false);
  const root = useRef<HTMLElement>(null);
  const ru = p.lang === "ru";
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    root.current?.querySelector<HTMLInputElement>("input")?.focus();
    return () => previous?.focus();
  }, []);
  const items = p.profiles
    .filter(
      (v) =>
        (!group ||
          (group === "manual"
            ? !v.subscription_id
            : v.subscription_id === group)) &&
        (!protocol || v.protocol === protocol) &&
        (!favoritesOnly || p.favorites.includes(v.id)) &&
        `${v.name} ${v.protocol} ${v.host}`
          .toLowerCase()
          .includes(search.toLowerCase()),
    )
    .sort((a, b) =>
      sort === "latency"
        ? (a.latency_ms ?? Infinity) - (b.latency_ms ?? Infinity) ||
          a.name.localeCompare(b.name)
        : a.name.localeCompare(b.name),
    );
  return (
    <div
      className="overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && !p.busy) p.close();
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape" && !p.busy) p.close();
        if (e.key === "Tab") {
          const elements = [
            ...root.current!.querySelectorAll<HTMLElement>(
              "button:not(:disabled),input,select",
            ),
          ];
          const first = elements[0],
            last = elements.at(-1);
          if (e.shiftKey && document.activeElement === first) {
            e.preventDefault();
            last?.focus();
          } else if (!e.shiftKey && document.activeElement === last) {
            e.preventDefault();
            first?.focus();
          }
        }
      }}
    >
      <section
        ref={root}
        className="modal server-picker"
        role="dialog"
        aria-modal="true"
        aria-labelledby="server-title"
      >
        <div className="section-top">
          <h2 id="server-title">{ru ? "Выбрать сервер" : "Choose server"}</h2>
          <button
            className="icon"
            disabled={p.busy}
            onClick={p.close}
            aria-label={ru ? "Закрыть" : "Close"}
          >
            <X />
          </button>
        </div>
        <div className="search">
          <Search size={18} />
          <span className="control-with-help">
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={
                ru ? "Название, протокол или адрес" : "Name, protocol or host"
              }
            />
            <Help topic="filter" />
          </span>
        </div>
        <div className="server-filters">
          <span className="control-with-help">
            <select
              aria-label="Subscription"
              value={group}
              onChange={(e) => setGroup(e.target.value)}
            >
              <option value="">{ru ? "Все подписки" : "All sources"}</option>
              <option value="manual">{ru ? "Вручную" : "Manual"}</option>
              {p.subscriptions.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
            <Help topic="source" />
          </span>
          <span className="control-with-help">
            <select
              aria-label="Protocol"
              value={protocol}
              onChange={(e) => setProtocol(e.target.value)}
            >
              <option value="">{ru ? "Все протоколы" : "All protocols"}</option>
              {[...new Set(p.profiles.map((v) => v.protocol))].map((v) => (
                <option key={v}>{v}</option>
              ))}
            </select>
            <Help topic="protocol" />
          </span>
          <span className="control-with-help">
            <select
              aria-label="Sort"
              value={sort}
              onChange={(e) => setSort(e.target.value)}
            >
              <option value="name">{ru ? "По имени" : "Name"}</option>
              <option value="latency">{ru ? "По задержке" : "Latency"}</option>
            </select>
            <Help topic="sort" />
          </span>
          <button
            aria-pressed={favoritesOnly}
            onClick={() => setFavoritesOnly(!favoritesOnly)}
          >
            <Star size={16} />
            {ru ? "Избранные" : "Favorites"}
          </button>
        </div>
        {group && group !== "manual" && (
          <button disabled={p.busy} onClick={() => p.refresh(group)}>
            <RefreshCw size={16} />
            {ru ? "Обновить подписку" : "Refresh subscription"}
          </button>
        )}
        <ExplorerView value={p.view} change={p.changeView} lang={p.lang} />
        <div className={`server-results ${displayView(p.view)}`}>
          {items.map((v) => (
            <article
              className={`server-option ${v.id === p.selected ? "chosen" : ""}`}
              key={v.id}
            >
              <CountryFlag name={v.name} />
              <div>
                <strong>{serverName(v.name)}</strong>
                <small>
                  {v.protocol.toUpperCase()} ·{" "}
                  {p.subscriptions.find((s) => s.id === v.subscription_id)
                    ?.name || (ru ? "Вручную" : "Manual")}
                </small>
                <small>
                  {v.host}:{v.port} ·{" "}
                  {v.latency_ms == null ? "—" : `${v.latency_ms} ms`}{" "}
                  {v.latency_method === "tcp_connect_rtt"
                    ? "TCP"
                    : v.latency_method
                      ? "HTTPS"
                      : ""}{" "}
                  {v.latency_at
                    ? `· ${new Date(v.latency_at * 1000).toLocaleString()}`
                    : ""}{" "}
                  {v.latency_status === "failed"
                    ? ru
                      ? "Ошибка проверки"
                      : "Probe failed"
                    : ""}
                </small>
              </div>
              <button
                className="icon"
                disabled={p.busy}
                aria-label={ru ? "Избранное" : "Favorite"}
                aria-pressed={p.favorites.includes(v.id)}
                onClick={() => p.toggleFavorite(v.id)}
              >
                <Star
                  fill={p.favorites.includes(v.id) ? "currentColor" : "none"}
                  size={18}
                />
              </button>
              <button disabled={p.busy} onClick={() => p.choose(v.id)}>
                {v.id === p.selected ? (
                  <Check size={18} />
                ) : ru ? (
                  "Выбрать"
                ) : (
                  "Choose"
                )}
              </button>
            </article>
          ))}
          {!items.length && (
            <p>{ru ? "Серверы не найдены" : "No matching servers"}</p>
          )}
        </div>
      </section>
    </div>
  );
}
