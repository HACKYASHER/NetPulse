import { test } from "node:test";
import assert from "node:assert/strict";
import { palettes, surfaceColors } from "./theme.ts";
const luminance = (color: string) =>
  [1, 3, 5]
    .map((i) => parseInt(color.slice(i, i + 2), 16) / 255)
    .map((x) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4))
    .reduce((sum, x, i) => sum + x * [0.2126, 0.7152, 0.0722][i], 0);
test("preset themes recolor all surfaces and keep text readable", () => {
  for (const p of palettes) {
    const c = surfaceColors(p.surface),
      text = luminance(c["--text"]);
    for (const key of ["--surface", "--bg", "--card"] as const) {
      const surface = luminance(c[key]);
      assert.ok(
        (Math.max(text, surface) + 0.05) / (Math.min(text, surface) + 0.05) >=
          4.5,
        p.name + key,
      );
    }
    assert.notEqual(c["--card"], c["--bg"]);
  }
});
