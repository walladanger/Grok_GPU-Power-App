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
