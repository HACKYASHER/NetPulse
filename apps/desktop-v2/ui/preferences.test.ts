import { test } from "node:test";
import assert from "node:assert/strict";
import {
  preferencesChanged,
  mergeQuickNetwork,
  mergeQuickUi,
  hasUnsavedChanges,
} from "./preferences.ts";
import type { Preferences } from "./types.ts";
import { mergeRouting, routingDocument } from "./routing.ts";
const saved: Preferences = {
  language: "ru",
  theme: "dark",
  accent: "#8B7CF8",
  surface: "#12151E",
  radius: 16,
  density: "comfortable",
  motion: true,
  route_mode: "global",
  applications: [],
  bypass_domains: [],
  whitelist: ["C:\\Apps\\ONE.exe", "C:\\Apps\\TWO.exe"],
  blacklist: [],
  proxy_port: 2080,
  system_proxy: false,
  tun: false,
  dns: { mode: "system" },
  dns_route: "direct",
  mtu: 1500,
  network_preset: "custom",
  applications_migrated: true,
  probe_url: "https://example.org/check",
  transport_engine: "auto",
  tls_defaults: null,
  fake_ip: false,
  routing_rules: [],
  sniff_excluded_apps: [],
  fragmentation: {
    enabled: false,
    packets: "tlshello",
    length: "50-100",
    interval: "10-20",
    max_split: "100-200",
  },
  multiplex: {
    enabled: false,
    concurrency: 8,
    xudp_concurrency: 8,
    udp_443: "skip",
  },
  ping_method: "proxy_https_rtt",
  ping_proxy_mode: "double",
  selected_profile: null,
  favorites: ["a", "b"],
  app_view: "list",
  server_view: "small",
  autostart: false,
  close_to_tray: true,
  minimize_to_tray: true,
  auto_reconnect: false,
  auto_failover: false,
  selection_samples: 10,
  logging_level: "INFO",
  window: { width: 1240, height: 850, maximized: false },
  application_groups: [],
};
test("routing import preserves VPN switches and unrelated unsaved edits", () => {
  const draft = {
    ...saved,
    tun: true,
    system_proxy: true,
    mtu: 1400,
    radius: 27,
  };
  const doc = routingDocument({
    ...saved,
    route_mode: "whitelist",
    applications: ["C:\\Apps\\new.exe"],
    bypass_domains: ["example.com"],
  });
  const imported = mergeRouting(draft, doc);
  assert.equal(imported.tun, true);
  assert.equal(imported.system_proxy, true);
  assert.equal(imported.mtu, 1400);
  assert.equal(imported.radius, 27);
  assert.equal(imported.route_mode, "whitelist");
  assert.deepEqual(imported.bypass_domains, ["example.com"]);
  assert.equal(preferencesChanged(draft, imported), true);
  assert.equal("tun" in doc, false);
  assert.equal("selected_profile" in doc, false);
});
test("domain edits normalize without leaving a spurious save prompt", () => {
  const base = { ...saved, bypass_domains: ["example.com"] };
  assert.equal(
    preferencesChanged(base, {
      ...base,
      bypass_domains: [" *.Example.COM. ", "", "example.com"],
    }),
    false,
  );
});
test("quick network changes keep an unrelated draft and do not create a save prompt", () => {
  const applied = {
    ...saved,
    tun: true,
    system_proxy: true,
    route_mode: "blacklist" as const,
  };
  assert.equal(
    preferencesChanged(applied, mergeQuickNetwork(saved, applied)),
    false,
  );
  const draft = { ...saved, radius: 24, whitelist: ["C:\\Apps\\Draft.exe"] };
  const merged = mergeQuickNetwork(draft, applied);
  assert.equal(merged.radius, 24);
  assert.deepEqual(merged.whitelist, draft.whitelist);
  assert.equal(preferencesChanged(applied, merged), true);
  assert.deepEqual(saved.whitelist, ["C:\\Apps\\ONE.exe", "C:\\Apps\\TWO.exe"]);
});
test("no dirty state after cloning, reordering paths, duplicate paths or color case", () => {
  assert.equal(preferencesChanged(saved, structuredClone(saved)), false);
  assert.equal(
    preferencesChanged(saved, {
      ...saved,
      accent: "#8b7cf8",
      whitelist: [
        "c:\\apps\\two.exe",
        "C:\\Apps\\ONE.exe",
        "C:\\APPS\\ONE.exe",
      ],
      favorites: ["b", "a", "a"],
    }),
    false,
  );
});
test("every editable setting marks the draft; reverting clears it", () => {
  for (const change of [
    { tun: true },
    { system_proxy: true },
    { radius: 24 },
    { language: "en" },
    { dns: { mode: "udp", server: "1.1.1.1" } },
    { whitelist: [] },
    { favorites: [] },
    { app_view: "cards" },
    { application_groups: [{ name: "Work", paths: ["C:\\Apps\\ONE.exe"] }] },
  ])
    assert.equal(
      preferencesChanged(saved, { ...saved, ...change } as Preferences),
      true,
    );
  const draft = { ...saved, radius: 24 };
  assert.equal(preferencesChanged(saved, draft), true);
  draft.radius = 16;
  assert.equal(preferencesChanged(saved, draft), false);
});
test("native geometry changes do not mark an unrelated form dirty", () =>
  assert.equal(
    preferencesChanged(saved, {
      ...saved,
      window: { width: 1600, height: 900, maximized: true },
    }),
    false,
  ));

test("saved view aliases normalize without a startup save flash", () => {
  assert.equal(
    preferencesChanged(
      { ...saved, app_view: "list", server_view: "small" },
      { ...saved, app_view: "table", server_view: "medium" },
    ),
    false,
  );
});

test("programmatic startup changes do not display a save prompt", () => {
  const launch = { ...saved, tun: true, selected_profile: "restored-profile" };
  assert.equal(hasUnsavedChanges(saved, launch, false), false);
  assert.equal(hasUnsavedChanges(saved, { ...saved, radius: 24 }, true), true);
  assert.equal(hasUnsavedChanges(saved, saved, true), false);
});

test("immediate views and favorites never create a save prompt or discard DNS/app edits", () => {
  const updated = {
    ...saved,
    app_view: "medium" as const,
    server_view: "table" as const,
    favorites: ["new"],
  };
  const baseline = mergeQuickUi(saved, updated);
  assert.equal(
    hasUnsavedChanges(baseline, mergeQuickUi(saved, updated), true),
    false,
  );
  const draft = {
    ...saved,
    mtu: 1400,
    applications: ["C:\\Apps\\draft.exe"],
    dns_route: "vpn" as const,
    dns: { mode: "https" as const, server: "example.org", path: "/dns-query" },
  };
  const merged = mergeQuickUi(draft, updated);
  assert.equal(merged.mtu, 1400);
  assert.deepEqual(merged.applications, draft.applications);
  assert.deepEqual(merged.dns, draft.dns);
  assert.equal(merged.dns_route, "vpn");
  assert.deepEqual(merged.favorites, ["new"]);
  assert.equal(hasUnsavedChanges(baseline, merged, true), true);
});

import { networkPreset } from "./networkPresets.ts";
test("games preset preserves MTU, routing list and transport opts while selecting encrypted DNS and TUN", () => {
  const old = {
    ...saved,
    mtu: 1420,
    system_proxy: true,
    tun: false,
    applications: ["C:\\Apps\\Roblox.exe"],
  };
  const next = networkPreset(old, "games");
  assert.equal(next.mtu, 1420);
  assert.equal(next.tun, true);
  assert.equal(next.system_proxy, true);
  assert.equal(
    networkPreset({ ...old, system_proxy: false }, "games").system_proxy,
    false,
  );
  assert.deepEqual(next.dns, {
    mode: "https",
    server: "1.1.1.1",
    path: "/dns-query",
  });
  assert.equal(next.dns_route, "vpn");
  assert.deepEqual(next.applications, old.applications);
  assert.deepEqual(next.fragmentation, old.fragmentation);
  assert.equal(old.system_proxy, true);
});
