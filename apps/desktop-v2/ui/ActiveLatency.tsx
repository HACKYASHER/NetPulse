import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw } from "lucide-react";
import { pingMethods } from "./PingSettings";
export function ActiveLatency(p: {
  activeId?: string;
  enginePid?: number | null;
  method: string;
  lang: string;
}) {
  const [result, setResult] = useState<{
    min_ms: number | null;
    successes: number;
    timestamp: number;
    error?: { code?: string; message: string };
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const guard = useRef(false),
    generation = useRef(0);
  const measure = async () => {
    if (!p.activeId || guard.current) return;
    guard.current = true;
    setBusy(true);
    const version = generation.current;
    try {
      const v = await invoke<typeof result>("active_latency", {
        id: p.activeId,
      });
      if (version === generation.current) setResult(v);
    } catch (e) {
      if (version === generation.current)
        setResult({
          min_ms: null,
          successes: 0,
          timestamp: Date.now() / 1000,
          error: e as { code?: string; message: string },
        });
    } finally {
      guard.current = false;
      if (version === generation.current) setBusy(false);
    }
  };
  useEffect(() => {
    generation.current++;
    setResult(null);
    setBusy(false);
    let timer: ReturnType<typeof setTimeout>;
    const firstSample = () => {
      if (guard.current) timer = setTimeout(firstSample, 300);
      else void measure();
    };
    timer = setTimeout(firstSample, 1000);
    const interval = setInterval(() => void measure(), 30000);
    return () => {
      generation.current++;
      clearTimeout(timer);
      clearInterval(interval);
    };
  }, [p.activeId, p.enginePid, p.method]);
  return (
    <div className="active-latency muted small">
      <span
        title={
          result?.error?.message ||
          (result ? new Date(result.timestamp * 1000).toLocaleTimeString() : "")
        }
      >
        {pingMethods.find((v) => v[0] === p.method)?.[1]} ·{" "}
        {busy
          ? "…"
          : result?.successes && result.min_ms != null
            ? `${result.min_ms < 1 ? "<1" : result.min_ms.toFixed(0)} ms`
            : result?.error
              ? result.error.code === "LatencyTimeout"
                ? "Timeout"
                : p.lang === "ru"
                  ? "Ошибка проверки"
                  : "Test failed"
              : "—"}
      </span>
      <button
        disabled={busy || !p.activeId}
        onClick={() => void measure()}
        aria-label={p.lang === "ru" ? "Обновить пинг" : "Refresh ping"}
      >
        <RefreshCw size={14} />
      </button>
    </div>
  );
}
