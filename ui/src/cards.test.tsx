import { render, screen, act } from "@testing-library/react";
import { vi } from "vitest";
import { Dashboard, FocusedView, DelayedHelp } from "./cards";
import { primary3090, gtx1050 } from "./fixtures";

test("omits empty sensors and shows the 1050 error without a zero watt reading", () => {
  render(<Dashboard cards={[primary3090(), gtx1050()]} range="5" staleIds={[]} />);
  expect(screen.getByText("NVIDIA GeForce GTX 1050")).toBeInTheDocument();
  expect(screen.getByText("Error")).toBeInTheDocument();
  expect(screen.queryByText("0 W")).not.toBeInTheDocument();
  expect(screen.queryByText(/memory temperature/i)).not.toBeInTheDocument();
  expect(screen.getByText(/8\.24/)).toBeInTheDocument();
  expect(screen.getByText("32.0.16.1692")).toBeInTheDocument();
  expect(screen.getByText("1234 steam.exe")).toBeInTheDocument();
});

test("stale card says updating", () => {
  const card = primary3090();
  render(<Dashboard cards={[card]} range="5" staleIds={[card.id]} />);
  expect(screen.getByText("updating")).toBeInTheDocument();
});

test("wide layout class starts at 960", () => {
  const { container } = render(<Dashboard cards={[primary3090(), primary3090()]} range="5" staleIds={[]} />);
  expect(container.querySelector("[data-layout]")).toHaveAttribute("data-breakpoint", "960");
});

test("FocusedView renders graph for healthy card", () => {
  const card = primary3090();
  render(<FocusedView card={card} stale={false} kind="graph" points={[]} />);
  expect(screen.getByText("NVIDIA GeForce RTX 3090")).toBeInTheDocument();
  expect(screen.queryByText("updating")).not.toBeInTheDocument();
  expect(screen.getByTestId(`graph-${card.id}`)).toBeInTheDocument();
});

test("FocusedView renders sensors for healthy card", () => {
  const card = primary3090();
  render(<FocusedView card={card} stale={false} kind="sensors" points={[]} />);
  expect(screen.getByText("NVIDIA GeForce RTX 3090")).toBeInTheDocument();
  expect(screen.queryByText("updating")).not.toBeInTheDocument();
  expect(screen.getByText("Average power")).toBeInTheDocument();
  expect(screen.getByText("8.66 W")).toBeInTheDocument();
});

test("FocusedView renders graph for error card", () => {
  const card = gtx1050();
  render(<FocusedView card={card} stale={false} kind="graph" points={[]} />);
  expect(screen.getByText("NVIDIA GeForce GTX 1050")).toBeInTheDocument();
  expect(screen.getByTestId(`graph-${card.id}`)).toBeInTheDocument();
});

test("FocusedView renders sensors for error card", () => {
  const card = gtx1050();
  render(<FocusedView card={card} stale={false} kind="sensors" points={[]} />);
  expect(screen.getByText("NVIDIA GeForce GTX 1050")).toBeInTheDocument();
  // Average power is null for gtx1050, so it shouldn't render
  expect(screen.queryByText("Average power")).not.toBeInTheDocument();
});

test("DelayedHelp shows tooltip after delay", () => {
  vi.useFakeTimers();
  render(
    <DelayedHelp id="open-dashboard">
      <button>Test Button</button>
    </DelayedHelp>
  );

  const span = screen.getByText("Test Button").closest("span") as HTMLElement;

  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();

  act(() => {
    span.dispatchEvent(new MouseEvent("mouseenter", { bubbles: true }));
  });
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();

  act(() => {
    vi.advanceTimersByTime(500);
  });

  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toBeInTheDocument();
  expect(tooltip.textContent).toBe("Open the dashboard window.");

  act(() => {
    span.dispatchEvent(new MouseEvent("mouseleave", { bubbles: true }));
  });
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();

  vi.useRealTimers();
});
