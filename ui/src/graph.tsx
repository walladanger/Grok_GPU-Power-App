import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";

export type GraphPoint = {
  unix: number;
  at: string;
  power_w: number | null;
  util_gpu: number | null;
  detail: string;
};

const VIEW_W = 100;
const VIEW_H = 40;

// Lower index wins an equal distance, matching history::snap_index.
export function snapPoint(points: GraphPoint[], unix: number): GraphPoint | null {
  let bestIndex = -1;
  let bestDistance = Infinity;
  for (let index = 0; index < points.length; index += 1) {
    const distance = Math.abs(points[index].unix - unix);
    if (distance < bestDistance) {
      bestDistance = distance;
      bestIndex = index;
    }
  }
  return bestIndex < 0 ? null : points[bestIndex];
}

export function PowerGraph(props: {
  cardId: string;
  points: GraphPoint[];
  pinnedUnix: number | null;
  onHoverUnix: (unix: number | null) => void;
  onPinUnix: (unix: number | null) => void;
  enforcedWatts?: number | null;
  hoverUnix?: number | null;
}) {
  const [localHover, setLocalHover] = useState<number | null>(null);
  const hoverRef = useRef<number | null>(null);
  const pinnedPoint = props.pinnedUnix == null ? null : snapPoint(props.points, props.pinnedUnix);
  const hoverSource = localHover ?? props.hoverUnix ?? null;
  const hoverPoint = hoverSource == null ? null : snapPoint(props.points, hoverSource);
  const shown = pinnedPoint ?? hoverPoint;
  const scale = powerScaleMax(props.points, props.enforcedWatts);
  const marker = shown == null ? -1 : props.points.indexOf(shown);
  const markerX = marker < 0 ? "0" : trimCoord(indexX(marker, props.points.length));

  function locate(event: MouseEvent<HTMLDivElement>): number | null {
    const rect = event.currentTarget.getBoundingClientRect();
    return pointerUnix(props.points, event.clientX, rect.left, rect.width);
  }

  function onMouseMove(event: MouseEvent<HTMLDivElement>) {
    const unix = locate(event);
    hoverRef.current = unix;
    setLocalHover(unix);
    props.onHoverUnix(unix);
  }

  function onMouseLeave() {
    hoverRef.current = null;
    setLocalHover(null);
    props.onHoverUnix(null);
  }

  function onClick(event: MouseEvent<HTMLDivElement>) {
    const unix = hoverRef.current ?? locate(event);
    if (unix == null) return;
    props.onPinUnix(unix === props.pinnedUnix ? null : unix);
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") props.onPinUnix(null);
  }

  useEffect(() => {
    function onWindowKey(event: globalThis.KeyboardEvent) {
      if (event.key === "Escape") props.onPinUnix(null);
    }
    window.addEventListener("keydown", onWindowKey);
    return () => window.removeEventListener("keydown", onWindowKey);
  }, [props.onPinUnix]);

  return (
    <div
      className="power-graph w-full"
      data-testid={`graph-${props.cardId}`}
      aria-label="Power graph"
      tabIndex={0}
      onMouseMove={onMouseMove}
      onMouseLeave={onMouseLeave}
      onClick={onClick}
      onKeyDown={onKeyDown}
    >
      <svg viewBox={`0 0 ${VIEW_W} ${VIEW_H}`} width="100%" height="48" aria-hidden="true">
        <polyline
          data-series="power"
          fill="none"
          stroke="#fafafa"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
          points={seriesPoints(props.points, (point) => point.power_w, scale)}
        />
        <polyline
          data-series="util"
          fill="none"
          stroke="#fafafa"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
          opacity="0.4"
          points={seriesPoints(props.points, (point) => point.util_gpu, 100)}
        />
        {marker >= 0 ? (
          <line
            x1={markerX}
            x2={markerX}
            y1="0"
            y2={VIEW_H}
            stroke="#fafafa"
            strokeOpacity="0.35"
          />
        ) : null}
      </svg>
      {shown ? (
        <p className="muted" data-testid={pinnedPoint ? `pin-${props.cardId}` : undefined}>
          {shown.detail}
        </p>
      ) : null}
    </div>
  );
}

function pointerUnix(points: GraphPoint[], clientX: number, left: number, width: number): number | null {
  if (points.length === 0) return null;
  const lastIndex = points.length - 1;
  // A zero-width box, which is what jsdom reports, is the left edge.
  if (lastIndex === 0 || width <= 0) return snapPoint(points, points[0].unix)?.unix ?? null;
  const ratio = Math.min(1, Math.max(0, (clientX - left) / width));
  const fractional = ratio * lastIndex;
  const rounded = Math.round(fractional);
  const clampedRound = Math.min(lastIndex, Math.max(0, Number.isFinite(rounded) ? rounded : 0));
  const lower = Math.min(lastIndex, Math.max(0, Math.floor(fractional)));
  const upper = Math.min(lastIndex, lower + 1);
  const rawUnix = points[lower].unix + (fractional - lower) * (points[upper].unix - points[lower].unix);
  // Math.round(fractional) lands on the later sample at a halfway pointer.
  // The linear unix keeps that pointer equidistant so snapPoint can pick the earlier one.
  const snapped = snapPoint(points, Number.isFinite(rawUnix) ? rawUnix : points[clampedRound].unix);
  return snapped?.unix ?? null;
}

function powerScaleMax(points: GraphPoint[], enforcedWatts: number | null | undefined): number {
  if (enforcedWatts != null && enforcedWatts > 0) return enforcedWatts;
  let max = 0;
  for (const point of points) {
    if (point.power_w != null && point.power_w > max) max = point.power_w;
  }
  return max > 0 ? max : 1;
}

function seriesPoints(
  points: GraphPoint[],
  read: (point: GraphPoint) => number | null,
  max: number,
): string {
  const safe = max > 0 ? max : 1;
  const parts: string[] = [];
  for (let index = 0; index < points.length; index += 1) {
    const value = read(points[index]);
    if (value == null || !Number.isFinite(value)) continue;
    const x = indexX(index, points.length);
    const clamped = Math.min(Math.max(value, 0), safe);
    const y = VIEW_H - (clamped / safe) * VIEW_H;
    parts.push(`${trimCoord(x)},${trimCoord(y)}`);
  }
  return parts.join(" ");
}

function indexX(index: number, count: number): number {
  if (count <= 1) return 0;
  return (index / (count - 1)) * VIEW_W;
}

function trimCoord(value: number): string {
  return (Math.round(value * 100) / 100).toString();
}
