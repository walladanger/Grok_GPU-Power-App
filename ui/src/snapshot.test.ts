import { expect, test } from "vitest";
import { combinedPower } from "./snapshot";
import { amdRaphael, gtx1050, primary3090, secondary3090 } from "./fixtures";

test("empty array -> {watts:0, missing_names:[]}", () => {
  expect(combinedPower([])).toEqual({ watts: 0, missing_names: [] });
});

test("sums power_draw_instant_w across cards", () => {
  const c1 = primary3090();
  c1.power_draw_instant_w = 100;
  const c2 = secondary3090();
  c2.power_draw_instant_w = 50;

  expect(combinedPower([c1, c2])).toEqual({
    watts: 150,
    missing_names: [],
  });
});

test("cards with power_draw_instant_w null (and undefined) are excluded from the sum and their names appear in missing_names in input order", () => {
  const c1 = primary3090();
  c1.power_draw_instant_w = null;
  c1.name = "Missing Null";

  const c2 = amdRaphael();
  // @ts-expect-error testing undefined behavior
  c2.power_draw_instant_w = undefined;
  c2.name = "Missing Undefined";

  expect(combinedPower([c1, c2])).toEqual({
    watts: 0,
    missing_names: ["Missing Null", "Missing Undefined"],
  });
});

test("a card with 0 W counts as present (not missing)", () => {
  const c1 = primary3090();
  c1.power_draw_instant_w = 0;

  expect(combinedPower([c1])).toEqual({
    watts: 0,
    missing_names: [],
  });
});

test("mixed list", () => {
  const c1 = primary3090();
  c1.power_draw_instant_w = 125;
  c1.name = "Present 125";

  const c2 = amdRaphael();
  c2.power_draw_instant_w = null;
  c2.name = "Missing 1";

  const c3 = gtx1050();
  c3.power_draw_instant_w = 0;
  c3.name = "Present 0";

  const c4 = secondary3090();
  // @ts-expect-error testing undefined behavior
  c4.power_draw_instant_w = undefined;
  c4.name = "Missing 2";

  const c5 = primary3090();
  c5.power_draw_instant_w = 75;
  c5.name = "Present 75";

  expect(combinedPower([c1, c2, c3, c4, c5])).toEqual({
    watts: 200,
    missing_names: ["Missing 1", "Missing 2"],
  });
});
