import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { App, type DriverPackage } from "./App";
import { amdRaphael, primary3090, secondary3090 } from "./fixtures";
import { helpText } from "./help";

type Call = { cmd: string; args?: Record<string, unknown> };

test("apply disabled when the plan refuses the custom value", async () => {
  render(<App invoke={async (cmd) => {
    if (cmd === "plan_apply") return [{ kind: "refused", name: "NVIDIA GeForce RTX 3090", min: 100, max: 350 }];
    return [];
  }} />);
  fireEvent.change(screen.getByLabelText("Custom limit"), { target: { value: "50" } });
  expect(await screen.findByRole("button", { name: "Apply" })).toBeDisabled();
});

test("refused apply names the returned range and a legal value can be applied", async () => {
  const calls: Call[] = [];
  const card = primary3090();
  render(<App cards={[card]} invoke={async (cmd, args) => {
    calls.push({ cmd, args });
    if (cmd !== "plan_apply") return { ok: true, message: "Applied.", reboot_required: false };
    const value = args?.value;
    if (value === 50) return [{ kind: "refused", id: card.id, name: card.name, min: 100, max: 350 }];
    return [{ kind: "write", id: card.id, name: card.name, unit: "watts", value }];
  }} />);
  fireEvent.change(screen.getByLabelText("Custom limit"), { target: { value: "200" } });
  const apply = await screen.findByRole("button", { name: "Apply" });
  await waitFor(() => expect(apply).toBeEnabled());
  fireEvent.click(apply);
  await waitFor(() => {
    expect(calls).toContainEqual({
      cmd: "helper_request",
      args: { args: ["--apply-limit", "--id", card.id, "--unit", "watts", "--value", "200"] },
    });
  });
  fireEvent.change(screen.getByLabelText("Custom limit"), { target: { value: "50" } });
  await waitFor(() => expect(screen.getByRole("button", { name: "Apply" })).toBeDisabled());
  expect(screen.getByRole("button", { name: "Apply" })).toHaveAttribute(
    "title",
    helpText("apply-disabled", { min: 100, max: 350, unit: "watts" }),
  );
  expect(calls).toContainEqual({
    cmd: "plan_apply",
    args: { id: card.id, value: 50, unit: "watts" },
  });
});

test("reinstall groups one vendor and driver version into one INF", async () => {
  const calls: Call[] = [];
  const first = {
    ...primary3090(),
    driver_version: "32.0.16.1692",
    device_id: "PCI\\A",
    display_inf: "C:\\Windows\\INF\\nv.inf",
  };
  const second = {
    ...secondary3090(),
    driver_version: "32.0.16.1692",
    device_id: "PCI\\B",
    display_inf: "C:\\Windows\\INF\\nv.inf",
  };
  const other = {
    ...amdRaphael(),
    device_id: "PCI\\AMD",
    display_inf: "C:\\Windows\\INF\\amd.inf",
  };
  render(<App cards={[first, second, other]} invoke={async (cmd, args) => {
    calls.push({ cmd, args });
    return { ok: true, message: "Applied.", reboot_required: false };
  }} />);
  fireEvent.click(screen.getByRole("button", { name: "Reinstall nvidia 32.0.16.1692" }));
  const dialog = screen.getByRole("dialog");
  expect(dialog).toHaveTextContent("The screen will go blank.");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(calls.some((call) => call.cmd === "helper_request")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Reinstall nvidia 32.0.16.1692" }));
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  await waitFor(() => {
    expect(calls).toContainEqual({
      cmd: "helper_request",
      args: {
        args: [
          "--reinstall-driver",
          "--inf",
          "C:\\Windows\\INF\\nv.inf",
          "--device",
          "PCI\\A",
          "--device",
          "PCI\\B",
        ],
      },
    });
  });
  expect(JSON.stringify(calls)).not.toContain("PCI\\\\AMD");
});

test("rollback confirms the candidate version and date before it runs", async () => {
  const calls: Call[] = [];
  const card = { ...primary3090(), device_id: "PCI\\A", display_inf: "C:\\Windows\\INF\\nv.inf" };
  const packages: DriverPackage[] = [
    {
      class_name: "System",
      version: "1.0.0.12",
      date: "2025-09-26",
      inf_name: "amd3dvcache.inf",
      bound_device_id: "PCI\\A",
      is_previous_for_selected: true,
    },
    {
      class_name: "Display",
      version: "32.0.15.6094",
      date: "2024-08-13",
      inf_name: "nv_dispig.inf",
      bound_device_id: "PCI\\OTHER",
      is_previous_for_selected: false,
    },
    {
      class_name: "Display",
      version: "32.0.15.7000",
      date: "2025-01-02",
      inf_name: "nvddsi.inf",
      bound_device_id: "PCI\\A",
      is_previous_for_selected: true,
    },
  ];
  render(<App cards={[card]} packages={packages} invoke={async (cmd, args) => {
    calls.push({ cmd, args });
    return { ok: true, message: "Applied.", reboot_required: true };
  }} />);
  fireEvent.click(screen.getByRole("button", { name: "Roll back NVIDIA GeForce RTX 3090" }));
  const dialog = screen.getByRole("dialog");
  expect(dialog).toHaveTextContent("32.0.15.7000");
  expect(dialog).toHaveTextContent("2025-01-02");
  expect(dialog).toHaveTextContent("The screen will go blank.");
  expect(calls.some((call) => call.cmd === "helper_request")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  await waitFor(() => {
    expect(calls).toContainEqual({
      cmd: "helper_request",
      args: { args: ["--rollback-driver", "--device", "PCI\\A"] },
    });
  });
  expect(await screen.findByRole("status")).toHaveTextContent("A reboot is required");
});

test("settings toggles call the tray commands and the reapply task", async () => {
  const calls: Call[] = [];
  render(<App cards={[primary3090()]} invoke={async (cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "helper_request") return { ok: true, message: "task updated", reboot_required: false };
    return [];
  }} />);
  fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  fireEvent.click(screen.getByLabelText("start-with-Windows"));
  fireEvent.click(screen.getByLabelText("detailed debug"));
  fireEvent.click(screen.getByLabelText("reapply-at-sign-in"));
  await waitFor(() => {
    expect(calls).toContainEqual({ cmd: "set_start_with_windows", args: { enabled: false } });
    expect(calls).toContainEqual({ cmd: "set_detailed_debug", args: { enabled: true } });
    expect(calls).toContainEqual({
      cmd: "helper_request",
      args: { args: ["--install-reapply-task"] },
    });
  });
  fireEvent.click(screen.getByLabelText("reapply-at-sign-in"));
  await waitFor(() => {
    expect(calls).toContainEqual({
      cmd: "helper_request",
      args: { args: ["--remove-reapply-task"] },
    });
  });
});

test("record, export, rescan, icon mode, and graph range call their commands", async () => {
  const calls: Call[] = [];
  vi.spyOn(window, "prompt").mockReturnValue("C:\\Temp\\GpuPower-test.csv");
  render(<App cards={[primary3090()]} invoke={async (cmd, args) => {
    calls.push({ cmd, args });
    return [];
  }} />);
  fireEvent.click(screen.getByRole("button", { name: "Rescan" }));
  fireEvent.click(screen.getByRole("button", { name: "Busiest card" }));
  fireEvent.click(screen.getByRole("button", { name: "1 min" }));
  fireEvent.click(screen.getByRole("button", { name: "Record" }));
  fireEvent.click(screen.getByRole("button", { name: "Export history" }));
  fireEvent.click(screen.getByRole("button", { name: "Open log folder" }));
  const graph = document.querySelector("[data-popout='graph']");
  if (!graph) throw new Error("graph popout missing");
  fireEvent.click(graph);
  await waitFor(() => {
    expect(calls).toContainEqual({ cmd: "rescan", args: undefined });
    expect(calls).toContainEqual({ cmd: "set_icon_mode", args: { mode: "busiest" } });
    expect(calls).toContainEqual({ cmd: "set_graph_range", args: { range: "1" } });
    expect(calls).toContainEqual({
      cmd: "start_recording",
      args: { path: "C:\\Temp\\GpuPower-test.csv", format: "csv" },
    });
    expect(calls).toContainEqual({
      cmd: "export_history",
      args: { path: "C:\\Temp\\GpuPower-test.csv", format: "csv" },
    });
    expect(calls).toContainEqual({ cmd: "open_log_folder", args: undefined });
    expect(calls).toContainEqual({
      cmd: "open_popout",
      args: { kind: "graph", id: primary3090().id },
    });
  });
});

test("snapshot events replace fixture cards when the shell is present", async () => {
  const sink: { emit: ((data: unknown) => void) | null } = { emit: null };
  const shell = {
    ...primary3090(),
    name: "From the shell",
    device_id: "PCI\\LIVE",
    display_inf: "C:\\Windows\\INF\\live.inf",
  };
  const internals = {
    invoke: async (cmd: string) => {
      if (cmd === "plugin:event|listen") return 4;
      if (cmd === "snapshot") {
        return {
          cards: [
            {
              snapshot: shell,
              device_id: "PCI\\LIVE",
              display_inf: "C:\\Windows\\INF\\live.inf",
              history: [{ unix: 30, at: "10:00:30", power_draw_instant_w: 12, util_gpu_percent: 4 }],
            },
          ],
          stale_ids: [shell.id],
          graph_range: "1",
          icon_mode: "busiest",
          start_with_windows: false,
          reapply_at_sign_in: true,
          detailed_debug: true,
          recording: { format: "csv", elapsed: "00:00:03", bytes: 40, path: "C:\\log.csv" },
        };
      }
      return [];
    },
    transformCallback: (callback: (data: unknown) => void) => {
      sink.emit = callback;
      return 1;
    },
    unregisterCallback: () => undefined,
  };
  (window as unknown as { __TAURI_INTERNALS__?: typeof internals }).__TAURI_INTERNALS__ = internals;
  render(<App invoke={async () => []} />);
  expect(await screen.findByText("From the shell")).toBeInTheDocument();
  expect(screen.queryByText("NVIDIA GeForce GTX 1050")).not.toBeInTheDocument();
  expect(screen.getByText("updating")).toBeInTheDocument();
  expect(screen.getByText("Recording csv 00:00:03, 40 bytes")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Reinstall nvidia 32.0.16.1692" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "Busiest card" })).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  expect(screen.getByLabelText("reapply-at-sign-in")).toBeChecked();
  expect(screen.getByLabelText("start-with-Windows")).not.toBeChecked();
  expect(screen.getByLabelText("detailed debug")).toBeChecked();
  await act(async () => {
    sink.emit?.({
      payload: {
        cards: [{ snapshot: { ...shell, name: "After the event" }, device_id: "PCI\\LIVE", display_inf: "C:\\Windows\\INF\\live.inf" }],
        stale_ids: [],
        graph_range: "15",
        icon_mode: "combined",
      },
    });
  });
  expect(await screen.findByText("After the event")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Combined watts" })).toHaveAttribute("aria-pressed", "true");
});

afterEach(() => {
  vi.restoreAllMocks();
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});
