export type Profile = {
  id: string;
  name: string;
  protocol: string;
  host: string;
  port: number;
  subscription_id: string | null;
  latency_ms: number | null;
  latency_method: string | null;
  latency_at: number | null;
  latency_status: string | null;
};
export type Subscription = {
  id: string;
  name: string;
  updated_at: number;
  count: number;
  interval_hours: number;
  url_protected: boolean;
  config_locked: boolean;
  usage: {
    upload: number | null;
    download: number | null;
    total: number | null;
    expire: number | null;
  } | null;
};
export type Event = {
  id: string;
  timestamp: number;
  level: string;
  subsystem: string;
  stage: string;
  code: string;
  message: string;
};
export type NativeError = { code: string; stage: string; message: string };
export type ClientRelease = {
  tag: string;
  page: string;
  url: string;
  sha256: string;
  size: number;
  prerelease: boolean;
};
export type Metrics = {
  upload_total: number;
  download_total: number;
  upload_bps: number;
  download_bps: number;
  connections: number;
};
export type Snapshot = {
  state:
    | "disconnected"
    | "validating"
    | "connecting"
    | "connected"
    | "switching"
    | "disconnecting"
    | "error";
  active: Profile | null;
  engine_pid: number | null;
  metrics: Metrics | null;
  metrics_error?: NativeError | null;
  error: NativeError | null;
  probe_ms: number | null;
  system_proxy_active: boolean;
  tun_active: boolean;
  applied_preferences: Preferences | null;
  recovery_pid: number | null;
};
export type Dns =
  | { mode: "system" }
  | { mode: "udp" | "tls"; server: string }
  | { mode: "https"; server: string; path: string };
export type Transport = {
  sni_mode: string;
  server_name: string;
  http_host: string | null;
  fingerprint: string | null;
};
export type Preferences = {
  tls_defaults: Transport | null;
  language: "ru" | "en";
  theme: "dark" | "light" | "system" | "custom";
  accent: string;
  surface: string;
  radius: number;
  density: "compact" | "comfortable";
  motion: boolean;
  route_mode: "global" | "whitelist" | "blacklist" | "direct";
  applications: string[];
  bypass_domains: string[];
  proxy_port: number;
  system_proxy: boolean;
  tun: boolean;
  dns: Dns;
  dns_route: "direct" | "vpn";
  mtu: number;
  network_preset: "balanced" | "compatibility" | "games" | "custom";
  applications_migrated: boolean;
  probe_url: string;
  transport_engine: "auto" | "sing-box" | "xray";
  fake_ip: boolean;
  routing_rules: {
    enabled: boolean;
    kind: "domain" | "cidr" | "geoip" | "geosite" | "process";
    value: string;
    action: "proxy" | "direct" | "reject";
  }[];
  sniff_excluded_apps: string[];
  fragmentation: {
    enabled: boolean;
    packets: "tlshello" | "1-3";
    length: string;
    interval: string;
    max_split: string;
  };
  multiplex: {
    enabled: boolean;
    concurrency: number;
    xudp_concurrency: number;
    udp_443: "reject" | "allow" | "skip";
  };
  ping_method:
    | "proxy_https_rtt"
    | "proxy_https_head_rtt"
    | "tcp_connect_rtt"
    | "icmp_rtt";
  ping_proxy_mode: "double" | "keepalive";
  selected_profile: string | null;
  favorites: string[];
  whitelist: string[];
  blacklist: string[];
  app_view: import("./ExplorerView").ExplorerMode;
  server_view: import("./ExplorerView").ExplorerMode;
  autostart: boolean;
  close_to_tray: boolean;
  minimize_to_tray: boolean;
  application_groups: { name: string; paths: string[] }[];
  auto_reconnect: boolean;
  auto_failover: boolean;
  selection_samples: number;
  logging_level: string;
  window: { width: number; height: number; maximized: boolean };
};
export type Bootstrap = {
  snapshot: Snapshot;
  preferences: Preferences;
  profiles: Profile[];
  subscriptions: Subscription[];
  logs: Event[];
  engine_ready: boolean;
  engine_version: string;
  xray_version: string;
  app_version: string;
  xray_ready: boolean;
  runtime_error: NativeError | null;
  runtime_preparing: boolean;
  startup_smoke: boolean;
  service: {
    version: string;
    installed: boolean;
    running: boolean;
    tun_ready: boolean;
    error: NativeError | null;
  };
};
export type ImportReport = {
  format: string;
  imported: number;
  issues: { index: number; code: string; message: string }[];
};
export type Application = { name: string; path: string; source: string };
