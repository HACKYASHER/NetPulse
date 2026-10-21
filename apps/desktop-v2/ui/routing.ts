import type { Preferences } from "./types";
export type RoutingDocument = Pick<
  Preferences,
  | "route_mode"
  | "applications"
  | "application_groups"
  | "bypass_domains"
  | "routing_rules"
  | "sniff_excluded_apps"
> & { version: number };
export const routingDocument = (p: Preferences): RoutingDocument => ({
  version: 1,
  route_mode: p.route_mode,
  applications: p.applications,
  application_groups: p.application_groups,
  bypass_domains: p.bypass_domains,
  routing_rules: p.routing_rules,
  sniff_excluded_apps: p.sniff_excluded_apps,
});
export const mergeRouting = (
  draft: Preferences,
  routing: RoutingDocument,
): Preferences => ({
  ...draft,
  route_mode: routing.route_mode,
  applications: routing.applications,
  application_groups: routing.application_groups,
  bypass_domains: routing.bypass_domains,
  routing_rules: routing.routing_rules || [],
  sniff_excluded_apps: routing.sniff_excluded_apps || [],
});
