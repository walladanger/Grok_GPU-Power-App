export const CONTROL_IDS: readonly string[] = [
  "open-dashboard",
  "quit",
  "icon-mode-combined",
  "icon-mode-busiest",
  "icon-mode-percent",
  "open-log-folder",
  "graph-range-1",
  "graph-range-5",
  "graph-range-15",
  "record",
  "export-history",
  "rescan",
  "start-with-windows",
  "reapply-at-sign-in",
  "detailed-debug",
  "open-card-window",
  "open-graph-window",
  "open-sensors-window",
  "preset-60",
  "preset-70",
  "preset-80",
  "preset-90",
  "preset-100",
  "custom-limit",
  "apply",
  "apply-disabled",
  "apply-all",
  "reinstall-display",
  "rollback-display",
  "close-window",
];

export type HelpContext = {
  defaultWatts?: number;
  min?: number;
  max?: number;
  unit?: "watts" | "percent";
};

const PRESET_PERCENT: Record<string, number> = {
  "preset-60": 60,
  "preset-70": 70,
  "preset-80": 80,
  "preset-90": 90,
  "preset-100": 100,
};

const STATIC_HELP: Record<string, string> = {
  "open-dashboard": "Open the dashboard window.",
  quit: "Quit GpuPower and close every window.",
  "icon-mode-combined": "Show combined instant watts on the tray icon.",
  "icon-mode-busiest": "Show watts for the card with the highest instant draw on the tray icon.",
  "icon-mode-percent": "Show the highest card's instant draw as a percent of its enforced limit.",
  "open-log-folder": "Open the folder that contains the debug log.",
  "graph-range-1": "Show the last one minute of stored samples.",
  "graph-range-5": "Show the last five minutes of stored samples.",
  "graph-range-15": "Show the last fifteen minutes of stored samples.",
  record: "Record one sample per card per second into a file.",
  "export-history": "Export the samples already in memory without starting a recording.",
  rescan: "Enumerate display adapters again and keep history for every card that is still present.",
  "start-with-windows": "Start GpuPower when you sign in to Windows.",
  "reapply-at-sign-in": "Reapply saved power limits the next time you sign in.",
  "detailed-debug": "Log vendor library result codes when a call fails.",
  "open-card-window": "Open this card in a new window.",
  "open-graph-window": "Open this power graph in a new window.",
  "open-sensors-window": "Open this sensor grid in a new window.",
  "custom-limit": "Enter a custom power limit in this card's unit.",
  apply: "Apply the selected limit to this card.",
  "apply-all":
    "Apply the selected percentage to every card that can set a limit. A custom value is sent only to cards that use the same unit.",
  "reinstall-display": "Reinstall the display driver already on this device. The screen will go blank.",
  "rollback-display":
    "Roll back to the previous display driver Windows has for this device. The screen will go blank.",
  "close-window": "Close this window.",
};

export function helpText(id: string, ctx?: HelpContext): string {
  const percent = PRESET_PERCENT[id];
  if (percent != null) return presetHelp(percent, ctx);
  if (id === "apply-disabled") return disabledHelp(ctx);
  return STATIC_HELP[id] ?? "This control has no further description.";
}

function presetHelp(percent: number, ctx?: HelpContext): string {
  const watts = ctx?.defaultWatts;
  if (ctx?.unit !== "percent" && typeof watts === "number" && Number.isFinite(watts)) {
    const value = Math.round(watts * percent / 100);
    return `Apply ${value} W, ${percent} percent of this card's default limit.`;
  }
  if (ctx?.unit === "percent") {
    return `Apply ${percent} percent of this card's allowed power-limit range.`;
  }
  return `Apply ${percent} percent of this card's default limit.`;
}

function disabledHelp(ctx?: HelpContext): string {
  if (ctx?.min != null && ctx.max != null && (ctx.unit === "watts" || ctx.unit === "percent")) {
    return `Apply is unavailable because the value must be from ${ctx.min} to ${ctx.max} ${ctx.unit}.`;
  }
  if (ctx?.min != null && ctx.max != null) {
    return `Apply is unavailable because the value must be from ${ctx.min} to ${ctx.max}.`;
  }
  return "Apply is unavailable because the value is outside the legal range.";
}
