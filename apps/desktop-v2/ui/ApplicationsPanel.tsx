import { HelpLabel, Help } from "./Help";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Monitor, Search, FolderOpen, Trash2 } from "lucide-react";
import { ExplorerView, displayView } from "./ExplorerView";
import type { Preferences, Application } from "./types";
import {
  mergeCatalog,
  catalogRows,
  normalizedPath,
  readCatalog,
  savedApplications,
  clearCatalog,
} from "./applicationCatalog";
const iconCache = new Map<string, Promise<string | null>>();
let readers = 0;
let catalogWrites: Promise<void> = Promise.resolve();
const iconQueue: (() => void)[] = [];
function iconFor(path: string) {
  if (!iconCache.has(path))
    iconCache.set(
      path,
      new Promise((resolve) => {
        const run = () => {
          readers++;
          invoke<string | null>("application_icon", { path })
            .then(resolve, () => resolve(null))
            .finally(() => {
              readers--;
              iconQueue.shift()?.();
            });
        };
        if (readers < 4) run();
        else iconQueue.push(run);
      }),
    );
  return iconCache.get(path)!;
}
function ApplicationIcon({ path }: { path: string }) {
  const [icon, setIcon] = useState<string | null>(null);
  const root = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    let disposed = false;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        observer.disconnect();
        iconFor(path).then((v) => {
          if (!disposed) setIcon(v);
        });
      }
    });
    if (root.current) observer.observe(root.current);
    return () => {
      disposed = true;
      observer.disconnect();
    };
  }, [path]);
  return (
    <span ref={root} className="app-icon">
      {icon ? (
        <img src={icon} alt="" width={28} height={28} />
      ) : (
        <Monitor size={24} />
      )}
    </span>
  );
}
const unique = (paths: string[]) => [
  ...new Map(paths.map((p) => [normalizedPath(p), p])).values(),
];
export function ApplicationsPanel(p: {
  draft: Preferences;
  smoke?: boolean;
  change: (v: Preferences) => void;
  changeView: (v: Preferences["app_view"]) => void;
  busy: boolean;
  lang: string;
  scan: () => Promise<Application[] | undefined>;
  choose: () => Promise<Application | null | undefined>;
}) {
  const [apps, setApps] = useState<Application[]>(() =>
      p.smoke
        ? [
            {
              name: "Fixture",
              path: "C:\\NetPulseFixture\\test.exe",
              source: "manual",
            },
            {
              name: "Codex fixture",
              path: "C:\\NetPulseFixture\\Codex\\bin\\0123456789abcdef\\codex.exe",
              source: "manual",
            },
            {
              name: "Codex fixture",
              path: "C:\\NetPulseFixture\\Codex\\bin\\fedcba9876543210\\codex.exe",
              source: "manual",
            },
          ]
        : mergeCatalog(savedApplications(p.draft.applications), readCatalog()),
    ),
    [search, setSearch] = useState(""),
    [source, setSource] = useState(""),
    [group, setGroup] = useState(""),
    [limit, setLimit] = useState(100),
    [selectedOnly, setSelectedOnly] = useState(false);
  const ru = p.lang === "ru",
    paths = p.draft.applications;
  const all = mergeCatalog(savedApplications(paths), apps);
  const [catalogLoaded, setCatalogLoaded] = useState(false);
  const [catalogError, setCatalogError] = useState("");
  useEffect(() => {
    if (p.smoke) return;
    let disposed = false;
    catalogWrites
      .catch(() => {})
      .then(() => invoke<Application[] | null>("read_application_catalog"))
      .then((saved) => {
        if (disposed) return;
        setApps((current) =>
          mergeCatalog(saved ?? [], current, savedApplications(paths)),
        );
        setCatalogLoaded(true);
      })
      .catch(() => {
        if (!disposed)
          setCatalogError(
            ru
              ? "Не удалось прочитать applications.json. Файл сохранён; повторите открытие раздела."
              : "Could not read applications.json. File preserved; reopen this section.",
          );
      });
    return () => {
      disposed = true;
    };
  }, []);
  useEffect(() => {
    if (!catalogLoaded) return;
    catalogWrites = catalogWrites
      .catch(() => {})
      .then(() => invoke("save_application_catalog", { applications: all }))
      .then(
        () => {
          clearCatalog();
          setCatalogError("");
        },
        () =>
          setCatalogError(
            ru
              ? "Не удалось сохранить каталог приложений."
              : "Could not save the application catalog.",
          ),
      );
  }, [apps, paths, catalogLoaded]);
  useEffect(() => {
    setApps((current) => {
      const merged = mergeCatalog(savedApplications(paths), current);
      return JSON.stringify(merged) === JSON.stringify(current)
        ? current
        : merged;
    });
  }, [paths]);
  const change = (values: string[]) => {
    // Preserve rows before removing their routing selection.
    setApps(all);
    p.change({ ...p.draft, applications: unique(values) });
  };
  const filtered = catalogRows(all).filter(
    (a) =>
      (!source || a.source === source) &&
      (!selectedOnly ||
        a.paths.some((path) =>
          paths.some((v) => normalizedPath(v) === normalizedPath(path)),
        )) &&
      `${a.name} ${a.paths.join(" ")}`
        .toLowerCase()
        .includes(search.toLowerCase()),
  );
  const visible = filtered.slice(0, limit);
  return (
    <section className="card">
      <div className="section-top">
        <div>
          <h3>
            {ru ? "Приложения" : "Applications"}{" "}
            <span className="count">
              {catalogRows(savedApplications(paths)).length}
            </span>{" "}
            <Help topic="app" />
          </h3>
          <p>
            {ru ? "Общий список" : "Shared list"} ·{" "}
            {ru
              ? "Каталог сохраняется в applications.json. Известные версии одной установки объединяются в строку. Разные пути одноимённых EXE остаются отдельными. Снятые галочки оставляют запись во «Всех приложениях»."
              : "Discord and Roblox include new versions and known updaters within the same installation. Add other apps’ EXEs separately. Deselected entries remain in All apps."}
          </p>
        </div>
        <div className="row">
          <button
            disabled={p.busy}
            onClick={async () => {
              const found = await p.scan();
              if (found)
                setApps((current) =>
                  mergeCatalog(savedApplications(paths), found, current),
                );
            }}
          >
            <Search size={16} />
            {ru ? "Найти приложения" : "Scan apps"}
          </button>
          <button
            disabled={p.busy}
            onClick={async () => {
              const found = await p.choose();
              if (found) {
                change([...paths, found.path]);
                setApps((a) => mergeCatalog([found], a));
              }
            }}
          >
            <FolderOpen size={16} />
            {ru ? "Добавить EXE" : "Add EXE"}
          </button>
        </div>
      </div>
      <div className="search">
        <Search size={16} />
        <span className="control-with-help">
          <input
            value={search}
            placeholder={ru ? "Поиск приложения" : "Search apps"}
            onChange={(e) => {
              setSearch(e.target.value);
              setLimit(100);
            }}
          />
          <Help topic="filter" />
        </span>
      </div>
      <div className="data-actions collection-toolbar">
        <span className="control-with-help">
          <select
            aria-label="Application source"
            value={source}
            onChange={(e) => {
              setSource(e.target.value);
              setLimit(100);
            }}
          >
            <option value="">{ru ? "Все источники" : "All sources"}</option>
            {[...new Set(all.map((a) => a.source))].sort().map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
          <Help topic="source" />
        </span>

        <button
          aria-pressed={!selectedOnly}
          onClick={() => {
            setSelectedOnly(false);
            setLimit(100);
          }}
        >
          {ru ? "Все приложения" : "All apps"}
        </button>
        <button
          aria-pressed={selectedOnly}
          onClick={() => {
            setSelectedOnly(true);
            setLimit(100);
          }}
        >
          {ru ? "Добавленные" : "Added"} ({paths.length})
        </button>
        <button
          disabled={p.busy}
          onClick={() =>
            change(
              unique([...paths, ...visible.flatMap((a) => a.paths)]).slice(
                0,
                200,
              ),
            )
          }
        >
          {ru ? "Выбрать видимые (до 200)" : "Select visible (up to 200)"}
        </button>
        <button
          disabled={p.busy}
          onClick={() =>
            change(
              paths.filter(
                (path) =>
                  !visible.some((a) =>
                    a.paths.some(
                      (v) => normalizedPath(v) === normalizedPath(path),
                    ),
                  ),
              ),
            )
          }
        >
          {ru ? "Снять видимые" : "Deselect visible"}
        </button>
        <ExplorerView
          value={p.draft.app_view}
          lang={p.lang}
          change={p.changeView}
        />
      </div>
      {catalogError && <p className="error-text">{catalogError}</p>}
      <div className={`apps-list ${displayView(p.draft.app_view)}`}>
        {visible.map((a) => (
          <label
            title={a.paths.join("\n")}
            className="application-row"
            key={a.path.toLowerCase()}
          >
            <input
              type="checkbox"
              disabled={p.busy}
              ref={(element) => {
                if (element) {
                  const selected = a.paths.filter((path) =>
                    paths.some(
                      (v) => normalizedPath(v) === normalizedPath(path),
                    ),
                  ).length;
                  element.indeterminate =
                    selected > 0 && selected < a.paths.length;
                }
              }}
              checked={a.paths.every((path) =>
                paths.some((v) => normalizedPath(v) === normalizedPath(path)),
              )}
              onChange={(e) =>
                change(
                  e.target.checked
                    ? [...paths, ...a.paths]
                    : paths.filter(
                        (v) =>
                          !a.paths.some(
                            (path) =>
                              normalizedPath(v) === normalizedPath(path),
                          ),
                      ),
                )
              }
            />
            <ApplicationIcon path={a.path} />
            <span>
              <strong>{a.name}</strong>
              <small>
                {a.path}
                {a.paths.length > 1
                  ? " · " + a.paths.length + (ru ? " версии" : " versions")
                  : ""}
              </small>
              <small>{a.source}</small>
            </span>
          </label>
        ))}
      </div>
      {!visible.length && (
        <p className="empty-small">
          {ru
            ? selectedOnly
              ? "Выбранных приложений нет. Откройте «Все приложения», чтобы вернуть галочки."
              : "Найдите приложения или добавьте EXE вручную."
            : selectedOnly
              ? "No selected apps. Open All apps to select them again."
              : "Scan applications or add an EXE manually."}
        </p>
      )}
      {filtered.length > visible.length && (
        <button onClick={() => setLimit(limit + 100)}>
          {ru ? "Показать ещё" : "Show more"} ({visible.length}/
          {filtered.length})
        </button>
      )}
      <h4>{ru ? "Группы приложений" : "Application groups"}</h4>
      <div className="data-actions">
        <span className="control-with-help">
          <input
            value={group}
            maxLength={128}
            placeholder={ru ? "Название группы" : "Group name"}
            onChange={(e) => setGroup(e.target.value)}
          />
          <Help topic="group" />
        </span>
        <button
          disabled={!group.trim() || !paths.length}
          onClick={() => {
            p.change({
              ...p.draft,
              application_groups: [
                ...p.draft.application_groups.filter(
                  (g) => g.name !== group.trim(),
                ),
                { name: group.trim(), paths },
              ],
            });
            setGroup("");
          }}
        >
          {ru ? "Сохранить выбранные в группу" : "Group selected apps"}
        </button>
      </div>
      {p.draft.application_groups.map((g) => (
        <div className="section-top" key={g.name}>
          <span>
            {g.name} · {g.paths.length}
          </span>
          <div className="row">
            <button onClick={() => change(g.paths)}>
              {ru ? "Применить к списку" : "Use group"}
            </button>
            <button
              className="icon"
              aria-label={ru ? "Удалить группу" : "Delete group"}
              onClick={() =>
                p.change({
                  ...p.draft,
                  application_groups: p.draft.application_groups.filter(
                    (v) => v.name !== g.name,
                  ),
                })
              }
            >
              <Trash2 size={15} />
            </button>
          </div>
        </div>
      ))}
    </section>
  );
}
