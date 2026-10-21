import type { Preferences } from "./types";
export function networkPreset(
  p: Preferences,
  preset: Preferences["network_preset"],
): Preferences {
  if (preset === "custom") return { ...p, network_preset: preset };
  return {
    ...p,
    network_preset: preset,
    mtu: preset === "games" ? p.mtu : preset === "compatibility" ? 1400 : 1500,
    dns: { mode: "https", server: "1.1.1.1", path: "/dns-query" },
    dns_route: preset === "compatibility" ? "direct" : "vpn",
    // A tuning preset must not silently disable an already selected proxy mode.
    ...(preset === "games" ? { tun: true } : {}),
  };
}
