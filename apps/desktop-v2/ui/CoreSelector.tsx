import { Help } from "./Help";
import type { Preferences } from "./types";

export function CoreSelector(p: {
  lang: string;
  value: Preferences["transport_engine"];
  disabled?: boolean;
  change: (value: Preferences["transport_engine"]) => void;
}) {
  const title = p.lang === "ru" ? "Транспортное ядро" : "Transport core";
  return (
    <div className="core-selector">
      <span className="field-caption">
        {p.lang === "ru" ? "Ядро" : "Core"}
        <Help topic="core" />
      </span>
      <div className="segments" role="group" aria-label={title}>
        {(["auto", "sing-box", "xray"] as const).map((value) => (
          <button
            type="button"
            key={value}
            disabled={p.disabled}
            aria-pressed={p.value === value}
            className={p.value === value ? "selected" : ""}
            onClick={() => p.change(value)}
          >
            {value === "auto"
              ? p.lang === "ru"
                ? "Авто"
                : "Auto"
              : value === "xray"
                ? "Xray"
                : "sing-box"}
          </button>
        ))}
      </div>
    </div>
  );
}
