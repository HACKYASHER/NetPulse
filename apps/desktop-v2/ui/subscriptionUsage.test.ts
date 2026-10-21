import { test } from "node:test";
import assert from "node:assert/strict";
import { subscriptionUsage } from "./subscriptionUsage.ts";
import type { Subscription } from "./types.ts";
const sub: Subscription = {
  id: "fixture",
  name: "Fixture",
  updated_at: 0,
  count: 1,
  interval_hours: 24,
  url_protected: true,
  config_locked: false,
  usage: null,
};
test("subscription usage never invents a quota or expiry", () => {
  assert.match(subscriptionUsage(sub, "ru"), /не передал/);
  assert.match(
    subscriptionUsage(
      {
        ...sub,
        usage: { upload: null, download: null, total: 1024, expire: null },
      },
      "ru",
    ),
    /остаток неизвестен/,
  );
  assert.match(
    subscriptionUsage(
      {
        ...sub,
        usage: { upload: 100, download: 200, total: 1024, expire: 1799999999 },
      },
      "en",
    ),
    /Remaining 724 B/,
  );
  assert.match(
    subscriptionUsage(
      {
        ...sub,
        usage: { upload: 1000, download: 200, total: 1024, expire: 0 },
      },
      "en",
    ),
    /Remaining 0 B/,
  );
  assert.match(
    subscriptionUsage(
      { ...sub, usage: { upload: 0, download: 0, total: 0, expire: 0 } },
      "en",
    ),
    /Unlimited traffic.*No expiry/,
  );
});
