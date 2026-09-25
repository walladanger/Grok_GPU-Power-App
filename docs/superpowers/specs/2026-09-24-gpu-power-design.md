# GpuPower design

GpuPower is a per-user Windows 11 tray app that shows every display adapter on the machine, records live GPU readings, and sets a power limit when that card's driver accepts one. The dashboard is a borderless, resizable window. Closing it leaves the app in the tray.

This spec covers the app approved for this machine: two NVIDIA GeForce RTX 3090s, an AMD Radeon Graphics adapter, and a GeForce GTX 1050 that Windows currently reports in an error state. The app does not hard-code those four devices. It lists whatever Windows reports at runtime.

## Decisions

- The shell is Tauri 2. The dashboard is a React page using Tailwind and shadcn/ui, drawn in the Windows 11 system WebView2. The web view is created when a window opens and is not kept alive while every window is closed.
- NVIDIA readings and power limits use NVML, the library behind `nvidia-smi`. AMD readings and power limits use ADLX, loaded from the installed AMD driver. Device discovery uses Windows display adapters, including adapters in an error state.
- The hot path does not spawn PowerShell or `nvidia-smi`. One in-process sampler ticks once a second.
- The tray process stays unelevated. Setting a limit, creating the sign-in reapply task, reinstalling a display driver, and rolling a display driver back run a helper from the same executable with administrator approval.
- A power limit takes effect immediately. It is saved and written back after a driver reinstall, a driver rollback, the display driver returning, or the next sign-in. The app does not reboot Windows as part of setting a limit.
- Fan curves, clock offsets, voltage edits, named profiles, timed limits, and per-program profiles are out of this version. On the installed 3090 driver, application clocks are deprecated, and the supported NVIDIA library does not expose those other controls.
- Visual style is a smoky Vercel/shadcn surface: background `#0a0a0a`, card surface `#111111`, 1px border at 10% white, primary text `#fafafa`, secondary text `#a1a1aa`, no gradient fill, no purple accent. The UI font is Geist Sans, with Segoe UI as the fallback. NVIDIA and AMD marks are monochrome.

## What the person sees

The app starts in the notification area. The tray menu has Open, the icon mode, Open log folder, and Quit. A second launch focuses the running app and does not start another sampler.

Opening the dashboard shows one scrolling page:

- A header with combined board power, the number of display adapters, the NVIDIA driver version, the AMD driver version when an AMD adapter is present, the graph range, the tray-icon mode, Record, Export history, and Rescan. Rescan enumerates display adapters and vendor libraries immediately and keeps history for every UUID that is still present. A settings menu holds start-with-Windows, reapply-at-sign-in, and detailed debug.
- One block per display adapter. On this PC that is both 3090s, the AMD Radeon Graphics adapter, and the GTX 1050 in its error state.

Each block contains, from top to bottom:

1. Vendor mark, adapter name, PCI bus id, performance state, and status.
2. Instant board power, the one-second average, the enforced limit, a meter of draw against that limit, and the power graph.
3. The limit control, when that card can accept one.
4. A sensor grid of the readings listed below.
5. Active clock-limit reasons.
6. Processes on that card.

A reading is rendered only when the driver returns a real value. An unsupported or empty sensor is omitted. It is never shown as zero.

### Readings

These fields are requested for every adapter. NVIDIA values come from NVML. AMD values come from ADLX. Windows supplies the name, vendor id, PCI id, driver version, and device status when a vendor library does not.

| Field | Notes from this PC |
|---|---|
| Instant board power and one-second average | Both 3090s return both |
| Requested limit, enforced limit, default, minimum, maximum | 3090s are 100–350 W, default 350 W |
| AMD power-limit percent and its allowed range | Shown when ADLX reports it |
| GPU temperature | 3090s return it |
| Hotspot, memory, intake temperature | Shown when returned. Memory temperature is empty on these 3090s |
| Target, max operating, slowdown, and shutdown temperature | Read-only. These 3090s report 83 / 93 / 95 / 98 °C |
| Fan speed | 3090s return it. The label says it is the driver's intended speed |
| Graphics, shader, video, and memory clocks, plus graphics and memory maximums | 3090s return them |
| GPU, memory, encoder, decoder, JPEG, and optical-flow utilization | 3090s return them |
| VRAM used and total | Each 3090 reports 24,576 MiB |
| Voltage | Shown when returned |
| Performance state | 3090s return it |
| PCIe generation and width, current and maximum | 3090s return them |
| Encoder session count and average FPS | 3090s return them |
| Clock-limit reasons | Idle, application clocks, software power cap, hardware slowdown, hardware thermal, hardware power brake, software thermal, sync boost, display clock, board limit, reliability |
| Processes | PID, executable name, and compute/graphics type. Per-process VRAM is included when the driver returns it |
| Windows device status | The GTX 1050 currently reports Error |

Combined board power is the sum of instant board power for adapters that returned one. The header names any adapter left out of the sum.

### Power limit control

The control is shown only when the backend reports that the card can set a limit.

NVIDIA presets are 60, 70, 80, 90, and 100 percent of that card's default limit. Each preset is rounded to the nearest watt and clamped into the driver's minimum and maximum. The menu shows the resulting watts. On these 3090s, 80 percent is 280 W and the legal range is 100–350 W. A custom field accepts whole watts. A value outside the range cannot be applied, and the hover text states the legal range.

AMD presets use the same percentages across the AMD driver's allowed power-limit range. The menu shows the percentage and the watt ceiling when ADLX reports one. The custom field uses the unit ADLX accepts for that card.

Apply sets the limit, then displays the enforced value read back from the driver. Apply to all is available when two or more cards can set a limit.

- A percentage preset is applied to every capable card, each in its own range.
- A custom value is applied only to capable cards that use the same unit. Cards on another unit are left unchanged and named in the result.
- If one card fails, the cards that succeeded keep the new limit. The result names the card that failed.

The 100 percent preset is the driver default after clamping. Choosing it and applying is how a card returns to stock.

### Driver reinstall and rollback

These actions appear in the dashboard's driver section, grouped by vendor. They act on display drivers only.

Reinstall installs the display driver package already bound to the selected devices. Devices that share the same current display INF are one action. On this PC the two working 3090s share one NVIDIA INF, and the GTX 1050 and the AMD Radeon Graphics adapter each have their own. The app does not download a driver and does not delete packages from the driver store.

The elevated helper reinstalls with `pnputil /update-driver`, pointed at the current display INF and the chosen device instance ids. Before it runs, the dashboard states that the screen will go blank and asks for confirmation, then for administrator approval. The log records the driver version before and after, and the helper's exit text.

Rollback is shown for a device only when Windows has a previous display driver for that same device. The confirmation names the version and date. The elevated helper calls `DiRollbackDriver` for that device after the in-app confirmation, with no second installer wizard. The older NVIDIA package currently bound to the GTX 1050 is not offered as the rollback for a 3090 unless Windows reports that package as that 3090's previous driver. Chipset, audio, USB, network, and other non-display packages are never listed.

If Windows requires a reboot, the dashboard says so and does not reboot on its own. Saved power limits are applied at the next sign-in. If rollback is unavailable, the control explains that and does nothing.

### Windows

The main window has no title bar, can be dragged from the header, and resizes from every edge. Its bounds are remembered. The minimum size is 480 by 640. At widths below 960 the cards stack. At 960 and above they sit in equal columns, two per row, with graphs in a row sharing one height. The watt figure and the graph grow with the card, up to a 72px watt figure.

Each card, its power graph, and its sensor grid has Open in new window. The dashboard copy stays. The new window is borderless, resizable, and fed by the same sampler. A popped-out graph has a minimum size of 480 by 320. A popped-out card or sensor grid has a minimum size of 480 by 640. Closing a popped-out window closes only that window. Closing the dashboard leaves popped-out windows open. Quit from the tray closes every window and the sampler.

### Graph

The sampler keeps 15 minutes of full samples at one per second. The visible range is 1 minute, 5 minutes, or 15 minutes, starting at 5. Changing the range uses samples already stored.

The line is instant board power. GPU utilization is a second, quieter line. The vertical power scale runs from 0 to that card's enforced limit, or to the default limit when no enforced limit exists. Time is shared across every graph, including popped-out copies.

The mouse snaps to the nearest stored second. Hovering shows a vertical marker on every open graph at that timestamp and a readout with local time, instant power, one-second average, GPU utilization, clocks, temperatures, fan speed, and active limiters. Clicking pins the marker. A second click on the graph, or Escape, clears the pin. A graph with no samples yet accepts hover without inventing a point.

### Tray icon

The icon is drawn in the notification area and is updated only when the displayed number changes. The three modes are:

1. Combined instant watts.
2. Watts of the card with the highest instant draw.
3. The highest card's instant draw as a percent of its enforced limit.

The tooltip lists every card's watts, limit, and temperature, plus the recording line when a recording is active. Cards without power are named as having no power reading. When every reading is older than three seconds, the last number remains and the tooltip says the readings are updating.

### Hover help

Hovering a graph sample updates immediately with the readout above. Hovering any other control, menu item, preset, input, sensor label, or window control shows a one-sentence explanation after 500ms. A preset's explanation includes the watts or percentage it will apply on that card. A disabled control's explanation says why it is disabled. Keyboard focus shows the same text.

### Recording and export

Record writes one sample per card per second into one file. The formats are CSV, XML, and JSON Lines. The first Record or Export asks for a path. The suggested name is `GpuPower-YYYYMMDD-HHMMSS` plus the extension. While recording, the header and the tray tooltip show the format, elapsed time, and file size.

CSV and JSON Lines append and flush every sample. XML writes the document and closes the root element on Stop. If the process ends before Stop, the log says the XML file may be unclosed. A missing sensor is an empty field. Process names are included. An `error` field carries the sampler error for that card and tick when there is one. A card that is offline still gets one row per second, with its last status and the error text, so every card's timeline stays aligned.

Export history writes the samples already in memory, up to 15 minutes, in the same three formats, without starting a recording.

If a write fails because the disk is full or the path is unavailable, recording stops, monitoring continues, and the dashboard and the log say why.

CSV and JSON Lines use these fields, in this order: `timestamp_iso`, `vendor`, `name`, `uuid`, `pci_bus_id`, `status`, `power_draw_instant_w`, `power_draw_average_w`, `power_limit_w`, `power_default_w`, `power_min_w`, `power_max_w`, `power_enforced_w`, `power_limit_percent`, `temperature_gpu_c`, `temperature_hotspot_c`, `temperature_memory_c`, `temperature_target_c`, `temperature_max_operating_c`, `temperature_slowdown_c`, `temperature_shutdown_c`, `fan_speed_percent`, `clock_graphics_mhz`, `clock_sm_mhz`, `clock_video_mhz`, `clock_memory_mhz`, `clock_graphics_max_mhz`, `clock_memory_max_mhz`, `util_gpu_percent`, `util_memory_percent`, `util_encoder_percent`, `util_decoder_percent`, `util_jpeg_percent`, `util_ofa_percent`, `vram_used_mib`, `vram_total_mib`, `voltage_mv`, `pstate`, `pcie_gen_current`, `pcie_gen_max`, `pcie_width_current`, `pcie_width_max`, `encoder_sessions`, `encoder_fps`, `throttle_reasons`, `processes`, `error`.

`throttle_reasons` is a semicolon-separated list of active reason ids. `processes` is a semicolon-separated list of `pid:name`. XML uses the same fields as attributes of a `gpu` element nested in a `sample` element with a `time` attribute.

### Settings and startup

Settings live in `%APPDATA%\GpuPower\settings.json`. The file stores window bounds, popped-out window bounds, icon mode, graph range, start-with-Windows, reapply-at-sign-in, detailed debug, last recording directory, and saved limits keyed by GPU UUID. A file that cannot be parsed is renamed with a `.bad` suffix and the app opens on defaults.

The app registers itself in the current user's Run key so the tray starts at sign-in. That registration is unelevated. The setting is on by default and can be turned off.

Saved limits are reapplied by a scheduled task named `GpuPower Reapply Limits`. The task is created only after administrator approval, runs at sign-in with highest privileges, starts `GpuPower.exe --apply-saved`, and exits. The helper reads saved limits from the same `%APPDATA%\GpuPower\settings.json` as the tray app. It skips a UUID that is not present. A saved watt or percent value outside the card's current range is clamped to the nearest allowed value, and the clamp is logged. Cancelling the approval leaves the task uncreated, leaves the toggle off, keeps the limit applied for the current session, and the dashboard says the limit resets on reboot.

## Architecture

```text
tray process (unelevated)
  sampler, 1 Hz
    Windows display adapters
    NVML reader
    ADLX reader, loaded only when the AMD library is present
  history ring, 15 min, full samples
  broadcast of the latest snapshot to every open window
  log append
  recording append

GpuPower.exe --apply-saved | --apply-limit | --reinstall-driver | --rollback-driver
  elevated helper, one shot, result on stdout
  appends one log line for that invocation
```

Limit changes and driver actions are writes. The tray process does not perform them. It starts the elevated helper and waits for its stdout. The helper writes the result line. The tray writes a line only when approval is cancelled before the helper starts.

Windows vendor ids `10DE` and `1002` select the NVIDIA and AMD marks. Any other vendor is shown by adapter name without a brand mark. NVML devices match adapters by PCI bus id. ADLX adapters match the same way. An adapter with no vendor match still appears with its Windows name and status.

The NVIDIA and AMD reads are isolated. A failure in one leaves the other updating, and the next tick tries the failed library again. The sampler does not switch to an external process.

Pop-out windows and the dashboard subscribe to the same snapshot. They do not sample on their own.

## Failure behavior

- A missing NVIDIA library leaves AMD updates running and the dashboard says the NVIDIA driver is not responding. A missing ADLX library leaves an AMD adapter visible by name and mark, with sensors and the limit control hidden.
- A card that disappears freezes its graph on stored samples, shows the time of the last reading, and keeps that state for the rest of the session so a pin still works. The same state is shown in any window opened from that card. The same UUID returning continues the graph. A UUID still absent on the next launch is not shown.
- A sample older than three seconds is dimmed and labeled updating in every window that shows it.
- Cancelling an administrator prompt leaves the current limit in place and says the approval was cancelled.
- A driver refusal leaves the previous limit in place and shows the driver's text plus a fresh readback.
- The elevated helper's failure text is logged and shown. The tray app keeps running.
- If the dashboard cannot be created, the tray menu still offers Open and Quit.
- The sign-in helper and the tray process append short lines to the same log. Each line is flushed.

## Debug log

The log file is `%APPDATA%\GpuPower\logs\gpu-power.log`. A line is local time, severity (`error`, `warn`, `info`), feature name, and message. The log rolls at 2 MB and keeps three files.

These events write a line: startup, library load failure, sampler failure, a card appearing or dropping out, cancelled administrator approval, sign-in task creation, recording start and stop, export, write failure, settings load and save, window open failure, second launch, and shutdown. The helper, not the tray, writes the line for a limit readback, a sign-in reapply, a reinstall, and a rollback, including the driver versions and the value read back.

One-second samples are not logged. Detailed debug, off by default, adds vendor library result codes for failed calls. The dashboard can open the log folder. The log is not uploaded.

## Install

The installer is a per-user Tauri NSIS package. It does not require administrator approval. It uses the system WebView2 and does not ship a browser. If WebView2 is missing, the tray process still runs and Open says the Windows web view is unavailable.

## Testing

Automatic tests use a fake GPU backend and recorded snapshots of this PC: both 3090s with a 100–350 W range, the AMD Radeon Graphics adapter, and the GTX 1050 in the error state. They do not call NVML, ADLX, `pnputil`, or `DiRollbackDriver`.

The automatic suite covers:

- Preset and custom values clamp to the card's own range, and an out-of-range value cannot be applied.
- NVIDIA targets are watts. AMD targets are the AMD driver's percentage.
- Apply to all uses a percentage for every capable card, and uses a custom value only for cards with the same unit.
- Identity is the GPU UUID. A new UUID is a new row. A returning UUID keeps its history.
- The mouse index snaps to the nearest stored second. A click pins it. Every graph in the snapshot shares that timestamp.
- CSV, XML, and JSON Lines contain the fields listed above, with empty fields where a sensor is missing. A CSV or JSON Lines file that stopped mid-recording is still parseable through the last complete row.
- The log formatter emits one line for a failed sample, a cancelled approval, a limit readback, a recording failure, and a driver action, and it does not emit a line for an ordinary sample.
- Rollback candidate filtering keeps display drivers for the selected device and rejects chipset, audio, USB, and network packages. Another device's driver is rejected unless the fixture marks it as the previous driver of the selected device.
- An unreadable settings file yields defaults. The single-instance guard does not construct a second sampler.

One manual checklist is run on this PC, because these actions change live limits or blank the screen: apply a limit and read it back, cancel the administrator prompt, confirm a saved limit returns after the display driver comes back, resize the dashboard and a popped-out graph, record a few seconds and open the file, check the three tray modes and the hover text, and run one reinstall or rollback only when the operator chooses to. The automatic suite does not perform a reinstall or a rollback.

## Out of scope

Fan control, clock offsets, undervolting, memory timing, LED control, driver downloads, driver-store wipes, named power profiles, timed limits, per-program limits, and uploading logs or recordings.

## Success criteria

- With both 3090s busy, the dashboard and any open copy show each card's instant power and enforced limit, updating once a second, and the tray icon shows the selected mode.
- A new or removed display adapter appears or goes offline without a restart.
- A custom or preset limit within range is read back from the driver. A value outside range is refused.
- After sign-in, or after the display driver returns, a saved limit is applied again for each UUID that is still present.
- Record produces a CSV, an XML file, or a JSON Lines file whose fields match this spec.
- A failure in sampling, applying a limit, recording, or a driver action appends a log line and leaves the tray app running.
