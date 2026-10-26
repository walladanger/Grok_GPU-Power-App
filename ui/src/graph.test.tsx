import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { PowerGraph, snapPoint } from "./graph";

test("hover shares one timestamp and click pins it", () => {
  const points = [
    { unix: 10, at: "10", power_w: 8.2, util_gpu: 0, detail: "8.2 W" },
    { unix: 12, at: "12", power_w: 200, util_gpu: 90, detail: "200 W" },
  ];
  function Harness() {
    const [pin, setPin] = useState<number | null>(null);
    const [hover, setHover] = useState<number | null>(null);
    return (
      <>
        <PowerGraph cardId="a" points={points} pinnedUnix={pin} onHoverUnix={setHover} onPinUnix={setPin} />
        <PowerGraph cardId="b" points={points} pinnedUnix={pin} onHoverUnix={setHover} onPinUnix={setPin} />
        <output>{hover ?? pin}</output>
      </>
    );
  }
  render(<Harness />);
  fireEvent.mouseMove(screen.getAllByTestId("graph-a")[0], { clientX: 0 });
  expect(screen.getAllByText("8.2 W").length).toBeGreaterThan(0);
  fireEvent.click(screen.getByTestId("graph-a"));
  fireEvent.mouseLeave(screen.getByTestId("graph-a"));
  expect(screen.getByTestId("pin-a")).toHaveTextContent("8.2 W");
});

test("snap uses the earlier sample on a tie", () => {
  const points = [
    { unix: 10, at: "10", power_w: 1, util_gpu: 0, detail: "1" },
    { unix: 12, at: "12", power_w: 2, util_gpu: 0, detail: "2" },
  ];
  expect(snapPoint(points, 11)?.unix).toBe(10);
});

test("snap returns null for empty array", () => {
  expect(snapPoint([], 10)).toBeNull();
});

test("snap always returns the single point for a 1-element array", () => {
  const points = [
    { unix: 10, at: "10", power_w: 1, util_gpu: 0, detail: "1" },
  ];
  expect(snapPoint(points, 0)?.unix).toBe(10);
  expect(snapPoint(points, 10)?.unix).toBe(10);
  expect(snapPoint(points, 20)?.unix).toBe(10);
});

test("snap returns the closest when target is before the first or after the last sample", () => {
  const points = [
    { unix: 10, at: "10", power_w: 1, util_gpu: 0, detail: "1" },
    { unix: 20, at: "20", power_w: 2, util_gpu: 0, detail: "2" },
  ];
  expect(snapPoint(points, 5)?.unix).toBe(10);
  expect(snapPoint(points, 25)?.unix).toBe(20);
});

test("snap returns the exact match", () => {
  const points = [
    { unix: 10, at: "10", power_w: 1, util_gpu: 0, detail: "1" },
    { unix: 20, at: "20", power_w: 2, util_gpu: 0, detail: "2" },
  ];
  expect(snapPoint(points, 20)?.unix).toBe(20);
});

test("snap handles unsorted input and lower index wins ties", () => {
  const points = [
    { unix: 30, at: "30", power_w: 1, util_gpu: 0, detail: "1" },
    { unix: 10, at: "10", power_w: 2, util_gpu: 0, detail: "2" },
    { unix: 20, at: "20", power_w: 3, util_gpu: 0, detail: "3" },
    { unix: 10, at: "10", power_w: 4, util_gpu: 0, detail: "4" },
  ];
  expect(snapPoint(points, 22)?.unix).toBe(20);
  expect(snapPoint(points, 22)?.detail).toBe("3");
  expect(snapPoint(points, 10)?.detail).toBe("2");
  expect(snapPoint(points, 20)?.detail).toBe("3");
  expect(snapPoint(points, 30)?.detail).toBe("1");
});
