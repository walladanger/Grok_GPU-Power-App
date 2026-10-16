import type { AdapterSnapshot, LimitRange } from "./snapshot";

function empty(id: string, vendor: AdapterSnapshot["vendor"], name: string): AdapterSnapshot {
  return {
    id,
    vendor,
    name,
    pci_bus_id: "",
    status: "OK",
    can_set_limit: false,
    limit: null,
    power_draw_instant_w: null,
    power_draw_average_w: null,
    temperature_gpu_c: null,
    temperature_hotspot_c: null,
    temperature_memory_c: null,
    temperature_target_c: null,
    temperature_max_operating_c: null,
    temperature_slowdown_c: null,
    temperature_shutdown_c: null,
    fan_speed_percent: null,
    clock_graphics_mhz: null,
    clock_sm_mhz: null,
    clock_video_mhz: null,
    clock_memory_mhz: null,
    clock_graphics_max_mhz: null,
    clock_memory_max_mhz: null,
    util_gpu_percent: null,
    util_memory_percent: null,
    util_encoder_percent: null,
    util_decoder_percent: null,
    util_jpeg_percent: null,
    util_ofa_percent: null,
    vram_used_mib: null,
    vram_total_mib: null,
    voltage_mv: null,
    pstate: null,
    pcie_gen_current: null,
    pcie_gen_max: null,
    pcie_width_current: null,
    pcie_width_max: null,
    encoder_sessions: null,
    encoder_fps: null,
    throttle_reasons: [],
    processes: [],
    error: null,
    driver_version: null,
    watt_ceiling_w: null,
  };
}

const rtx3090Limit: LimitRange = {
  unit: "watts",
  min: 100,
  max: 350,
  default_value: 350,
  current: 350,
  enforced: 350,
};

function rtx3090(id: string, pciBusId: string, instantW: number): AdapterSnapshot {
  return {
    ...empty(id, "nvidia", "NVIDIA GeForce RTX 3090"),
    pci_bus_id: pciBusId,
    can_set_limit: true,
    limit: { ...rtx3090Limit },
    power_draw_instant_w: instantW,
  };
}

export function primary3090(): AdapterSnapshot {
  return {
    ...rtx3090("GPU-d7e740b1-1660-9a36-63d3-4c5357ed5c33", "00000000:01:00.0", 8.24),
    power_draw_average_w: 8.66,
    temperature_gpu_c: 37,
    temperature_memory_c: null,
    temperature_target_c: 83,
    temperature_max_operating_c: 93,
    temperature_slowdown_c: 95,
    temperature_shutdown_c: 98,
    fan_speed_percent: 30,
    vram_total_mib: 24576,
    driver_version: "32.0.16.1692",
    processes: [{ pid: 1234, name: "steam.exe" }],
  };
}

export function secondary3090(): AdapterSnapshot {
  return rtx3090("GPU-d5409026-7ecf-9d9a-0152-3bbde5a9675e", "00000000:03:00.0", 7.73);
}

export function amdRaphael(): AdapterSnapshot {
  return {
    ...empty(
      "PCI\\VEN_1002&DEV_164E&SUBSYS_D0001458&REV_CB\\4&16012499&0&0041",
      "amd",
      "AMD Radeon Graphics",
    ),
    driver_version: "32.0.21042.62",
  };
}

export function gtx1050(): AdapterSnapshot {
  return {
    ...empty(
      "PCI\\VEN_10DE&DEV_1C81&SUBSYS_37471458&REV_A1\\6&58735F5&0&00000011",
      "nvidia",
      "NVIDIA GeForce GTX 1050",
    ),
    status: "Error",
    driver_version: "32.0.15.6094",
  };
}
