import type { Preferences } from "./types";
export function mergeQuickNetwork(
  draft: Preferences,
  saved: Preferences,
): Preferences {
  return {
    ...draft,
    tun: saved.tun,
    system_proxy: saved.system_proxy,
    route_mode: saved.route_mode,
  };
}
export function mergeQuickUi(
  draft: Preferences,
  saved: Preferences,
): Preferences {
  return {
    ...draft,
    app_view: saved.app_view,
    server_view: saved.server_view,
    favorites: saved.favorites,
  };
}
const paths = (values: string[]) =>
  [...new Set(values.map((v) => v.toLowerCase()))].sort();
export function normalizedPreferences(p: Preferences) {
  return {
    ...p,
    window: null,
    app_view: ["list", "table"].includes(p.app_view) ? "table" : "medium",
    server_view: ["list", "table"].includes(p.server_view) ? "table" : "medium",
    accent: p.accent.toUpperCase(),
    surface: p.surface.toUpperCase(),
    applications: paths(p.applications),
    sniff_excluded_apps: paths(p.sniff_excluded_apps),
    bypass_domains: [
      ...new Set(
        p.bypass_domains
          .map((v) =>
            v
              .trim()
              .toLowerCase()
              .replace(/^\*\.|^\./, "")
              .replace(/\.$/, ""),
          )
          .filter(Boolean),
      ),
    ].sort(),
    whitelist: paths(p.whitelist),
    blacklist: paths(p.blacklist),
    favorites: [...new Set(p.favorites)].sort(),
    application_groups: p.application_groups
      .map((g) => ({ ...g, paths: paths(g.paths) }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  };
}
function canonical(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(canonical).join(",")}]`;
  if (v && typeof v === "object")
    return `{${Object.entries(v)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([k, value]) => `${JSON.stringify(k)}:${canonical(value)}`)
      .join(",")}}`;
  return JSON.stringify(v);
}
export const preferencesChanged = (saved: Preferences, draft: Preferences) =>
  canonical(normalizedPreferences(saved)) !==
  canonical(normalizedPreferences(draft));

export const hasUnsavedChanges = (
  saved: Preferences,
  draft: Preferences,
  edited: boolean,
) => edited && preferencesChanged(saved, draft);
