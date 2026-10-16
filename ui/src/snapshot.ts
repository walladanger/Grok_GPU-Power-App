// Vendor strings match serde rename_all = "lowercase" on the Rust enum.
export type Vendor = "nvidia" | "amd" | "other";

// Accept the helper's lowercase names and serde's default variant names.
export type LimitUnit = "watts" | "percent" | "Watts" | "Percent";

export interface LimitRange {
  unit: LimitUnit;
  min: number;
  max: number;
  default_value: number;
  current: number;
  enforced: number;
}

export interface ProcessRow {
  pid: number;
  name: string;
}

export interface AdapterSnapshot {
  id: string;
  vendor: Vendor;
  name: string;
  pci_bus_id: string;
  status: string;
  can_set_limit: boolean;
  limit: LimitRange | null;
  power_draw_instant_w: number | null;
  power_draw_average_w: number | null;
  temperature_gpu_c: number | null;
  temperature_hotspot_c: number | null;
  temperature_memory_c: number | null;
  temperature_target_c: number | null;
  temperature_max_operating_c: number | null;
  temperature_slowdown_c: number | null;
  temperature_shutdown_c: number | null;
  fan_speed_percent: number | null;
  clock_graphics_mhz: number | null;
  clock_sm_mhz: number | null;
  clock_video_mhz: number | null;
  clock_memory_mhz: number | null;
  clock_graphics_max_mhz: number | null;
  clock_memory_max_mhz: number | null;
  util_gpu_percent: number | null;
  util_memory_percent: number | null;
  util_encoder_percent: number | null;
  util_decoder_percent: number | null;
  util_jpeg_percent: number | null;
  util_ofa_percent: number | null;
  vram_used_mib: number | null;
  vram_total_mib: number | null;
  voltage_mv: number | null;
  pstate: string | null;
  pcie_gen_current: number | null;
  pcie_gen_max: number | null;
  pcie_width_current: number | null;
  pcie_width_max: number | null;
  encoder_sessions: number | null;
  encoder_fps: number | null;
  throttle_reasons: string[];
  processes: ProcessRow[];
  error: string | null;
  driver_version: string | null;
  watt_ceiling_w: number | null;
}

export interface CombinedPower {
  watts: number;
  missing_names: string[];
}

export function combinedPower(cards: AdapterSnapshot[]): CombinedPower {
  let watts = 0;
  const missing_names: string[] = [];
  for (const card of cards) {
    if (card.power_draw_instant_w == null) {
      missing_names.push(card.name);
    } else {
      watts += card.power_draw_instant_w;
    }
  }
  return { watts, missing_names };
}
