import { describe, it, expect } from "vitest";
import {
  SETTINGS_LIMITS,
  createDefaultSettings,
  sanitizeSettings,
} from "../settingsSchema";
import type { Settings } from "../settingsSchema";

const VERSION = "3.9.2";

describe("background blur settings", () => {
  const BLUR_FIELDS = [
    "backgroundBlur",
    "backgroundBlurSimple",
    "backgroundBlurAdvanced",
    "backgroundBlurZones",
    "backgroundBlurClickPoints",
    "backgroundBlurSettings",
  ] as const;

  it("defaults all background blur fields to 0", () => {
    const defaults = createDefaultSettings(VERSION);
    for (const field of BLUR_FIELDS) {
      expect(defaults[field]).toBe(0);
    }
  });

  it("registers a 0-20 limit for all background blur fields", () => {
    for (const field of BLUR_FIELDS) {
      const limit = SETTINGS_LIMITS[field];
      expect(limit.min).toBe(0);
      expect(limit.max).toBe(20);
    }
  });

  it("fills missing background blur keys with 0 for legacy saved settings", () => {
    const legacy: Partial<Settings> = {
      version: "3.9.1",
      clickSpeed: 25,
      clickInterval: "s",
      panelBlur: 5,
    };
    const sanitized = sanitizeSettings(legacy, VERSION);
    for (const field of BLUR_FIELDS) {
      expect(sanitized[field]).toBe(0);
    }
  });

  it("clamps out-of-range background blur values", () => {
    const tooHigh = sanitizeSettings({ backgroundBlur: 50 }, VERSION);
    expect(tooHigh.backgroundBlur).toBe(20);

    const tooLow = sanitizeSettings({ backgroundBlur: -5 }, VERSION);
    expect(tooLow.backgroundBlur).toBe(0);

    const valid = sanitizeSettings({ backgroundBlur: 8 }, VERSION);
    expect(valid.backgroundBlur).toBe(8);
  });

  it("clamps per-page background blur fields independently", () => {
    const sanitized = sanitizeSettings(
      { backgroundBlurSimple: 99, backgroundBlurSettings: -1 },
      VERSION,
    );
    expect(sanitized.backgroundBlurSimple).toBe(20);
    expect(sanitized.backgroundBlurSettings).toBe(0);
  });
});
