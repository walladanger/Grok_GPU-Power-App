import { render, screen } from "@testing-library/react";
import { Dashboard } from "./cards";
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
