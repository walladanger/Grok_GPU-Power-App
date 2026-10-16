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
