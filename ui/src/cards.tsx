import { cloneElement, useEffect, useId, useRef, useState, type ReactElement, type ReactNode } from "react";
import { PowerGraph, type GraphPoint } from "./graph";
import { helpText, type HelpContext } from "./help";
import { combinedPower } from "./snapshot";
import type { AdapterSnapshot, LimitUnit } from "./snapshot";

type PopoutKind = "card" | "graph" | "sensors";

const NO_POINTS: Record<string, GraphPoint[]> = {};
const EMPTY_POINTS: GraphPoint[] = [];

function noopPopout() {}

export function Dashboard(props: {
  cards: AdapterSnapshot[];
  range: "1" | "5" | "15";
  staleIds: string[];
  onPopout?: (cardId: string, kind: PopoutKind) => void;
  pointsById?: Record<string, GraphPoint[]>;
  headerExtra?: ReactNode;
  cardExtra?: (card: AdapterSnapshot) => ReactNode;
}) {
  const combined = combinedPower(props.cards);
  const drivers = uniqueDriverVersions(props.cards);
  const stale = new Set(props.staleIds);
  const [pinnedUnix, setPinnedUnix] = useState<number | null>(null);
  const [hoverUnix, setHoverUnix] = useState<number | null>(null);
  const onPopout = props.onPopout ?? noopPopout;
  const pointsById = props.pointsById ?? NO_POINTS;
  return (
    <main className="page bg-canvas text-ink">
      <header className="page-header">
        <p className="combined" data-tauri-drag-region="">
          {combined.watts.toFixed(1)} W
        </p>
        <p className="muted">{props.cards.length} adapters</p>
        {drivers.map((version) => (
          <p key={version}>{version}</p>
        ))}
        {combined.missing_names.length > 0 ? (
          <p>no power reading: {combined.missing_names.join(", ")}</p>
        ) : null}
        <p className="muted">{props.range} min</p>
        {props.headerExtra}
      </header>
      <div className="cards" data-layout="cards" data-breakpoint="960">
        {props.cards.map((card, index) => (
          <AdapterCard
            key={`${card.id}-${index}`}
            card={card}
            stale={stale.has(card.id)}
            points={pointsById[card.id] ?? EMPTY_POINTS}
            pinnedUnix={pinnedUnix}
            hoverUnix={hoverUnix}
            onHoverUnix={setHoverUnix}
            onPinUnix={setPinnedUnix}
            onPopout={onPopout}
            extra={props.cardExtra?.(card) ?? null}
          />
        ))}
      </div>
    </main>
  );
}

function AdapterCard({
  card,
  stale,
  points,
  pinnedUnix,
  hoverUnix,
  onHoverUnix,
  onPinUnix,
  onPopout,
  extra,
}: {
  card: AdapterSnapshot;
  stale: boolean;
  points: GraphPoint[];
  pinnedUnix: number | null;
  hoverUnix: number | null;
  onHoverUnix: (unix: number | null) => void;
  onPinUnix: (unix: number | null) => void;
  onPopout: (cardId: string, kind: PopoutKind) => void;
  extra: ReactNode;
}) {
  const rows = readingRows(card);
  const instant = card.power_draw_instant_w;
  const enforced = enforcedWatts(card);
  const showFigures = instant != null || enforced != null || rows.length > 0;
  return (
    <article className="gpu-card bg-card text-ink">
      <div className="card-top">
        <VendorMark vendor={card.vendor} />
        <div>
          <h2>{card.name}</h2>
          <p className="card-id muted">{card.pci_bus_id}</p>
          {card.pstate ? <p className="muted">{card.pstate}</p> : null}
          <p className="card-status">{card.status}</p>
        </div>
      </div>
      {stale ? <p className="stale">updating</p> : null}
      {showFigures ? (
        <div className="figures" data-stale={stale ? "true" : "false"}>
          {instant != null ? <p className="watt">{watts(instant)}</p> : null}
          {instant != null && enforced != null ? (
            <div className="meter" aria-hidden="true">
              <div
                className="meter-fill"
                style={{ width: `${Math.min(100, (instant / enforced) * 100)}%` }}
              />
            </div>
          ) : null}
          {rows.length > 0 ? (
            <dl className="sensors">
              {rows.map((row) => (
                <Reading key={row.label} label={row.label} value={row.value} />
              ))}
            </dl>
          ) : null}
        </div>
      ) : null}
      <PowerGraph
        cardId={card.id}
        points={points}
        pinnedUnix={pinnedUnix}
        hoverUnix={hoverUnix}
        enforcedWatts={enforced}
        onHoverUnix={onHoverUnix}
        onPinUnix={onPinUnix}
      />
      {extra}
      <div className="flex flex-wrap gap-2">
        <PopoutButton cardId={card.id} kind="card" onPopout={onPopout} />
        <PopoutButton cardId={card.id} kind="graph" onPopout={onPopout} />
        <PopoutButton cardId={card.id} kind="sensors" onPopout={onPopout} />
      </div>
      {card.throttle_reasons.length > 0 ? (
        <ul className="chips">
          {card.throttle_reasons.map((reason, index) => (
            <li key={`${reason}-${index}`} className="chip">
              {reason}
            </li>
          ))}
        </ul>
      ) : null}
      {card.processes.map((proc, index) => (
        <p key={`${proc.pid}-${index}`} className="process">
          {proc.pid} {proc.name}
        </p>
      ))}
      {card.error ? <p>{card.error}</p> : null}
    </article>
  );
}

function PopoutButton(props: {
  cardId: string;
  kind: PopoutKind;
  onPopout: (cardId: string, kind: PopoutKind) => void;
}) {
  const id =
    props.kind === "card"
      ? "open-card-window"
      : props.kind === "graph"
        ? "open-graph-window"
        : "open-sensors-window";
  return (
    <DelayedHelp id={id}>
      <button
        type="button"
        className="rounded-full border border-white/10 px-2 py-0.5 text-xs text-ink"
        data-popout={props.kind}
        onClick={() => props.onPopout(props.cardId, props.kind)}
      >
        Open in new window
      </button>
    </DelayedHelp>
  );
}

// 500ms hover help for controls. PowerGraph sample readouts stay immediate and must not use this timer.
// Hover listens on the host so a disabled control, which does not receive pointer events, still explains itself.
export function DelayedHelp(props: { id: string; ctx?: HelpContext; children: ReactElement }) {
  const text = helpText(props.id, props.ctx);
  const tipId = useId();
  const hostRef = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const control = host.querySelector<HTMLElement>("button, a, input, select, textarea");
    let timer: ReturnType<typeof window.setTimeout> | null = null;
    let focused = false;
    let pointerInside = false;
    let hoverReady = false;
    const clearTimer = () => {
      if (timer != null) {
        window.clearTimeout(timer);
        timer = null;
      }
    };
    const sync = () => {
      setOpen(focused || (pointerInside && hoverReady));
    };
    const onMouseEnter = () => {
      pointerInside = true;
      if (!hoverReady) {
        clearTimer();
        timer = window.setTimeout(() => {
          timer = null;
          hoverReady = true;
          sync();
        }, 500);
      }
      sync();
    };
    const onMouseLeave = () => {
      pointerInside = false;
      hoverReady = false;
      clearTimer();
      sync();
    };
    const onFocus = () => {
      focused = true;
      sync();
    };
    const onBlur = () => {
      focused = false;
      sync();
    };
    host.addEventListener("mouseenter", onMouseEnter);
    host.addEventListener("mouseleave", onMouseLeave);
    control?.addEventListener("focus", onFocus);
    control?.addEventListener("blur", onBlur);
    return () => {
      clearTimer();
      host.removeEventListener("mouseenter", onMouseEnter);
      host.removeEventListener("mouseleave", onMouseLeave);
      control?.removeEventListener("focus", onFocus);
      control?.removeEventListener("blur", onBlur);
    };
  }, []);

  return (
    <span ref={hostRef} className="relative inline-flex" title={text}>
      {cloneElement(props.children, {
        title: text,
        "aria-describedby": open ? tipId : undefined,
      })}
      {open ? (
        <span
          id={tipId}
          role="tooltip"
          className="pointer-events-none absolute left-0 top-full z-10 mt-1 max-w-xs rounded border border-white/10 bg-card px-2 py-1 text-xs text-ink"
        >
          {text}
        </span>
      ) : null}
    </span>
  );
}

function Reading({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </>
  );
}

function VendorMark({ vendor }: { vendor: AdapterSnapshot["vendor"] }) {
  if (vendor === "nvidia") {
    return (
      <svg className="mark" viewBox="0 0 48 24" width="48" height="24" aria-hidden="true">
        <path
          d="M4 12C12 5 20 4 24 4C28 4 36 5 44 12C36 19 28 20 24 20C20 20 12 19 4 12Z"
          fill="none"
          stroke="#fafafa"
          strokeWidth="1.5"
        />
      </svg>
    );
  }
  if (vendor === "amd") {
    return (
      <svg className="mark" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
        <path d="M6 4L18 12L6 20" fill="none" stroke="#fafafa" strokeWidth="1.5" />
      </svg>
    );
  }
  return null;
}

function uniqueDriverVersions(cards: AdapterSnapshot[]): string[] {
  const seen = new Set<string>();
  const versions: string[] = [];
  for (const card of cards) {
    const version = card.driver_version;
    if (version && !seen.has(version)) {
      seen.add(version);
      versions.push(version);
    }
  }
  return versions;
}

function readingRows(card: AdapterSnapshot): { label: string; value: string }[] {
  const rows: { label: string; value: string | null }[] = [
    { label: "Average power", value: text(card.power_draw_average_w, watts) },
    { label: "GPU temperature", value: text(card.temperature_gpu_c, celsius) },
    { label: "Hotspot temperature", value: text(card.temperature_hotspot_c, celsius) },
    { label: "Memory temperature", value: text(card.temperature_memory_c, celsius) },
    { label: "Target temperature", value: text(card.temperature_target_c, celsius) },
    { label: "Max operating temperature", value: text(card.temperature_max_operating_c, celsius) },
    { label: "Slowdown temperature", value: text(card.temperature_slowdown_c, celsius) },
    { label: "Shutdown temperature", value: text(card.temperature_shutdown_c, celsius) },
    { label: "Fan speed, driver's intended speed", value: text(card.fan_speed_percent, percent) },
    { label: "Graphics clock", value: text(card.clock_graphics_mhz, mhz) },
    { label: "Shader clock", value: text(card.clock_sm_mhz, mhz) },
    { label: "Video clock", value: text(card.clock_video_mhz, mhz) },
    { label: "Memory clock", value: text(card.clock_memory_mhz, mhz) },
    { label: "Graphics clock max", value: text(card.clock_graphics_max_mhz, mhz) },
    { label: "Memory clock max", value: text(card.clock_memory_max_mhz, mhz) },
    { label: "GPU utilization", value: text(card.util_gpu_percent, percent) },
    { label: "Memory utilization", value: text(card.util_memory_percent, percent) },
    { label: "Encoder utilization", value: text(card.util_encoder_percent, percent) },
    { label: "Decoder utilization", value: text(card.util_decoder_percent, percent) },
    { label: "JPEG utilization", value: text(card.util_jpeg_percent, percent) },
    { label: "Optical flow utilization", value: text(card.util_ofa_percent, percent) },
    { label: "VRAM used", value: text(card.vram_used_mib, mib) },
    { label: "VRAM total", value: text(card.vram_total_mib, mib) },
    { label: "Voltage", value: text(card.voltage_mv, mv) },
    { label: "PCIe generation", value: text(card.pcie_gen_current, plain) },
    { label: "PCIe generation max", value: text(card.pcie_gen_max, plain) },
    { label: "PCIe width", value: text(card.pcie_width_current, plain) },
    { label: "PCIe width max", value: text(card.pcie_width_max, plain) },
    { label: "Encoder sessions", value: text(card.encoder_sessions, plain) },
    { label: "Encoder FPS", value: text(card.encoder_fps, plain) },
    { label: "Watt ceiling", value: text(card.watt_ceiling_w, watts) },
  ];
  const limit = card.limit;
  if (limit) {
    const unit = limit.unit;
    rows.push(
      { label: "Minimum limit", value: limitText(limit.min, unit) },
      { label: "Maximum limit", value: limitText(limit.max, unit) },
      { label: "Default limit", value: limitText(limit.default_value, unit) },
      { label: "Requested limit", value: limitText(limit.current, unit) },
      { label: "Enforced limit", value: limitText(limit.enforced, unit) },
    );
  }
  return rows.flatMap((row) => (row.value == null ? [] : [{ label: row.label, value: row.value }]));
}

function text(value: number | null, format: (value: number) => string): string | null {
  if (value == null) return null;
  return format(value);
}

function enforcedWatts(card: AdapterSnapshot): number | null {
  const limit = card.limit;
  if (!limit || limit.unit.toLowerCase() !== "watts" || limit.enforced <= 0) return null;
  return limit.enforced;
}

function limitText(value: number, unit: LimitUnit): string {
  return unit.toLowerCase() === "percent" ? `${value}%` : `${value} W`;
}

function watts(value: number): string {
  return `${trimNumber(value)} W`;
}

function celsius(value: number): string {
  return `${trimNumber(value)} °C`;
}

function percent(value: number): string {
  return `${trimNumber(value)}%`;
}

function mhz(value: number): string {
  return `${trimNumber(value)} MHz`;
}

function mib(value: number): string {
  return `${trimNumber(value)} MiB`;
}

function mv(value: number): string {
  return `${trimNumber(value)} mV`;
}

function plain(value: number): string {
  return trimNumber(value);
}

function trimNumber(value: number): string {
  return value.toFixed(2).replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, "");
}
