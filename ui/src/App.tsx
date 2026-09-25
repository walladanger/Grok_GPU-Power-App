import { useCallback, useEffect, useId, useRef, useState } from "react";
import { Dashboard, DelayedHelp } from "./cards";
import { amdRaphael, gtx1050, primary3090, secondary3090 } from "./fixtures";
import type { GraphPoint } from "./graph";
import type { AdapterSnapshot, Vendor } from "./snapshot";

const PERCENTS = [60, 70, 80, 90, 100] as const;
const controlClass =
  "rounded-full border border-white/10 px-2 py-0.5 text-xs text-ink disabled:opacity-40";

export type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

export type DriverPackage = {
  class_name: string;
  version: string;
  date: string;
  inf_name: string;
  bound_device_id: string;
  is_previous_for_selected: boolean;
};

type HistorySample = {
  at?: string;
  unix: number;
  power_draw_instant_w?: number | null;
  power_draw_average_w?: number | null;
  util_gpu_percent?: number | null;
  temperature_gpu_c?: number | null;
  fan_speed_percent?: number | null;
  clock_graphics_mhz?: number | null;
  clock_memory_mhz?: number | null;
  throttle_reasons?: string[];
};

type PresentCard = AdapterSnapshot & {
  device_id?: string | null;
  display_inf?: string | null;
  history?: HistorySample[];
};

type PlanView = {
  kind: string;
  id?: string;
  name?: string;
  unit?: string | null;
  value?: number | null;
  min?: number | null;
  max?: number | null;
};

type ApplyTarget = { presetPercent: number } | { value: number; unit: string };

type IconModeName = "combined" | "busiest" | "highest_percent";
type RangeName = "1" | "5" | "15";
type RecordFormatName = "csv" | "xml" | "jsonl";

type RecordingInfo = {
  format: string;
  elapsed: string;
  bytes: number;
  path: string;
};

type SnapshotPayload = {
  cards: unknown[];
  stale_ids?: string[];
  graph_range?: string;
  icon_mode?: string;
  start_with_windows?: boolean;
  reapply_at_sign_in?: boolean;
  detailed_debug?: boolean;
  recording?: RecordingInfo | null;
  packages?: DriverPackage[];
};

type ReinstallGroup = {
  vendor: string;
  driverVersion: string;
  inf: string;
  deviceIds: string[];
  cards: PresentCard[];
};

type PendingConfirm =
  | { kind: "reinstall"; text: string; inf: string; deviceIds: string[] }
  | { kind: "rollback"; text: string; deviceId: string };

type TauriInternals = {
  invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  transformCallback: (callback: (data: unknown) => void, once?: boolean) => number;
  unregisterCallback: (id: number) => void;
};

const UNAVAILABLE_ROLLBACK =
  "Rollback is unavailable because Windows has no previous display driver for this device.";

export function App(props: { invoke?: Invoke; cards?: PresentCard[]; packages?: DriverPackage[] }) {
  const call = useInvoke(props.invoke);
  const live = useLiveSnapshot();
  const [inShell] = useState(() => tauriInternals() != null);
  const [fallbackCards] = useState<PresentCard[]>(() => props.cards ?? previewCards());
  // Fixture cards are the test and dev stand-in. A real shell waits for `snapshot`.
  const cards = live ? cardsFromPayload(live) : inShell ? [] : (props.cards ?? fallbackCards);
  const packages = live?.packages?.length ? live.packages : (props.packages ?? []);
  const [range, setRange] = useState<RangeName>("5");
  const [iconMode, setIconMode] = useState<IconModeName>("combined");
  const [startWithWindows, setStartWithWindows] = useState(true);
  const [reapply, setReapply] = useState(false);
  const [detailed, setDetailed] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [recordFormat, setRecordFormat] = useState<RecordFormatName>("csv");
  const [recording, setRecording] = useState<RecordingInfo | null>(null);
  const [notice, setNotice] = useState("");
  const [confirm, setConfirm] = useState<PendingConfirm | null>(null);
  const [shared, setShared] = useState<ApplyTarget | null>(null);

  useEffect(() => {
    if (!live) return;
    if (live.graph_range === "1" || live.graph_range === "5" || live.graph_range === "15") {
      setRange(live.graph_range);
    }
    if (
      live.icon_mode === "combined" ||
      live.icon_mode === "busiest" ||
      live.icon_mode === "highest_percent"
    ) {
      setIconMode(live.icon_mode);
    }
    if (typeof live.start_with_windows === "boolean") setStartWithWindows(live.start_with_windows);
    if (typeof live.reapply_at_sign_in === "boolean") setReapply(live.reapply_at_sign_in);
    if (typeof live.detailed_debug === "boolean") setDetailed(live.detailed_debug);
    setRecording(live.recording ?? null);
  }, [live]);

  const capable = cards.filter((card) => card.can_set_limit);
  const groups = reinstallGroups(cards);

  async function runHelper(args: string[]) {
    try {
      const report = await call("helper_request", { args });
      const message = reportMessage(report);
      if (message) setNotice(message);
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function onIcon(mode: IconModeName) {
    try {
      await call("set_icon_mode", { mode });
      setIconMode(mode);
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function onRange(next: RangeName) {
    setRange(next);
    try {
      await call("set_graph_range", { range: next });
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function onStart(enabled: boolean) {
    try {
      await call("set_start_with_windows", { enabled });
      setStartWithWindows(enabled);
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function onReapply(enabled: boolean) {
    try {
      const report = await call("helper_request", {
        args: [enabled ? "--install-reapply-task" : "--remove-reapply-task"],
      });
      setReapply(enabled);
      const message = reportMessage(report);
      if (message) setNotice(message);
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function onDetailed(enabled: boolean) {
    try {
      await call("set_detailed_debug", { enabled });
      setDetailed(enabled);
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function onRecord() {
    if (recording) {
      try {
        await call("stop_recording");
        setRecording(null);
      } catch (err) {
        setNotice(errorText(err));
      }
      return;
    }
    const path = window.prompt("Choose a path for the recording", suggestedRecordingName(recordFormat));
    if (!path?.trim()) return;
    try {
      await call("start_recording", { path: path.trim(), format: recordFormat });
      setRecording({ format: recordFormat, elapsed: "00:00:00", bytes: 0, path: path.trim() });
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function onExport() {
    const path = window.prompt("Choose a path for the export", suggestedRecordingName(recordFormat));
    if (!path?.trim()) return;
    try {
      await call("export_history", { path: path.trim(), format: recordFormat });
      setNotice(`Exported history to ${path.trim()}`);
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function onRescan() {
    try {
      await call("rescan");
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function onLogFolder() {
    try {
      await call("open_log_folder");
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  async function applyAll() {
    if (!shared) return;
    try {
      const args = "presetPercent" in shared ? { presetPercent: shared.presetPercent } : shared;
      const plans = asPlans(await call("plan_apply", args));
      const skipped = plans.filter((plan) => plan.kind === "skipped").map((plan) => plan.name ?? plan.id ?? "");
      for (const plan of plans) {
        if (plan.kind !== "write" || plan.value == null || !plan.unit || !plan.id) continue;
        await runHelper([
          "--apply-limit",
          "--id",
          plan.id,
          "--unit",
          plan.unit,
          "--value",
          String(plan.value),
        ]);
      }
      const named = skipped.filter((name) => name.length > 0);
      if (named.length > 0) {
        setNotice((prev) => `${prev ? `${prev} ` : ""}Left unchanged: ${named.join(", ")}`);
      }
    } catch (err) {
      setNotice(errorText(err));
    }
  }

  function openReinstall(group: ReinstallGroup) {
    if (!group.inf || group.deviceIds.length === 0) return;
    const listed = group.deviceIds.join(", ");
    setConfirm({
      kind: "reinstall",
      inf: group.inf,
      deviceIds: group.deviceIds,
      text: `Reinstall the display driver already on this device for ${listed} using ${group.inf}. The screen will go blank.`,
    });
  }

  function openRollback(card: PresentCard, candidate: DriverPackage) {
    const deviceId = deviceIdOf(card);
    setConfirm({
      kind: "rollback",
      deviceId,
      text: `Roll back ${card.name} to version ${candidate.version} dated ${candidate.date}. The screen will go blank.`,
    });
  }

  async function acceptConfirm() {
    const pending = confirm;
    if (!pending) return;
    setConfirm(null);
    if (pending.kind === "reinstall") {
      const args = ["--reinstall-driver", "--inf", pending.inf];
      for (const id of pending.deviceIds) args.push("--device", id);
      await runHelper(args);
      return;
    }
    await runHelper(["--rollback-driver", "--device", pending.deviceId]);
  }

  const header = (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <IconButton mode="combined" current={iconMode} label="Combined watts" onPick={onIcon} />
        <IconButton mode="busiest" current={iconMode} label="Busiest card" onPick={onIcon} />
        <IconButton mode="highest_percent" current={iconMode} label="Highest percent" onPick={onIcon} />
        <RangeButton range="1" current={range} onPick={onRange} />
        <RangeButton range="5" current={range} onPick={onRange} />
        <RangeButton range="15" current={range} onPick={onRange} />
        <label className="flex items-center gap-2 text-xs">
          Recording format
          <select
            className="rounded border border-white/10 bg-canvas px-2 py-1 text-ink"
            value={recordFormat}
            onChange={(event) => setRecordFormat(event.target.value as RecordFormatName)}
          >
            <option value="csv">CSV</option>
            <option value="xml">XML</option>
            <option value="jsonl">JSON Lines</option>
          </select>
        </label>
        {recording ? (
          <button
            type="button"
            className={controlClass}
            title="Stop the recording and close the file."
            onClick={() => void onRecord()}
          >
            Stop recording
          </button>
        ) : (
          <DelayedHelp id="record">
            <button type="button" className={controlClass} onClick={() => void onRecord()}>
              Record
            </button>
          </DelayedHelp>
        )}
        <DelayedHelp id="export-history">
          <button type="button" className={controlClass} onClick={() => void onExport()}>
            Export history
          </button>
        </DelayedHelp>
        <DelayedHelp id="rescan">
          <button type="button" className={controlClass} onClick={() => void onRescan()}>
            Rescan
          </button>
        </DelayedHelp>
        <DelayedHelp id="open-log-folder">
          <button type="button" className={controlClass} onClick={() => void onLogFolder()}>
            Open log folder
          </button>
        </DelayedHelp>
        <button
          type="button"
          className={controlClass}
          aria-expanded={settingsOpen}
          aria-controls="settings-menu"
          onClick={() => setSettingsOpen((open) => !open)}
        >
          Settings
        </button>
        {capable.length >= 2 ? (
          <DelayedHelp id="apply-all">
            <button
              type="button"
              className={controlClass}
              disabled={!shared}
              style={!shared ? { pointerEvents: "none" } : undefined}
              onClick={() => void applyAll()}
            >
              Apply to all
            </button>
          </DelayedHelp>
        ) : null}
      </div>
      {recording ? (
        <p>
          Recording {recording.format} {recording.elapsed}, {recording.bytes} bytes
        </p>
      ) : null}
      {settingsOpen ? (
        <div id="settings-menu" role="menu" className="flex flex-col gap-2 text-sm">
          <SettingToggle
            id="start-with-windows"
            label="start-with-Windows"
            checked={startWithWindows}
            onChange={onStart}
          />
          <SettingToggle
            id="reapply-at-sign-in"
            label="reapply-at-sign-in"
            checked={reapply}
            onChange={onReapply}
          />
          <SettingToggle id="detailed-debug" label="detailed debug" checked={detailed} onChange={onDetailed} />
        </div>
      ) : null}
      {notice ? (
        <p role="status" className="text-sm">
          {notice}
        </p>
      ) : null}
      <section className="flex flex-col gap-2">
        <h2 className="text-sm font-medium">Display drivers</h2>
        {groups.map((group) => {
          const versionLabel = group.driverVersion || "unknown";
          const reinstallName = `Reinstall ${group.vendor} ${versionLabel}`;
          return (
            <div key={`${group.vendor}-${versionLabel}`} className="flex flex-wrap items-center gap-2">
              {group.inf ? (
                <DelayedHelp id="reinstall-display">
                  <button type="button" className={controlClass} onClick={() => openReinstall(group)}>
                    {reinstallName}
                  </button>
                </DelayedHelp>
              ) : (
                <span className="inline-flex" title="The display INF for this driver is not available.">
                  <button type="button" className={controlClass} disabled style={{ pointerEvents: "none" }}>
                    {reinstallName}
                  </button>
                </span>
              )}
              {group.cards.map((card) => {
                const deviceId = deviceIdOf(card);
                const candidate = rollbackCandidate(deviceId, packages);
                const name = rollbackName(card, cards);
                if (!candidate) {
                  return (
                    <span key={deviceId} className="inline-flex" title={UNAVAILABLE_ROLLBACK}>
                      <button type="button" className={controlClass} disabled style={{ pointerEvents: "none" }}>
                        {name}
                      </button>
                    </span>
                  );
                }
                return (
                  <DelayedHelp key={deviceId} id="rollback-display">
                    <button type="button" className={controlClass} onClick={() => openRollback(card, candidate)}>
                      {name}
                    </button>
                  </DelayedHelp>
                );
              })}
            </div>
          );
        })}
      </section>
    </div>
  );

  return (
    <>
      <Dashboard
        cards={cards}
        range={range}
        staleIds={live?.stale_ids ?? []}
        pointsById={pointsFor(cards, range)}
        headerExtra={header}
        onPopout={(cardId, kind) => {
          void call("open_popout", { kind, id: cardId }).catch((err) => setNotice(errorText(err)));
        }}
        cardExtra={(card) => {
          if (!card.can_set_limit) return null;
          const present = card as PresentCard;
          const capableIndex = capable.indexOf(present);
          return (
            <CardLimits
              card={present}
              capableIndex={capableIndex < 0 ? 0 : capableIndex}
              call={call}
              onNotice={setNotice}
              onTarget={setShared}
            />
          );
        }}
      />
      {confirm ? (
        <div
          role="dialog"
          aria-modal="true"
          aria-label="Confirm driver change"
          className="fixed inset-0 z-20 flex items-center justify-center bg-black/60 p-6"
        >
          <div className="max-w-md rounded border border-white/10 bg-card p-4 text-ink">
            <p>{confirm.text}</p>
            <div className="mt-3 flex gap-2">
              <button type="button" className={controlClass} onClick={() => void acceptConfirm()}>
                Continue
              </button>
              <button type="button" className={controlClass} onClick={() => setConfirm(null)}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}

function CardLimits(props: {
  card: PresentCard;
  capableIndex: number;
  call: Invoke;
  onNotice: (text: string) => void;
  onTarget: (target: ApplyTarget) => void;
}) {
  const inputId = useId();
  const generation = useRef(0);
  const [draft, setDraft] = useState("");
  const [preset, setPreset] = useState<number | null>(null);
  const [plans, setPlans] = useState<PlanView[]>([]);
  const distinguished = props.capableIndex > 0;
  const bus = props.card.pci_bus_id || props.card.id;
  const unit = limitUnit(props.card);
  const refusal = findRefusal(plans, props.card);
  const write = findWrite(plans, props.card);
  const disabled = refusal != null || write == null;
  const helpId = refusal && refusal.min != null && refusal.max != null ? "apply-disabled" : "apply";
  const helpCtx =
    refusal && refusal.min != null && refusal.max != null
      ? { min: refusal.min, max: refusal.max, unit: disabledUnit(props.card, refusal) }
      : undefined;

  async function onCustom(raw: string) {
    setDraft(raw);
    setPreset(null);
    const parsed = parseWhole(raw);
    const token = ++generation.current;
    if (parsed == null) {
      setPlans([]);
      return;
    }
    const limit = props.card.limit;
    if (limit && (parsed < limit.min || parsed > limit.max)) {
      setPlans([
        {
          kind: "refused",
          id: props.card.id,
          name: props.card.name,
          min: limit.min,
          max: limit.max,
          unit,
        },
      ]);
    } else {
      setPlans([]);
    }
    props.onTarget({ value: parsed, unit });
    try {
      const result = asPlans(
        await props.call("plan_apply", { id: props.card.id, value: parsed, unit }),
      );
      if (generation.current !== token) return;
      setPlans(result);
    } catch (err) {
      if (generation.current !== token) return;
      props.onNotice(errorText(err));
    }
  }

  async function onPreset(percent: number) {
    setPreset(percent);
    setDraft("");
    const token = ++generation.current;
    props.onTarget({ presetPercent: percent });
    try {
      const result = asPlans(
        await props.call("plan_apply", { id: props.card.id, presetPercent: percent }),
      );
      if (generation.current !== token) return;
      setPlans(result);
    } catch (err) {
      if (generation.current !== token) return;
      props.onNotice(errorText(err));
    }
  }

  async function apply() {
    if (!write || write.value == null || !write.unit) return;
    const id = write.id || props.card.id;
    try {
      const report = await props.call("helper_request", {
        args: ["--apply-limit", "--id", id, "--unit", write.unit, "--value", String(write.value)],
      });
      const message = reportMessage(report);
      if (message) props.onNotice(message);
    } catch (err) {
      props.onNotice(errorText(err));
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap gap-2">
        {PERCENTS.map((percent) => (
          <DelayedHelp
            key={percent}
            id={`preset-${percent}`}
            ctx={{
              defaultWatts: props.card.limit?.default_value,
              min: props.card.limit?.min,
              max: props.card.limit?.max,
              unit,
            }}
          >
            <button
              type="button"
              className={controlClass}
              aria-pressed={preset === percent}
              onClick={() => void onPreset(percent)}
            >
              {presetLabel(props.card, percent)}
            </button>
          </DelayedHelp>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <label htmlFor={inputId} className="text-xs">
          Custom limit
          {distinguished ? <span className="sr-only"> {bus}</span> : null}
        </label>
        <DelayedHelp id="custom-limit">
          <input
            id={inputId}
            className="w-24 rounded border border-white/10 bg-canvas px-2 py-1 text-sm text-ink"
            inputMode="numeric"
            value={draft}
            onChange={(event) => void onCustom(event.target.value)}
          />
        </DelayedHelp>
        <DelayedHelp id={helpId} ctx={helpCtx}>
          <button
            type="button"
            className={controlClass}
            disabled={disabled}
            aria-label={distinguished ? `Apply ${bus}` : undefined}
            style={disabled ? { pointerEvents: "none" } : undefined}
            onClick={() => void apply()}
          >
            Apply
          </button>
        </DelayedHelp>
      </div>
    </div>
  );
}

function IconButton(props: {
  mode: IconModeName;
  current: IconModeName;
  label: string;
  onPick: (mode: IconModeName) => void;
}) {
  const id =
    props.mode === "combined"
      ? "icon-mode-combined"
      : props.mode === "busiest"
        ? "icon-mode-busiest"
        : "icon-mode-percent";
  return (
    <DelayedHelp id={id}>
      <button
        type="button"
        className={controlClass}
        aria-pressed={props.current === props.mode}
        onClick={() => props.onPick(props.mode)}
      >
        {props.label}
      </button>
    </DelayedHelp>
  );
}

function RangeButton(props: { range: RangeName; current: RangeName; onPick: (range: RangeName) => void }) {
  return (
    <DelayedHelp id={`graph-range-${props.range}`}>
      <button
        type="button"
        className={controlClass}
        aria-pressed={props.current === props.range}
        onClick={() => props.onPick(props.range)}
      >
        {props.range} min
      </button>
    </DelayedHelp>
  );
}

function SettingToggle(props: {
  id: string;
  label: string;
  checked: boolean;
  onChange: (enabled: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-2">
      <DelayedHelp id={props.id}>
        <input
          type="checkbox"
          checked={props.checked}
          onChange={(event) => props.onChange(event.target.checked)}
        />
      </DelayedHelp>
      {props.label}
    </label>
  );
}

function useInvoke(invoke: Invoke | undefined): Invoke {
  return useCallback(
    (cmd, args) => {
      if (invoke) return invoke(cmd, args);
      const internals = tauriInternals();
      if (!internals) return Promise.resolve(undefined);
      return internals.invoke(cmd, args ?? {});
    },
    [invoke],
  );
}

function useLiveSnapshot(): SnapshotPayload | null {
  const [live, setLive] = useState<SnapshotPayload | null>(null);
  useEffect(() => {
    const internals = tauriInternals();
    if (!internals) return;
    const watch = { cancelled: false, eventId: null as number | null };
    const handler = internals.transformCallback((data: unknown) => {
      const payload = payloadFromEvent(data);
      if (payload) setLive(payload);
    });
    void (async () => {
      try {
        const id = await internals.invoke("plugin:event|listen", {
          event: "snapshot",
          target: { kind: "Any" },
          handler,
        });
        const eventId = typeof id === "number" ? id : Number(id);
        if (!Number.isFinite(eventId)) return;
        watch.eventId = eventId;
        if (watch.cancelled) {
          unlistenSnapshot(internals, eventId);
          return;
        }
        const current = payloadFromEvent(await internals.invoke("snapshot"));
        if (!watch.cancelled && current) setLive(current);
      } catch {
        if (!watch.cancelled) internals.unregisterCallback(handler);
      }
    })();
    return () => {
      watch.cancelled = true;
      if (watch.eventId != null) unlistenSnapshot(internals, watch.eventId);
      else internals.unregisterCallback(handler);
    };
  }, []);
  return live;
}

function tauriInternals(): TauriInternals | null {
  const internals = (window as Window & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__;
  if (!internals?.invoke || !internals.transformCallback || !internals.unregisterCallback) return null;
  return internals;
}

function unlistenSnapshot(internals: TauriInternals, eventId: number) {
  const plugin = (
    window as Window & {
      __TAURI_EVENT_PLUGIN_INTERNALS__?: { unregisterListener?: (event: string, eventId: number) => void };
    }
  ).__TAURI_EVENT_PLUGIN_INTERNALS__;
  plugin?.unregisterListener?.("snapshot", eventId);
  void internals.invoke("plugin:event|unlisten", { event: "snapshot", eventId }).catch(() => undefined);
}

function payloadFromEvent(data: unknown): SnapshotPayload | null {
  if (!data || typeof data !== "object") return null;
  const record = data as { payload?: unknown; cards?: unknown };
  if (Array.isArray(record.cards)) return record as SnapshotPayload;
  if (record.payload && typeof record.payload === "object") {
    const inner = record.payload as { cards?: unknown };
    if (Array.isArray(inner.cards)) return inner as SnapshotPayload;
  }
  return null;
}

function cardsFromPayload(payload: SnapshotPayload): PresentCard[] {
  const cards: PresentCard[] = [];
  for (const item of payload.cards) {
    const card = asPresentCard(item);
    if (card) cards.push(card);
  }
  return cards;
}

function asPresentCard(item: unknown): PresentCard | null {
  if (!item || typeof item !== "object") return null;
  const row = item as {
    snapshot?: AdapterSnapshot;
    device_id?: string | null;
    display_inf?: string | null;
    history?: HistorySample[];
  };
  const source = row.snapshot ?? (item as AdapterSnapshot);
  if (!source || typeof source.id !== "string" || typeof source.name !== "string") return null;
  const present = source as PresentCard;
  return {
    ...present,
    vendor: normalizeVendor(source.vendor),
    throttle_reasons: source.throttle_reasons ?? [],
    processes: source.processes ?? [],
    can_set_limit: Boolean(source.can_set_limit),
    device_id: row.device_id ?? present.device_id ?? null,
    display_inf: row.display_inf ?? present.display_inf ?? null,
    history: Array.isArray(row.history) ? row.history : present.history,
  };
}

function normalizeVendor(value: unknown): Vendor {
  const text = String(value ?? "").toLowerCase();
  if (text === "nvidia" || text === "amd" || text === "other") return text;
  return "other";
}

function pointsFor(cards: PresentCard[], range: RangeName): Record<string, GraphPoint[]> {
  const windowSec = Number(range) * 60;
  const pointsById: Record<string, GraphPoint[]> = {};
  for (const card of cards) {
    const mapped = (card.history ?? []).filter((sample) => typeof sample.unix === "number").map(toPoint);
    if (mapped.length === 0) continue;
    const newest = mapped[mapped.length - 1].unix;
    pointsById[card.id] = mapped.filter((point) => newest - point.unix <= windowSec);
  }
  return pointsById;
}

function toPoint(sample: HistorySample): GraphPoint {
  const parts = [sample.at ?? ""];
  if (sample.power_draw_instant_w != null) parts.push(`${sample.power_draw_instant_w} W`);
  if (sample.power_draw_average_w != null) parts.push(`avg ${sample.power_draw_average_w} W`);
  if (sample.util_gpu_percent != null) parts.push(`${sample.util_gpu_percent}% GPU`);
  if (sample.clock_graphics_mhz != null) parts.push(`${sample.clock_graphics_mhz} MHz graphics`);
  if (sample.clock_memory_mhz != null) parts.push(`${sample.clock_memory_mhz} MHz memory`);
  if (sample.temperature_gpu_c != null) parts.push(`${sample.temperature_gpu_c} °C`);
  if (sample.fan_speed_percent != null) parts.push(`fan ${sample.fan_speed_percent}%`);
  if (sample.throttle_reasons && sample.throttle_reasons.length > 0) {
    parts.push(sample.throttle_reasons.join(", "));
  }
  return {
    unix: sample.unix,
    at: sample.at ?? "",
    power_w: sample.power_draw_instant_w ?? null,
    util_gpu: sample.util_gpu_percent ?? null,
    detail: parts.filter((part) => part.length > 0).join(", "),
  };
}

function previewCards(): PresentCard[] {
  return [primary3090(), secondary3090(), amdRaphael(), gtx1050()];
}

// Same vendor and display-driver version share one INF and every device id in the group.
export function reinstallGroups(cards: PresentCard[]): ReinstallGroup[] {
  const groups = new Map<string, ReinstallGroup>();
  for (const card of cards) {
    const driverVersion = card.driver_version ?? "";
    const key = `${card.vendor}\0${driverVersion}`;
    const deviceId = deviceIdOf(card);
    let group = groups.get(key);
    if (!group) {
      group = { vendor: card.vendor, driverVersion, inf: card.display_inf ?? "", deviceIds: [], cards: [] };
      groups.set(key, group);
    }
    if (!group.inf && card.display_inf) group.inf = card.display_inf;
    if (!group.deviceIds.includes(deviceId)) group.deviceIds.push(deviceId);
    group.cards.push(card);
  }
  return [...groups.values()];
}

// Display class, marked previous for this device, and bound to the selected device id.
export function rollbackCandidate(selectedDeviceId: string, packages: DriverPackage[]): DriverPackage | null {
  return (
    packages.find(
      (pkg) =>
        pkg.class_name.toLowerCase() === "display" &&
        pkg.is_previous_for_selected &&
        pkg.bound_device_id === selectedDeviceId,
    ) ?? null
  );
}

function deviceIdOf(card: PresentCard): string {
  return card.device_id || card.id;
}

function rollbackName(card: PresentCard, cards: PresentCard[]): string {
  const duplicates = cards.filter((item) => item.name === card.name).length > 1;
  if (!duplicates) return `Roll back ${card.name}`;
  return `Roll back ${card.name} ${card.pci_bus_id || deviceIdOf(card)}`;
}

function limitUnit(card: PresentCard): "watts" | "percent" {
  return card.limit?.unit?.toLowerCase() === "percent" ? "percent" : "watts";
}

function disabledUnit(card: PresentCard, plan: PlanView): "watts" | "percent" {
  const fromPlan = (plan.unit ?? "").toLowerCase();
  if (fromPlan === "percent") return "percent";
  if (fromPlan === "watts") return "watts";
  return limitUnit(card);
}

function presetLabel(card: PresentCard, percent: number): string {
  const limit = card.limit;
  if (!limit || limitUnit(card) === "percent") return `${percent}%`;
  return `${percent}% ${Math.round((limit.default_value * percent) / 100)} W`;
}

function parseWhole(raw: string): number | null {
  const text = raw.trim();
  if (!/^-?\d+$/.test(text)) return null;
  const value = Number(text);
  return Number.isSafeInteger(value) ? value : null;
}

function asPlans(value: unknown): PlanView[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (!item || typeof item !== "object") return [];
    const plan = item as PlanView;
    return typeof plan.kind === "string" ? [plan] : [];
  });
}

function planMatches(plan: PlanView, card: PresentCard): boolean {
  if (plan.id) return plan.id === card.id;
  if (plan.name) return plan.name === card.name;
  return true;
}

function findRefusal(plans: PlanView[], card: PresentCard): PlanView | null {
  return plans.find((plan) => plan.kind === "refused" && planMatches(plan, card)) ?? null;
}

function findWrite(plans: PlanView[], card: PresentCard): PlanView | null {
  return (
    plans.find(
      (plan) =>
        plan.kind === "write" &&
        planMatches(plan, card) &&
        plan.value != null &&
        typeof plan.unit === "string" &&
        plan.unit.length > 0,
    ) ?? null
  );
}

function reportMessage(value: unknown): string {
  if (typeof value === "string") return value;
  if (!value || typeof value !== "object") return "";
  const report = value as { message?: unknown; reboot_required?: unknown };
  const message = typeof report.message === "string" ? report.message : "";
  if (report.reboot_required === true && message && !/reboot/i.test(message)) {
    return `${message} A reboot is required. GpuPower will not reboot the PC.`;
  }
  if (report.reboot_required === true && !message) {
    return "A reboot is required. GpuPower will not reboot the PC.";
  }
  return message;
}

function errorText(err: unknown): string {
  if (typeof err === "string" && err.trim()) return err;
  if (err && typeof err === "object" && "message" in err) {
    const message = (err as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) return message;
  }
  return "The request failed.";
}

function suggestedRecordingName(format: RecordFormatName): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `GpuPower-${stamp}.${format}`;
}
