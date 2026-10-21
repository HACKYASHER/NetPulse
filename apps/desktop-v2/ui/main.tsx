import { checkLayout } from "./layoutSmoke";
import { CoreSelector } from "./CoreSelector";
import {
  connectionUnconfirmed,
  recoveredNotice,
  serviceMonitorError,
} from "./connectionHealth";
import { HelpLabel, Help, setHelpLanguage } from "./Help";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { clearCatalog } from "./applicationCatalog";
import { createRoot } from "react-dom/client";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Check,
  ChevronRight,
  Clipboard,
  Download,
  FileImage,
  Globe,
  Layers,
  Link,
  LoaderCircle,
  Plus,
  Power,
  RefreshCw,
  Search,
  Server,
  Settings,
  Shield,
  SlidersHorizontal,
  Trash2,
  Wifi,
  X,
  Upload,
  FolderOpen,
  AlertCircle,
  Monitor,
} from "lucide-react";
import type {
  Application,
  Bootstrap,
  ClientRelease,
  ImportReport,
  NativeError,
  Preferences,
  Profile,
  Snapshot,
} from "./types";
import { ru, en, type Label } from "./i18n";
import "./style.css";
import { ImportModal } from "./ImportModal";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  preferencesChanged,
  mergeQuickNetwork,
  mergeQuickUi,
  hasUnsavedChanges,
} from "./preferences";
import { ServerSelector } from "./ServerSelector";
import { NetworkSettings, DnsSettings } from "./NetworkSettings";
import { ProfileNetwork, TransportSettings, Playground } from "./NetworkTools";
import { AdvancedRules } from "./AdvancedRules";
import { FileLogs } from "./FileLogs";
import { MtuAnalysis } from "./MtuAnalysis";
import { ActiveLatency } from "./ActiveLatency";
import { PingSettings } from "./PingSettings";
import { CoreSettings } from "./CoreSettings";
import { ServerList } from "./ServerList";
import { CountryFlag, serverName } from "./CountryFlag";
import { ApplicationsPanel } from "./ApplicationsPanel";
import { AboutSettings } from "./AboutSettings";
import { BestServer } from "./BestServer";
import { RoutingSettings } from "./RoutingSettings";
import { palettes, surfaceColors } from "./theme";
import { pollClientUpdates } from "./clientUpdate";

type Page =
  | "dashboard"
  | "connections"
  | "routing"
  | "activity"
  | "networkSettings"
  | "settings";
const icons = {
  dashboard: Layers,
  connections: Server,
  routing: SlidersHorizontal,
  activity: Activity,
  networkSettings: Globe,
  settings: Settings,
};
const bytes = (n: number) => {
  const i = Math.min(Math.floor(Math.log(Math.max(n, 1)) / Math.log(1024)), 3);
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${["B", "KB", "MB", "GB"][i]}`;
};
function App() {
  const observedError = useRef("");
  const errorNoticeKey = useRef("");
  const [settingsTab, setSettingsTab] = useState("appearance");
  const [networkTab, setNetworkTab] = useState("general");
  const [clientRelease, setClientRelease] = useState<ClientRelease | null>(
    null,
  );
  const [showUpdateNotice, setShowUpdateNotice] = useState(false);
  const notifiedUpdate = useRef("");
  const receiveClientRelease = useCallback((release: ClientRelease | null) => {
    setClientRelease(release);
    if (!release) setShowUpdateNotice(false);
    else if (notifiedUpdate.current !== release.tag) {
      notifiedUpdate.current = release.tag;
      setShowUpdateNotice(true);
    }
  }, []);
  useEffect(() => {
    if (!showUpdateNotice) return;
    const timer = setTimeout(() => setShowUpdateNotice(false), 12_000);
    return () => clearTimeout(timer);
  }, [showUpdateNotice, clientRelease?.tag]);
  const [bannerPaused, setBannerPaused] = useState(false);
  const [data, setData] = useState<Bootstrap | null>(null),
    [page, setPage] = useState<Page>("dashboard"),
    [fatal, setFatal] = useState(""),
    [busy, setBusy] = useState(false),
    [notice, setNotice] = useState(""),
    [error, setErrorState] = useState<NativeError | null>(null),
    [selected, setSelected] = useState(""),
    [modal, setModal] = useState(false),
    [report, setReport] = useState<ImportReport | null>(null),
    [publicIp, setPublicIp] = useState(""),
    [directIp, setDirectIp] = useState(""),
    [history, setHistory] = useState<{ up: number; down: number }[]>([]);
  const setError = useCallback((value: NativeError | null) => {
    if (value) {
      const key = `${value.code}:${value.stage}:${value.message}`;
      if (errorNoticeKey.current === key) return;
      errorNoticeKey.current = key;
    }
    setErrorState(value);
  }, []);
  useEffect(() => {
    const snapshot = data?.snapshot;
    if (!snapshot) return;
    const next = snapshot.error;
    const key = next ? `${next.code}:${next.stage}:${next.message}` : "";
    if (key && key !== observedError.current) setError(next);
    else if (!serviceMonitorError(next)) {
      setErrorState((current) => recoveredNotice(current, snapshot));
    }
    if (
      !serviceMonitorError(next) &&
      /^(ServiceTimeout|IpcTimeout|ServiceBusy|ServiceMonitorUnavailable):|^IoError:(service|ipc)/.test(
        errorNoticeKey.current,
      )
    ) {
      errorNoticeKey.current = "";
    }
    observedError.current = key;
  }, [data?.snapshot.error, data?.snapshot.engine_pid, setError]);
  const [draft, setDraft] = useState<Preferences | null>(null),
    [search, setSearch] = useState(""),
    [group, setGroup] = useState(""),
    [sort, setSort] = useState("name");
  useEffect(() => {
    if (!notice || bannerPaused) return;
    const timer = setTimeout(() => setNotice(""), 5000);
    return () => clearTimeout(timer);
  }, [notice, bannerPaused]);
  useEffect(() => {
    if (!report || bannerPaused) return;
    const timer = setTimeout(() => setReport(null), 12000);
    return () => clearTimeout(timer);
  }, [report, bannerPaused]);
  useEffect(() => {
    if (!error || bannerPaused) return;
    const timer = setTimeout(() => setError(null), 12000);
    return () => clearTimeout(timer);
  }, [error, bannerPaused]);
  useEffect(() => {
    if (!isTauri()) return;
    const record = (action: string, target: string) => {
      void invoke("ui_event", { action, target }).catch(() => {});
    };
    const failure = () => record("error", "runtime");
    window.addEventListener("error", failure);
    window.addEventListener("unhandledrejection", failure);
    return () => {
      window.removeEventListener("error", failure);
      window.removeEventListener("unhandledrejection", failure);
    };
  }, []);
  useEffect(() => {
    setBannerPaused(false);
  }, [notice, error, report]);
  const [chooseNetwork, setChooseNetwork] = useState(false);
  const [connectTun, setConnectTun] = useState(true);
  const [connectProxy, setConnectProxy] = useState(false);
  const [serverPicker, setServerPicker] = useState(false);
  const [pendingNavigation, setPendingNavigation] = useState<
    Page | "close" | "quit" | null
  >(null);
  const [edited, setEdited] = useState(false);
  const editedRef = useRef(false);
  editedRef.current = edited;
  const dirty =
    !!data && !!draft && hasUnsavedChanges(data.preferences, draft, edited);
  const dirtyRef = useRef(false);
  dirtyRef.current = dirty;
  const closeRef = useRef<() => void>(() => {});
  const trayRef = useRef<(id: string) => void>(() => {});
  useEffect(() => {
    if (!isTauri()) return;
    const off = getCurrentWindow().onCloseRequested((e) => {
      e.preventDefault();
      closeRef.current();
    });
    return () => {
      off.then((f) => f());
    };
  }, []);
  useEffect(() => {
    if (!isTauri()) return;
    const a = listen<string>("tray-action", (e) => trayRef.current(e.payload));
    const b = listen<Snapshot>("connection-snapshot", (e) => {
      const id = e.payload.engine_pid ? e.payload.active?.id : null;
      setData((d) =>
        d
          ? {
              ...d,
              snapshot: e.payload,
              preferences: id
                ? { ...d.preferences, selected_profile: id }
                : d.preferences,
            }
          : d,
      );
      if (id) {
        setSelected(id);
        setDraft((d) => (d ? { ...d, selected_profile: id } : d));
      }
    });
    return () => {
      a.then((f) => f());
      b.then((f) => f());
    };
  }, []);
  const navigate = (next: Page) => {
    if (dirty) setPendingNavigation(next);
    else setPage(next);
  };
  const lang = draft?.language || data?.preferences.language || "ru";
  const t = (key: Label) => (lang === "ru" ? ru[key] : en[key]);
  const openClientUpdate = () => {
    setShowUpdateNotice(false);
    setSettingsTab("about");
    navigate("settings");
    window.scrollTo({ top: 0, behavior: "smooth" });
  };
  const reload = useCallback(async (preserveDraft = false) => {
    const b = await invoke<Bootstrap>("bootstrap");
    setData(b);
    if (!preserveDraft || !editedRef.current) {
      setDraft(b.preferences);
      setEdited(false);
      editedRef.current = false;
    } else
      setDraft((p) =>
        p
          ? {
              ...p,
              selected_profile: b.preferences.selected_profile,
              window: b.preferences.window,
            }
          : b.preferences,
      );
    setSelected(
      (s) =>
        b.snapshot.active?.id ||
        b.preferences.selected_profile ||
        (b.profiles.some((p) => p.id === s) ? s : b.profiles[0]?.id || ""),
    );
    return b;
  }, []);
  useEffect(() => {
    if (!isTauri()) {
      setFatal(ru.nativeOnly);
      return;
    }
    reload().catch((e) => setFatal(e?.message || "Native core unavailable"));
  }, [reload]);
  useEffect(() => {
    if (!isTauri()) return;
    return pollClientUpdates(
      () => invoke<ClientRelease | null>("check_client_update"),
      receiveClientRelease,
    );
  }, [receiveClientRelease]);
  useEffect(() => {
    if (!isTauri()) return;
    const off = listen<ClientRelease | null>("client-release", (e) =>
      receiveClientRelease(e.payload),
    );
    return () => {
      void off.then((f) => f());
    };
  }, [receiveClientRelease]);
  useEffect(() => {
    if (!isTauri()) return;
    const off = listen(
      "runtime-prepared",
      () => void reload(true).catch((e) => setError(e)),
    );
    return () => {
      void off.then((f) => f());
    };
  }, [reload, setError]);
  useEffect(() => {
    if (!data?.runtime_preparing) return;
    const timer = setTimeout(
      () => void reload(true).catch((e) => setError(e)),
      1500,
    );
    return () => clearTimeout(timer);
  }, [data, reload, setError]);
  const [smokeStep, setSmokeStep] = useState(0);
  const smokeViews = useRef<string[]>([]);
  useEffect(() => {
    if (!data?.startup_smoke || data.runtime_preparing || !draft) return;
    const steps: [Page, string?][] = [
      ["dashboard"],
      ["connections"],
      ["routing"],
      ["activity"],
      ["networkSettings", "general"],
      ["networkSettings", "tls"],
      ["networkSettings", "dns"],
      ["networkSettings", "rules"],
      ["networkSettings", "playground"],
      ["settings", "appearance"],
      ["settings", "ping"],
      ["settings", "cores"],
      ["settings", "about"],
      ["settings", "data"],
      ["connections", "import"],
      ["dashboard", "picker"],
    ];
    if (smokeStep >= steps.length) {
      void invoke("startup_smoke_ready", { views: smokeViews.current }).catch(
        (e) => setFatal(String(e?.message || e)),
      );
      return;
    }
    const [next, tab] = steps[smokeStep];
    setPage(next);
    setModal(tab === "import");
    setServerPicker(tab === "picker");
    if (next === "networkSettings") setNetworkTab(tab!);
    if (next === "settings") setSettingsTab(tab!);
    const timer = setTimeout(() => {
      const controls = [...document.querySelectorAll("input,select,textarea")];
      const missing = controls.some(
        (c) =>
          !c.closest("label")?.querySelector(".setting-help") &&
          !c.closest(".control-with-help")?.querySelector(".setting-help") &&
          !c.closest(".application-row"),
      );
      if (missing) {
        void invoke("startup_smoke_failed", {
          message: "A field has no explanation",
        });
        setFatal("Native UI check: a field has no explanation");
        return;
      }
      try {
        checkLayout();
      } catch (e) {
        void invoke("startup_smoke_failed", { message: String(e) });
        setFatal("Native layout check: " + String(e));
        return;
      }
      smokeViews.current.push(next + (tab ? ":" + tab : ""));
      setSmokeStep((v) => v + 1);
    }, 150);
    return () => clearTimeout(timer);
  }, [data?.startup_smoke, data?.runtime_preparing, draft, smokeStep]);
  useEffect(() => {
    if (!isTauri()) return;
    const off = listen(
      "subscriptions-updated",
      () => void reload(true).catch((e) => setError(e)),
    );
    return () => {
      void off.then((f) => f());
    };
  }, [reload, setError]);
  const didAutoStart = useRef(false);
  const manualDisconnect = useRef(false);
  const [reconnectAttempts, setReconnectAttempts] = useState(0);
  useEffect(() => {
    if (!data || data.runtime_preparing || didAutoStart.current) return;
    didAutoStart.current = true;
    if (
      data.preferences.auto_reconnect &&
      data.preferences.selected_profile &&
      data.profiles.some((p) => p.id === data.preferences.selected_profile) &&
      data.engine_ready &&
      !data.snapshot.engine_pid
    ) {
      if (!data.preferences.tun && !data.preferences.system_proxy)
        setChooseNetwork(true);
      else void action("connect", { id: data.preferences.selected_profile });
    }
  }, [data]);
  useEffect(() => {
    if (data?.snapshot.state === "connected") {
      setReconnectAttempts(0);
      return;
    }
    if (
      manualDisconnect.current ||
      !data?.preferences.auto_reconnect ||
      data.preferences.auto_failover ||
      data.snapshot.state !== "error" ||
      busy ||
      reconnectAttempts >= 3 ||
      !data.preferences.selected_profile
    )
      return;
    const timer = setTimeout(() => {
      setReconnectAttempts((v) => v + 1);
      if (!data.preferences.tun && !data.preferences.system_proxy)
        setChooseNetwork(true);
      else void action("connect", { id: data.preferences.selected_profile });
    }, [5000, 15000, 45000][reconnectAttempts]);
    return () => clearTimeout(timer);
  }, [
    data?.snapshot.state,
    data?.preferences.auto_reconnect,
    busy,
    reconnectAttempts,
  ]);
  useEffect(() => {
    if (!isTauri()) return;
    const off = listen<Snapshot["state"]>("connection-state", (event) =>
      setData((d) =>
        d ? { ...d, snapshot: { ...d.snapshot, state: event.payload } } : d,
      ),
    );
    return () => {
      off.then((f) => f());
    };
  }, []);
  useEffect(() => {
    if (!data) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (!busy && !document.hidden) {
        try {
          const s = await invoke<Snapshot>("snapshot");
          if (!disposed) {
            setData((d) => (d ? { ...d, snapshot: s } : d));
            if (s.metrics)
              setHistory((h) => [
                ...h.slice(-59),
                { up: s.metrics!.upload_bps, down: s.metrics!.download_bps },
              ]);
          }
        } catch (e) {
          if (!disposed) setError(e as NativeError);
        }
      }
      if (!disposed) timer = setTimeout(poll, 1500);
    };
    timer = setTimeout(poll, 1500);
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [!!data, busy]);
  useEffect(() => {
    const p = draft;
    if (!p) return;
    const root = document.documentElement;
    const media = matchMedia("(prefers-color-scheme: light)");
    const applyTheme = () => {
      root.dataset.theme =
        p.theme === "light" ||
        (p.theme === "system" && media.matches) ||
        (p.theme === "custom" && surfaceColors(p.surface).light)
          ? "light"
          : "dark";
    };
    applyTheme();
    media.addEventListener("change", applyTheme);
    root.dataset.density = p.density;
    root.style.setProperty("--accent", p.accent);
    root.style.setProperty("--radius", `${p.radius}px`);
    root.style.setProperty("--motion", p.motion ? "180ms" : "0ms");
    const tokens = surfaceColors(p.surface);
    for (const key of [
      "--surface",
      "--bg",
      "--card",
      "--border",
      "--text",
      "--muted",
    ])
      root.style.removeProperty(key);
    if (p.theme === "custom") {
      for (const [key, value] of Object.entries(tokens))
        if (key.startsWith("--")) root.style.setProperty(key, String(value));
      root.dataset.theme = tokens.light ? "light" : "dark";
    }
    root.lang = p.language;
    setHelpLanguage(p.language);
    return () => media.removeEventListener("change", applyTheme);
  }, [draft]);
  const action = async <T,>(
    command: string,
    args?: Record<string, unknown>,
    refresh = true,
  ): Promise<T | undefined> => {
    if (command === "disconnect") manualDisconnect.current = true;
    if (command === "connect") manualDisconnect.current = false;
    setBusy(true);
    errorNoticeKey.current = "";
    setError(null);
    setNotice("");
    try {
      const result = await invoke<T>(command, args);
      if (command === "delete_local_data") clearCatalog();
      if (refresh)
        await reload(
          command !== "save_preferences" &&
            command !== "restore_data" &&
            command !== "delete_local_data",
        );
      if (command === "disconnect")
        setDraft((d) => (d ? { ...d, tun: false, system_proxy: false } : d));
      return result;
    } catch (e) {
      const err = e as NativeError;
      setError({
        code: err.code || "NativeError",
        stage: err.stage || command,
        message: err.message || String(e),
      });
      if (refresh) await reload(true).catch(() => {});
    } finally {
      setBusy(false);
    }
  };
  const imported = async (command: string, args?: Record<string, unknown>) => {
    const r = await action<ImportReport | null>(command, args);
    if (r) {
      setReport(r);
      setModal(false);
      setPage("connections");
      return true;
    }
    return false;
  };
  const save = async () => {
    if (draft) {
      const result = await action<boolean>("save_preferences", {
        preferences: draft,
      });
      if (result === true) {
        setNotice(t("saved"));
        return true;
      }
    }
    return false;
  };
  const uiQueue = useRef<Promise<unknown>>(Promise.resolve());
  const quickUi = async (
    patch: Partial<
      Pick<Preferences, "app_view" | "server_view" | "favorites">
    > & { favorite_id?: string },
  ) => {
    try {
      const request = uiQueue.current
        .catch(() => {})
        .then(() => invoke<Preferences>("quick_ui", { patch }));
      uiQueue.current = request;
      const saved = await request;
      setData((d) =>
        d ? { ...d, preferences: mergeQuickUi(d.preferences, saved) } : d,
      );
      setDraft((d) => (d ? mergeQuickUi(d, saved) : d));
    } catch (e) {
      setError(e as NativeError);
    }
  };
  const toggleFavorite = (id: string) => void quickUi({ favorite_id: id });
  const requestConnect = () => {
    manualDisconnect.current = false;
    if (!data?.preferences.tun && !data?.preferences.system_proxy)
      setChooseNetwork(true);
    else void action("connect", { id: selected });
  };
  const field = <K extends keyof Preferences>(
    key: K,
    value: Preferences[K],
  ) => {
    setEdited(true);
    editedRef.current = true;
    setDraft((p) => (p ? { ...p, [key]: value } : p));
  };
  const finishNavigation = async (next: Page | "close" | "quit") => {
    if (
      next === "quit" ||
      (next === "close" && !data?.preferences.close_to_tray)
    ) {
      await action("quit_application", {}, false);
    } else if (next === "close") await action("hide_to_tray", {}, false);
    else setPage(next);
  };
  closeRef.current = () => {
    if (dirtyRef.current) setPendingNavigation("close");
    else void finishNavigation("close");
  };
  trayRef.current = (id) => {
    if (busy) return;
    if (id === "settings") navigate("settings");
    else if (id === "clipboard") void imported("import_clipboard");
    else if (id === "qr") void imported("import_file", { qr: true });
    else if (id === "connection") {
      if (data?.snapshot.engine_pid) void action("disconnect");
      else requestConnect();
    } else if (id === "quit") {
      if (dirtyRef.current) setPendingNavigation("quit");
      else void finishNavigation("quit");
    }
  };
  const quickNetwork = async (values: {
    tun?: boolean;
    systemProxy?: boolean;
    routeMode?: Preferences["route_mode"];
    transportEngine?: Preferences["transport_engine"];
  }) => {
    const result = await action<boolean>("quick_network", values);
    if (result) {
      const b = await reload(true);
      setDraft((d) =>
        d
          ? {
              ...mergeQuickNetwork(d, b.preferences),
              ...(values.transportEngine
                ? { transport_engine: b.preferences.transport_engine }
                : {}),
            }
          : d,
      );
    }
    return result === true;
  };
  const removeProfile = (p: Profile) => action("delete_profile", { id: p.id });
  const selectProfile = async (id: string) => {
    const result = await action(
      data?.snapshot.engine_pid ? "connect" : "select_profile",
      { id },
    );
    setServerPicker(false);
    if (result !== undefined) {
      setSelected(id);
      setPublicIp("");
    }
  };
  const snapshot = data?.snapshot;
  const active = !!snapshot?.engine_pid;
  const unconfirmed = connectionUnconfirmed(snapshot);
  const m = snapshot?.metrics;
  const current = data?.profiles.find((p) => p.id === selected);
  const source = (p: Profile) =>
    data?.subscriptions.find((s) => s.id === p.subscription_id)?.name ||
    t("manual");
  const filtered =
    data?.profiles
      .filter(
        (p) =>
          (!group ||
            p.subscription_id === group ||
            (group === "manual" && !p.subscription_id)) &&
          `${p.name} ${p.host} ${p.protocol}`
            .toLowerCase()
            .includes(search.toLowerCase()),
      )
      .sort((a, b) =>
        sort === "latency"
          ? (a.latency_ms ?? Infinity) - (b.latency_ms ?? Infinity)
          : a.name.localeCompare(b.name),
      ) || [];
  if (!data || !draft)
    return (
      <main className="loading">
        <div className="brand">
          <Activity />
          netpulse<span>.</span>
        </div>
        {fatal ? (
          <div role="alert" className="banner error">
            <AlertCircle />
            <span className="selectable">{fatal}</span>
            <button
              onClick={() => {
                setFatal("");
                void reload().catch((e) => setFatal(e?.message || String(e)));
              }}
            >
              {lang === "ru" ? "Повторить" : "Retry"}
            </button>
          </div>
        ) : (
          <LoaderCircle className="spin" />
        )}
      </main>
    );
  const state = snapshot?.state || "disconnected";
  const stateLabel = unconfirmed
    ? lang === "ru"
      ? "Связь со службой не подтверждена"
      : "Service connection unconfirmed"
    : t(state);
  const max = Math.max(1, ...history.flatMap((p) => [p.up, p.down]));
  const graph = (key: "up" | "down") =>
    history
      .map((v, i) => `${(i * 800) / 59},${154 - (v[key] / max) * 134}`)
      .join(" ");
  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <Activity />
          netpulse<span>.</span>
        </div>
        <span className="eyebrow">DESKTOP 3.0</span>
        <nav aria-label="Navigation">
          {(
            [
              "dashboard",
              "connections",
              "routing",
              "networkSettings",
              "activity",
              "settings",
            ] as Page[]
          ).map((p) => {
            const Icon = icons[p];
            return (
              <button
                key={p}
                className={`nav ${page === p ? "selected" : ""}`}
                aria-current={page === p ? "page" : undefined}
                onClick={() => navigate(p)}
              >
                <Icon size={19} />
                {t(p)}
                {page === p && <ChevronRight size={15} />}
              </button>
            );
          })}
        </nav>
        <div className="sidebar-bottom">
          {clientRelease && (
            <button className="client-update-badge" onClick={openClientUpdate}>
              <Download size={15} />
              <span>
                {lang === "ru" ? "Доступно обновление" : "Update available"}
              </span>
            </button>
          )}
          <div className="local">
            <Shield size={16} />
            {t("local")}
          </div>
          <span className="version">{data.app_version}</span>
        </div>
      </aside>
      <main className="main">
        <header>
          <div>
            <div className="eyebrow">NETPULSE / {t(page).toUpperCase()}</div>
            <h1>
              {t(
                page === "dashboard"
                  ? "network"
                  : page === "routing"
                    ? "routeTitle"
                    : page === "activity"
                      ? "activityTitle"
                      : page === "networkSettings"
                        ? "networkSettings"
                        : page === "settings"
                          ? "settingsTitle"
                          : "connections",
              )}
            </h1>
            <p>
              {t(
                page === "dashboard"
                  ? "intro"
                  : page === "routing"
                    ? "routeIntro"
                    : page === "activity"
                      ? "activityIntro"
                      : page === "networkSettings"
                        ? "networkIntro"
                        : page === "settings"
                          ? "settingsIntro"
                          : "keyHint",
              )}
            </p>
          </div>
          <span className="alpha">
            <span
              className={`dot ${data.app_version.includes("-") ? "amber" : "green"}`}
            />
            {data.app_version.includes("-")
              ? t("alpha")
              : `NetPulse ${data.app_version}`}
          </span>
        </header>
        <div aria-live="polite">
          {data.runtime_preparing && (
            <div className="banner">
              <LoaderCircle className="spin" size={18} />
              {lang === "ru"
                ? "Проверка ядер… Настройки и журналы уже доступны."
                : "Checking runtimes… Settings and logs are available."}
            </div>
          )}
          {data.runtime_error && (
            <div className="banner error">
              <AlertCircle size={20} />
              <div className="grow selectable">
                <strong>
                  {lang === "ru" ? "Ядро недоступно" : "Runtime unavailable"}
                </strong>
                <p>{data.runtime_error.message}</p>
              </div>
              <button
                onClick={() => {
                  setSettingsTab("cores");
                  navigate("settings");
                }}
              >
                {lang === "ru" ? "Ядра и восстановление" : "Cores and recovery"}
              </button>
            </div>
          )}
          {clientRelease && showUpdateNotice && (
            <div className="banner client-update-notice" role="status">
              <Download size={20} />
              <div className="grow">
                <strong>
                  {lang === "ru"
                    ? "Доступна новая версия NetPulse"
                    : "A new NetPulse version is available"}{" "}
                  · {clientRelease.tag}
                </strong>
                <p>
                  {lang === "ru"
                    ? "Посмотреть и скачать обновление можно в «О NetPulse»."
                    : "View and download the update in About NetPulse."}
                </p>
              </div>
              <button onClick={openClientUpdate}>
                {lang === "ru" ? "Обновить" : "Update"}
              </button>
              <button
                className="icon"
                aria-label={t("close")}
                onClick={() => setShowUpdateNotice(false)}
              >
                <X size={18} />
              </button>
            </div>
          )}
          {busy && (
            <div className="working">
              <LoaderCircle className="spin" size={16} />
              {t("busy")}
            </div>
          )}
          {error && (
            <div
              className="banner error"
              role="alert"
              onMouseEnter={() => setBannerPaused(true)}
              onMouseLeave={() => setBannerPaused(false)}
            >
              <AlertCircle size={20} />
              <div>
                <strong>
                  {error.code} · {error.stage}
                </strong>
                <p>{error.message}</p>
              </div>
              {[
                "ServiceTimeout",
                "IpcTimeout",
                "ServiceBusy",
                "ServiceMonitorUnavailable",
              ].includes(error.code) && (
                <button
                  disabled={busy || !data.engine_ready}
                  onClick={() =>
                    void action("manage_service", {
                      install: true,
                      upgrade: true,
                    })
                  }
                >
                  {lang === "ru"
                    ? "Восстановить службу (UAC)"
                    : "Recover service (UAC)"}
                </button>
              )}
              <button
                className="icon"
                onClick={() => setError(null)}
                aria-label={t("close")}
              >
                <X />
              </button>
            </div>
          )}
          {notice && (
            <div
              className="banner success"
              role="status"
              onMouseEnter={() => setBannerPaused(true)}
              onMouseLeave={() => setBannerPaused(false)}
            >
              <Check size={18} />
              {notice}
              <button
                className="icon"
                aria-label={t("close")}
                onClick={() => setNotice("")}
              >
                <X size={16} />
              </button>
            </div>
          )}
          {report && (
            <div
              onMouseEnter={() => setBannerPaused(true)}
              onMouseLeave={() => setBannerPaused(false)}
              className={`banner ${report.issues.length ? "warning" : "success"}`}
            >
              <Check size={20} />
              <div>
                <strong>
                  {t("imported")}: {report.imported} · {report.format}
                </strong>
                {report.issues.length > 0 && (
                  <details>
                    <summary>
                      {t("issues")}: {report.issues.length}
                    </summary>
                    {report.issues.map((i) => (
                      <p key={i.index}>
                        #{i.index} · {i.message}
                      </p>
                    ))}
                  </details>
                )}
              </div>
              <button
                className="icon"
                onClick={() => setReport(null)}
                aria-label={t("close")}
              >
                <X />
              </button>
            </div>
          )}
        </div>
        {page === "dashboard" && (
          <>
            <section
              className={`connection-hero ${unconfirmed ? "unconfirmed" : active ? "online" : ""}`}
            >
              <div className="hero-top">
                <span
                  className={`status ${unconfirmed ? "unconfirmed" : active ? "green" : ""}`}
                >
                  <span className="dot" />
                  {stateLabel}
                </span>
                <Wifi className="hero-wifi" size={24} />
              </div>
              <div className="hero-body">
                <div>
                  <span className="muted small">{t("activeServer")}</span>
                  <h2>
                    {current && <CountryFlag name={current.name} />}{" "}
                    {current ? serverName(current.name) : t("noProfiles")}
                  </h2>
                  <p>
                    {current
                      ? `${current.protocol.toUpperCase()} · ${current.host}:${current.port}`
                      : t("noProfilesHelp")}
                  </p>
                  {current && <span className="chip">{source(current)}</span>}
                </div>
                <button
                  className={`power ${active ? "on" : ""}`}
                  disabled={
                    busy || (!active && (!selected || !data.engine_ready))
                  }
                  onClick={async () => {
                    setPublicIp("");
                    setHistory([]);
                    if (active) await action("disconnect");
                    else requestConnect();
                  }}
                >
                  <Power size={28} />
                  {t(active ? "disconnect" : "connect")}
                </button>
              </div>
              <div className="hero-footer">
                <button
                  className="server-trigger"
                  disabled={busy}
                  onClick={() => setServerPicker(true)}
                >
                  <Server size={18} />
                  <span>
                    {current && <CountryFlag name={current.name} />}{" "}
                    {current ? serverName(current.name) : t("chooseServer")}
                  </span>
                  <ChevronRight size={18} />
                </button>
                <CoreSelector
                  lang={lang}
                  value={data.preferences.transport_engine}
                  disabled={busy || data.runtime_preparing}
                  change={(value) =>
                    void quickNetwork({ transportEngine: value })
                  }
                />
                <ActiveLatency
                  activeId={snapshot?.active?.id}
                  enginePid={snapshot?.engine_pid}
                  method={data.preferences.ping_method}
                  lang={lang}
                />
              </div>
            </section>
            <section className="card">
              <div className="section-top">
                <h3>{t("modes")}</h3>
                <span className="chip">{t(draft.route_mode)}</span>
              </div>
              <div className="two-columns mode-cards">
                <div>
                  <div className="row">
                    <strong>TUN</strong>
                    <span className="badge">
                      {lang === "ru"
                        ? "Применяется сразу"
                        : "Applies immediately"}
                    </span>
                    <span className="control-with-help">
                      <input
                        type="checkbox"
                        checked={draft.tun}
                        disabled={busy}
                        onChange={(e) =>
                          void quickNetwork({ tun: e.target.checked })
                        }
                        aria-label="TUN"
                      />
                      <Help topic="tun" />
                    </span>
                  </div>
                  <p>{t("tunHelp")}</p>
                  {!draft.tun &&
                    ["whitelist", "blacklist"].includes(draft.route_mode) && (
                      <p className="small">
                        {lang === "ru"
                          ? "Для правил приложений включите TUN; System Proxy охватывает только приложения, поддерживающие прокси Windows."
                          : "Enable TUN for process rules; System Proxy covers only applications using Windows proxy settings."}
                      </p>
                    )}
                  <small>
                    {lang === "ru"
                      ? "Фактически включён: "
                      : "Actually active: "}
                    {snapshot?.tun_active ? "✓" : "—"}
                  </small>
                </div>
                <div>
                  <div className="row">
                    <strong>System Proxy</strong>
                    <span className="badge">
                      {lang === "ru"
                        ? "Применяется сразу"
                        : "Applies immediately"}
                    </span>
                    <span className="control-with-help">
                      <input
                        type="checkbox"
                        checked={draft.system_proxy}
                        disabled={busy}
                        onChange={(e) =>
                          void quickNetwork({ systemProxy: e.target.checked })
                        }
                        aria-label="System Proxy"
                      />
                      <Help topic="proxy" />
                    </span>
                  </div>
                  <small>
                    {lang === "ru"
                      ? "Фактически включён: "
                      : "Actually active: "}
                    {snapshot?.system_proxy_active ? "✓" : "—"}
                  </small>
                  <p>{t("proxyHelp")}</p>
                </div>
              </div>
            </section>
            <div className="segments routing-modes">
              {(["global", "whitelist", "blacklist"] as const).map((mode) => (
                <button
                  key={mode}
                  className={draft.route_mode === mode ? "selected" : ""}
                  disabled={busy}
                  onClick={() => void quickNetwork({ routeMode: mode })}
                >
                  {t(mode)}
                </button>
              ))}
            </div>
            <section className="metric-grid">
              <Metric
                icon={<ArrowDown />}
                label={t("download")}
                value={m ? `${bytes(m.download_bps)}/s` : "—"}
                sub={`${t("total")}: ${m ? bytes(m.download_total) : "—"}`}
                color="purple"
              />
              <Metric
                icon={<ArrowUp />}
                label={t("upload")}
                value={m ? `${bytes(m.upload_bps)}/s` : "—"}
                sub={`${t("total")}: ${m ? bytes(m.upload_total) : "—"}`}
                color="green"
              />
              <Metric
                icon={<Link />}
                label={t("activeConnections")}
                value={m ? String(m.connections) : "—"}
                sub={
                  active
                    ? snapshot?.metrics_error
                      ? lang === "ru"
                        ? "Статистика временно недоступна"
                        : "Statistics temporarily unavailable"
                      : lang === "ru"
                        ? "Чтение не чаще раза в 5 с"
                        : "Read at most every 5 sec"
                    : t("noConnection")
                }
                color="blue"
              />
            </section>
            <div className="two-columns">
              <section className="card chart">
                <div className="section-top">
                  <div>
                    <h3>{t("live")}</h3>
                    <p>{t("liveHelp")}</p>
                  </div>
                  <span className="chip">{active ? "LIVE" : "—"}</span>
                </div>
                <div className="graph">
                  {history.length > 1 ? (
                    <svg
                      viewBox="0 0 800 170"
                      preserveAspectRatio="none"
                      role="img"
                      aria-label={t("live")}
                    >
                      <path
                        className="grid-line"
                        d="M0 20H800 M0 65H800 M0 110H800 M0 155H800"
                      />
                      <polyline className="down-line" points={graph("down")} />
                      <polyline className="up-line" points={graph("up")} />
                    </svg>
                  ) : (
                    <span>{t("notMeasured")}</span>
                  )}
                </div>
                <div className="legend">
                  <span>
                    <i className="purple" />
                    {t("download")}
                  </span>
                  <span>
                    <i className="green" />
                    {t("upload")}
                  </span>
                </div>
              </section>
              <section className="card">
                <Globe className="card-icon" />
                <h3>{t("publicIp")}</h3>
                <p>{lang === "ru" ? "Через VPN" : "Through VPN"}</p>
                <div className="ip-row">
                  <span className="ip selectable">{publicIp || "—"}</span>
                  <button
                    className="icon"
                    disabled={!publicIp}
                    aria-label={
                      lang === "ru" ? "Копировать IP VPN" : "Copy VPN IP"
                    }
                    onClick={() => action("copy_ip", { ip: publicIp }, false)}
                  >
                    <Clipboard size={16} />
                  </button>
                </div>
                <p>
                  {lang === "ru"
                    ? "Без NetPulse (внешний IP провайдера)"
                    : "Without NetPulse (ISP public IP)"}
                </p>
                <div className="ip-row">
                  <span className="ip selectable">{directIp || "—"}</span>
                  <button
                    className="icon"
                    disabled={!directIp}
                    aria-label={
                      lang === "ru" ? "Копировать обычный IP" : "Copy direct IP"
                    }
                    onClick={() => action("copy_ip", { ip: directIp }, false)}
                  >
                    <Clipboard size={16} />
                  </button>
                </div>
                <p className="small muted">
                  {lang === "ru"
                    ? "Проверка через api.ipify.org. Другой системный VPN может влиять на маршрут без NetPulse."
                    : "Checked via api.ipify.org. Other system VPNs may affect the direct route."}
                </p>
                <button
                  disabled={busy}
                  onClick={async () => {
                    const ips = await action<{
                      vpn: string | null;
                      direct: string | null;
                      vpn_error: NativeError | null;
                      direct_error: NativeError | null;
                    }>("public_ips", {}, false);
                    if (ips) {
                      setPublicIp(ips.vpn || "");
                      setDirectIp(ips.direct || "");
                      if (ips.direct_error || ips.vpn_error)
                        setError(ips.direct_error || ips.vpn_error);
                    }
                  }}
                >
                  <RefreshCw size={15} />
                  {t("checkIp")}
                </button>
              </section>
            </div>
            <section className="card engine-card">
              <div>
                <h3>
                  {t("localProxy")} · 127.0.0.1:{data.preferences.proxy_port}
                </h3>
                <p>{t("localProxyHelp")}</p>
              </div>
              <button
                disabled={!active || busy}
                onClick={async () => {
                  const copied = await action<boolean>(
                    "copy_proxy_endpoint",
                    {},
                    false,
                  );
                  if (copied) setNotice(t("proxyCopied"));
                }}
              >
                <Clipboard size={16} />
                {t("copyProxy")}
              </button>
            </section>
            {!data.engine_ready && (
              <section className="card engine-card">
                <div>
                  <h3>{t("engine")}</h3>
                  <p>{t("engineNote")}</p>
                </div>
                <button
                  className="primary"
                  disabled={busy}
                  onClick={() =>
                    action("activate_core_bundle", { rollback: false })
                  }
                >
                  <Download size={17} />
                  {t("installEngine")}
                </button>
              </section>
            )}
          </>
        )}
        {page === "connections" && (
          <div className="connections-workspace">
            <div className="connections-content">
              <div className="toolbar">
                <div className="search">
                  <Search size={17} />
                  <span className="control-with-help">
                    <input
                      aria-label={t("search")}
                      placeholder={t("search")}
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                    />
                    <Help topic="filter" />
                  </span>
                </div>
                <span className="control-with-help">
                  <select
                    aria-label={t("source")}
                    value={group}
                    onChange={(e) => setGroup(e.target.value)}
                  >
                    <option value="">{t("allGroups")}</option>
                    <option value="manual">{t("manual")}</option>
                    {data.subscriptions.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.name}
                      </option>
                    ))}
                  </select>
                  <Help topic="source" />
                </span>
                <span className="control-with-help">
                  <select
                    value={sort}
                    onChange={(e) => setSort(e.target.value)}
                    aria-label={t("sortName")}
                  >
                    <option value="name">{t("sortName")}</option>
                    <option value="latency">{t("sortLatency")}</option>
                  </select>
                  <Help topic="sort" />
                </span>
                <button className="primary" onClick={() => setModal(true)}>
                  <Plus size={17} />
                  {t("add")}
                </button>
              </div>
              <div className="section-top">
                <h3>
                  {t("profiles")}{" "}
                  <span className="count">{filtered.length}</span>
                </h3>
              </div>
              <ServerList
                profiles={filtered}
                sourceFilter={group}
                query={search}
                subscriptionAction={(command, id) => {
                  if (command === "update_subscription")
                    void imported(command, { id });
                  else void action(command, { id });
                }}
                subscriptions={data.subscriptions}
                selected={selected}
                favorites={draft.favorites}
                toggleFavorite={toggleFavorite}
                pingMethod={data.preferences.ping_method}
                view={draft.server_view}
                changeView={(v) => void quickUi({ server_view: v })}
                choose={selectProfile}
                remove={removeProfile}
                activeId={snapshot?.active?.id}
                busy={busy || data.runtime_preparing}
                lang={lang}
                error={setError}
                refresh={() => reload(true)}
              />
            </div>
          </div>
        )}
        {page === "routing" && (
          <>
            <section className="card">
              <h3>{t("modes")}</h3>
              <div className="segments routing-modes">
                {(["global", "whitelist", "blacklist"] as const).map((mode) => (
                  <button
                    key={mode}
                    className={draft.route_mode === mode ? "selected" : ""}
                    disabled={busy}
                    onClick={() => void quickNetwork({ routeMode: mode })}
                  >
                    {t(mode)}
                  </button>
                ))}
              </div>
              <p>{t("routeRuleHelp")}</p>
              {!draft.tun && (
                <p className="warning">
                  {lang === "ru"
                    ? "Один список приложений для всех режимов. В белом списке выбранные приложения идут через VPN; в чёрном — в обход VPN. В глобальном режиме VPN используется всеми приложениями. Для правил по EXE включите TUN."
                    : "One shared app list. Whitelist sends selected apps through VPN; blacklist bypasses VPN for them. Global routes all apps through VPN. Enable TUN for EXE rules."}
                </p>
              )}
            </section>
            <ApplicationsPanel
              smoke={data.startup_smoke}
              draft={draft}
              changeView={(v) => void quickUi({ app_view: v })}
              change={(v) => {
                setEdited(true);
                editedRef.current = true;
                setDraft(v);
              }}
              busy={busy}
              lang={lang}
              scan={() => action<Application[]>("scan_applications", {}, false)}
              choose={() => action<Application | null>("choose_exe", {}, false)}
            />
            <RoutingSettings
              draft={draft}
              lang={lang}
              busy={busy}
              action={action}
              change={(v) => {
                setEdited(true);
                editedRef.current = true;
                setDraft(v);
              }}
            />
          </>
        )}
        {page === "activity" && <FileLogs lang={lang} />}
        {page === "networkSettings" && (
          <>
            {" "}
            <div>
              <div
                className="settings-tabs network-tabs"
                role="tablist"
                aria-label={lang === "ru" ? "Разделы сети" : "Network sections"}
              >
                {[
                  ["general", "Общее", "General"],
                  ["tls", "TLS и транспорт", "TLS and transport"],
                  ["dns", "DNS", "DNS"],
                  ["rules", "Правила", "Rules"],
                  ["playground", "Playground", "Playground"],
                ].map(([key, ru, en]) => (
                  <button
                    key={key}
                    role="tab"
                    aria-selected={networkTab === key}
                    aria-pressed={networkTab === key}
                    onClick={() => setNetworkTab(key)}
                  >
                    {lang === "ru" ? ru : en}
                  </button>
                ))}
              </div>
              <div hidden={networkTab !== "tls"}>
                <ProfileNetwork
                  draft={draft}
                  lang={lang}
                  change={(v) => {
                    setEdited(true);
                    editedRef.current = true;
                    setDraft(v);
                  }}
                />
                <TransportSettings
                  draft={draft}
                  lang={lang}
                  change={(v) => {
                    setEdited(true);
                    editedRef.current = true;
                    setDraft(v);
                  }}
                />
              </div>
              <div hidden={networkTab !== "dns"}>
                <DnsSettings
                  draft={draft}
                  lang={lang}
                  change={(v) => {
                    setEdited(true);
                    editedRef.current = true;
                    setDraft(v);
                  }}
                />
              </div>
              <div hidden={networkTab !== "rules"}>
                <AdvancedRules
                  draft={draft}
                  lang={lang}
                  change={(v) => {
                    setEdited(true);
                    editedRef.current = true;
                    setDraft(v);
                  }}
                />
              </div>
              <div hidden={networkTab !== "playground"}>
                <Playground
                  smoke={data.startup_smoke}
                  lang={lang}
                  visible={
                    page === "networkSettings" && networkTab === "playground"
                  }
                />
              </div>
              <div hidden={networkTab !== "general"}>
                <MtuAnalysis
                  lang={lang}
                  busy={busy}
                  error={setError}
                  apply={(mtu) => {
                    field("mtu", mtu);
                    field("network_preset", "custom");
                  }}
                />
                <NetworkSettings
                  draft={draft}
                  change={(v) => {
                    setEdited(true);
                    editedRef.current = true;
                    setDraft(v);
                  }}
                  lang={lang}
                />
              </div>
            </div>
            <div hidden={networkTab !== "general"}>
              <section className="card">
                <h3>{t("native")}</h3>
                <div className="engine-status section-top">
                  <span>
                    {lang === "ru" ? "Служба TUN" : "TUN service"}:{" "}
                    {data.service.installed
                      ? data.service.running
                        ? "Running"
                        : "Installed"
                      : "—"}
                  </span>
                  <button
                    disabled={busy || active || !data.engine_ready}
                    onClick={() =>
                      action("manage_service", {
                        install: !data.service.installed,
                      })
                    }
                  >
                    {lang === "ru"
                      ? data.service.installed
                        ? "Удалить службу (UAC)"
                        : "Установить службу (UAC)"
                      : data.service.installed
                        ? "Remove service (UAC)"
                        : "Install service (UAC)"}
                  </button>
                </div>
                {data.service.installed && (
                  <button
                    disabled={busy || !data.engine_ready}
                    onClick={() =>
                      action("manage_service", { install: true, upgrade: true })
                    }
                  >
                    {lang === "ru"
                      ? "Обновить службу до версии клиента (UAC)"
                      : "Upgrade service to client version (UAC)"}
                  </button>
                )}
                <p>
                  {lang === "ru"
                    ? "Установщик автоматически устанавливает или обновляет службу TUN с подтверждением Windows. Кнопки здесь предназначены для восстановления службы и переносной версии. Интерфейс работает без прав администратора."
                    : "Windows asks for UAC only to install or remove the service. Runtimes are bundled. Upgrade an existing service after updating the client."}
                </p>
                <div className="form-grid">
                  <HelpLabel topic="proxyPort">
                    {t("proxyPort")}
                    <input
                      type="number"
                      min="1024"
                      max="65535"
                      value={draft.proxy_port}
                      onChange={(e) =>
                        field("proxy_port", Number(e.target.value))
                      }
                    />
                  </HelpLabel>
                  <HelpLabel topic="probe">
                    {t("probeUrl")}
                    <input
                      type="url"
                      value={draft.probe_url}
                      onChange={(e) => field("probe_url", e.target.value)}
                    />
                  </HelpLabel>
                </div>
                <p>{t("probeNote")}</p>
              </section>
            </div>
            <div hidden={networkTab !== "general"}>
              <details className="advanced-disclosure">
                <summary>
                  {lang === "ru"
                    ? "Автовыбор сервера — экспериментально"
                    : "Server selection — experimental"}
                </summary>
                <BestServer
                  lang={lang}
                  busy={busy || data.runtime_preparing}
                  count={data.profiles.length}
                  enabled={draft.auto_failover}
                  samples={draft.selection_samples}
                  change={(key, value) =>
                    key === "auto_failover"
                      ? field(key, value as boolean)
                      : field(key, value as number)
                  }
                  refresh={() => reload(true)}
                  error={setError}
                />
              </details>
            </div>
          </>
        )}
        {page === "settings" && (
          <>
            <div
              className="settings-tabs"
              role="tablist"
              aria-label={
                lang === "ru" ? "Раздел настроек" : "Settings section"
              }
            >
              {[
                ["appearance", "Оформление", "Appearance"],
                ["ping", "Пинг", "Ping"],
                ["cores", "Ядра и обновления", "Cores and updates"],
                ["about", "О NetPulse", "About NetPulse"],
                ["data", "Локальные данные", "Local data"],
              ].map(([key, r, en]) => (
                <button
                  key={key}
                  role="tab"
                  aria-selected={settingsTab === key}
                  onClick={() => setSettingsTab(key)}
                >
                  {lang === "ru" ? r : en}
                </button>
              ))}
            </div>
            <div hidden={settingsTab !== "appearance"}>
              <section className="card">
                <h3>{t("appearance")}</h3>
                <div className="form-grid">
                  <HelpLabel topic="language">
                    {t("language")}
                    <select
                      value={draft.language}
                      onChange={(e) =>
                        field("language", e.target.value as "ru" | "en")
                      }
                    >
                      <option value="ru">Русский</option>
                      <option value="en">English</option>
                    </select>
                  </HelpLabel>
                  <HelpLabel topic="theme">
                    {t("theme")}
                    <select
                      value={draft.theme}
                      onChange={(e) =>
                        field("theme", e.target.value as Preferences["theme"])
                      }
                    >
                      {(["dark", "light", "system", "custom"] as const).map(
                        (v) => (
                          <option key={v} value={v}>
                            {t(v)}
                          </option>
                        ),
                      )}
                    </select>
                  </HelpLabel>
                  <div className="palette-options">
                    <span>
                      {lang === "ru" ? "Готовые цвета" : "Color presets"}
                    </span>
                    <div className="data-actions">
                      {palettes.map((palette) => (
                        <button
                          key={palette.name}
                          style={{ borderColor: palette.accent }}
                          onClick={() => {
                            field("theme", "custom");
                            field("accent", palette.accent);
                            field("surface", palette.surface);
                          }}
                        >
                          <span
                            className="palette-dot"
                            style={{ background: palette.accent }}
                          />
                          {lang === "ru" ? palette.name : palette.en}
                        </button>
                      ))}
                    </div>
                  </div>
                  <HelpLabel topic="accent">
                    {t("accent")}
                    <input
                      type="color"
                      value={draft.accent}
                      onChange={(e) => field("accent", e.target.value)}
                    />
                  </HelpLabel>
                  {draft.theme === "custom" && (
                    <HelpLabel topic="surface">
                      {t("surface")}
                      <input
                        type="color"
                        value={draft.surface}
                        onChange={(e) => field("surface", e.target.value)}
                      />
                    </HelpLabel>
                  )}
                  <HelpLabel topic="radius">
                    {t("radius")} · {draft.radius}px
                    <input
                      type="range"
                      min="0"
                      max="32"
                      value={draft.radius}
                      onChange={(e) => field("radius", Number(e.target.value))}
                    />
                  </HelpLabel>
                  <HelpLabel topic="density">
                    {t("density")}
                    <select
                      value={draft.density}
                      onChange={(e) =>
                        field(
                          "density",
                          e.target.value as Preferences["density"],
                        )
                      }
                    >
                      <option value="comfortable">{t("comfortable")}</option>
                      <option value="compact">{t("compact")}</option>
                    </select>
                  </HelpLabel>
                  <HelpLabel topic="motion" className="check-label">
                    <input
                      type="checkbox"
                      checked={draft.motion}
                      onChange={(e) => field("motion", e.target.checked)}
                    />
                    {t("motion")}
                  </HelpLabel>
                </div>
              </section>
            </div>
            <div hidden={settingsTab !== "ping"}>
              <PingSettings draft={draft} change={field} lang={lang} />
            </div>
            <div hidden={settingsTab !== "cores"}>
              <CoreSettings
                transport={draft.transport_engine}
                changeTransport={(v) => field("transport_engine", v)}
                singboxVersion={data.engine_version}
                xrayVersion={data.xray_version}
                busy={busy || data.runtime_preparing}
                active={active}
                lang={lang}
                action={action}
              />
            </div>
            <div hidden={settingsTab !== "about"}>
              <AboutSettings
                release={clientRelease}
                receiveRelease={receiveClientRelease}
                draft={draft}
                change={field}
                lang={lang}
                version={data.app_version}
                busy={busy}
                dirty={dirty}
                action={action}
              />
            </div>
            <div hidden={settingsTab !== "data"}>
              <section className="card">
                <h3>{t("data")}</h3>
                <p>{t("backupNote")}</p>
                <div className="data-actions">
                  <button
                    disabled={busy}
                    onClick={() =>
                      action("export_data", { kind: "settings" }, false)
                    }
                  >
                    <Download size={16} />
                    {t("exportSettings")}
                  </button>
                  <button
                    disabled={busy || active}
                    onClick={() => action("restore_data", { backup: false })}
                  >
                    <Upload size={16} />
                    {t("importSettings")}
                  </button>
                  <button
                    disabled={busy}
                    onClick={() =>
                      action("export_data", { kind: "backup" }, false)
                    }
                  >
                    <Download size={16} />
                    {t("backup")}
                  </button>
                  <button
                    disabled={busy || active}
                    onClick={() => action("restore_data", { backup: true })}
                  >
                    <Upload size={16} />
                    {t("restore")}
                  </button>
                </div>
                <button
                  className="danger"
                  disabled={busy}
                  onClick={() => {
                    if (window.confirm(t("deleteConfirm")))
                      action("delete_local_data");
                  }}
                >
                  <Trash2 size={16} />
                  {t("deleteData")}
                </button>
              </section>
            </div>
          </>
        )}
        {dirty && draft && (
          <div className="save-bar" role="status">
            <span>
              {lang === "ru"
                ? active
                  ? "Есть изменения. Сохранение применит сетевые настройки с проверкой и откатом."
                  : "Есть несохранённые изменения."
                : "Unsaved changes. Network settings are applied with a verified restart."}
            </span>
            <button
              disabled={busy}
              onClick={() => {
                setEdited(false);
                editedRef.current = false;
                setDraft(data.preferences);
              }}
            >
              {lang === "ru" ? "Отменить" : "Cancel"}
            </button>
            <button className="primary" disabled={busy} onClick={save}>
              <Check size={16} />
              {t("save")}
            </button>
          </div>
        )}
        <footer>
          <span>
            <span
              className={`dot ${unconfirmed ? "unconfirmed" : active ? "green" : ""}`}
            />
            {stateLabel}
          </span>
          <span>Windows · {data.app_version}</span>
        </footer>
      </main>
      {chooseNetwork && (
        <div className="overlay">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="connect-mode-title"
          >
            <h2 id="connect-mode-title">
              {lang === "ru"
                ? "Как подключить VPN?"
                : "Choose VPN connection mode"}
            </h2>
            <p>
              {lang === "ru"
                ? "Выберите TUN и/или System Proxy. До подтверждения сетевой трафик не перенаправляется."
                : "Choose TUN and/or System Proxy before network traffic is redirected."}
            </p>
            <HelpLabel topic="tun" className="check-label">
              <input
                type="checkbox"
                checked={connectTun}
                onChange={(e) => setConnectTun(e.target.checked)}
              />
              TUN
            </HelpLabel>
            <HelpLabel topic="proxy" className="check-label">
              <input
                type="checkbox"
                checked={connectProxy}
                onChange={(e) => setConnectProxy(e.target.checked)}
              />
              System Proxy
            </HelpLabel>
            <div className="row">
              <button
                className="primary"
                disabled={busy || (!connectTun && !connectProxy)}
                onClick={async () => {
                  const ok = await quickNetwork({
                    tun: connectTun,
                    systemProxy: connectProxy,
                  });
                  if (ok) {
                    setChooseNetwork(false);
                    await action("connect", { id: selected });
                  }
                }}
              >
                {lang === "ru" ? "Подключиться" : "Connect"}
              </button>
              <button disabled={busy} onClick={() => setChooseNetwork(false)}>
                {t("cancel")}
              </button>
            </div>
          </section>
        </div>
      )}
      {serverPicker && (
        <ServerSelector
          profiles={data.profiles}
          view={draft.server_view}
          changeView={(v) => void quickUi({ server_view: v })}
          subscriptions={data.subscriptions}
          selected={selected}
          favorites={draft?.favorites || []}
          busy={busy}
          lang={lang}
          choose={selectProfile}
          close={() => setServerPicker(false)}
          toggleFavorite={toggleFavorite}
          refresh={async (id) => {
            await action("update_subscription", { id });
          }}
        />
      )}
      {pendingNavigation && (
        <div className="overlay">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="unsaved-title"
          >
            <h2 id="unsaved-title">
              {lang === "ru" ? "Сохранить изменения?" : "Save changes?"}
            </h2>
            <p>
              {lang === "ru"
                ? "Настройки изменены. Выберите действие перед переходом или закрытием окна."
                : "Settings have changed. Choose an action before leaving."}
            </p>
            <div className="row">
              <button
                disabled={busy}
                className="primary"
                onClick={async () => {
                  if (await save()) {
                    const next = pendingNavigation;
                    setPendingNavigation(null);
                    await finishNavigation(next);
                  }
                }}
              >
                {lang === "ru" ? "Сохранить и продолжить" : "Save and continue"}
              </button>
              <button
                disabled={busy}
                onClick={async () => {
                  setEdited(false);
                  editedRef.current = false;
                  setDraft(data.preferences);
                  const next = pendingNavigation;
                  setPendingNavigation(null);
                  await finishNavigation(next);
                }}
              >
                {lang === "ru" ? "Не сохранять" : "Discard"}
              </button>
              <button
                disabled={busy}
                onClick={() => setPendingNavigation(null)}
              >
                {lang === "ru" ? "Остаться" : "Stay"}
              </button>
            </div>
          </section>
        </div>
      )}
      {modal && (
        <ImportModal
          t={t}
          busy={busy}
          error={error}
          close={() => setModal(false)}
          imported={imported}
        />
      )}
    </div>
  );
}
function Metric({
  icon,
  label,
  value,
  sub,
  color,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  sub: string;
  color: string;
}) {
  return (
    <section className="card metric">
      <div className="row">
        <span className={`metric-icon ${color}`}>{icon}</span>
        <span className="muted">{label}</span>
      </div>
      <strong>{value}</strong>
      <small>{sub}</small>
    </section>
  );
}
document.addEventListener("contextmenu", (e) => e.preventDefault());
createRoot(document.getElementById("root")!).render(<App />);
