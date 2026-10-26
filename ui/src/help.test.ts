import { CONTROL_IDS, helpText } from "./help";

test("every control has help and preset 80 names 280 W", () => {
  for (const id of CONTROL_IDS) {
    expect(helpText(id).length).toBeGreaterThan(0);
  }
  expect(helpText("preset-80", { defaultWatts: 350, min: 100, max: 350, unit: "watts" })).toContain("280");
});

test("disabled apply explains the legal range", () => {
  const text = helpText("apply-disabled", { min: 100, max: 350, unit: "watts" });
  expect(text).toContain("100");
  expect(text).toContain("350");
});

test("every id in CONTROL_IDS returns non-empty text with and without a HelpContext", () => {
  for (const id of CONTROL_IDS) {
    expect(helpText(id).length).toBeGreaterThan(0);
    expect(helpText(id, { defaultWatts: 100, min: 50, max: 150, unit: "watts" }).length).toBeGreaterThan(0);
  }
});

test("preset ids return the exact computed values for watts and percent units", () => {
  // Test unit: "watts" with a valid defaultWatts
  expect(helpText("preset-60", { defaultWatts: 200, unit: "watts" })).toBe("Apply 120 W, 60 percent of this card's default limit.");
  expect(helpText("preset-70", { defaultWatts: 200, unit: "watts" })).toBe("Apply 140 W, 70 percent of this card's default limit.");
  expect(helpText("preset-80", { defaultWatts: 200, unit: "watts" })).toBe("Apply 160 W, 80 percent of this card's default limit.");
  expect(helpText("preset-90", { defaultWatts: 200, unit: "watts" })).toBe("Apply 180 W, 90 percent of this card's default limit.");
  expect(helpText("preset-100", { defaultWatts: 200, unit: "watts" })).toBe("Apply 200 W, 100 percent of this card's default limit.");

  // Test unit: "percent"
  expect(helpText("preset-60", { unit: "percent" })).toBe("Apply 60 percent of this card's allowed power-limit range.");
  expect(helpText("preset-70", { unit: "percent" })).toBe("Apply 70 percent of this card's allowed power-limit range.");
  expect(helpText("preset-80", { unit: "percent" })).toBe("Apply 80 percent of this card's allowed power-limit range.");
  expect(helpText("preset-90", { unit: "percent" })).toBe("Apply 90 percent of this card's allowed power-limit range.");
  expect(helpText("preset-100", { unit: "percent" })).toBe("Apply 100 percent of this card's allowed power-limit range.");

  // Test undefined context or missing defaultWatts / missing unit
  expect(helpText("preset-60")).toBe("Apply 60 percent of this card's default limit.");
  expect(helpText("preset-70")).toBe("Apply 70 percent of this card's default limit.");
  expect(helpText("preset-80")).toBe("Apply 80 percent of this card's default limit.");
  expect(helpText("preset-90")).toBe("Apply 90 percent of this card's default limit.");
  expect(helpText("preset-100")).toBe("Apply 100 percent of this card's default limit.");
});

test("apply-disabled returns exact strings depending on min, max, and unit", () => {
  // Test with min, max, and unit: "watts" or "percent"
  expect(helpText("apply-disabled", { min: 100, max: 350, unit: "watts" })).toBe("Apply is unavailable because the value must be from 100 to 350 watts.");
  expect(helpText("apply-disabled", { min: 50, max: 100, unit: "percent" })).toBe("Apply is unavailable because the value must be from 50 to 100 percent.");

  // Test with min, max, but no unit
  expect(helpText("apply-disabled", { min: 100, max: 350 })).toBe("Apply is unavailable because the value must be from 100 to 350.");

  // Test with missing min or max
  expect(helpText("apply-disabled", { min: 100 })).toBe("Apply is unavailable because the value is outside the legal range.");
  expect(helpText("apply-disabled", { max: 350 })).toBe("Apply is unavailable because the value is outside the legal range.");
  expect(helpText("apply-disabled")).toBe("Apply is unavailable because the value is outside the legal range.");
});

test("unknown id returns the fallback text", () => {
  expect(helpText("non-existent-id")).toBe("This control has no further description.");
});

test("CONTROL_IDS has no duplicates", () => {
  const uniqueIds = new Set(CONTROL_IDS);
  expect(uniqueIds.size).toBe(CONTROL_IDS.length);
});
