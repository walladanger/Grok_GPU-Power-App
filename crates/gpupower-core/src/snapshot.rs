use serde::Serialize;

use crate::LimitRange;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Nvidia,
    Amd,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdapterSnapshot {
    pub id: String,
    pub vendor: Vendor,
    pub name: String,
    pub pci_bus_id: String,
    pub status: String,
    pub can_set_limit: bool,
    pub limit: Option<LimitRange>,
    pub power_draw_instant_w: Option<f64>,
    pub power_draw_average_w: Option<f64>,
    pub temperature_gpu_c: Option<f64>,
    pub temperature_hotspot_c: Option<f64>,
    pub temperature_memory_c: Option<f64>,
    pub temperature_target_c: Option<f64>,
    pub temperature_max_operating_c: Option<f64>,
    pub temperature_slowdown_c: Option<f64>,
    pub temperature_shutdown_c: Option<f64>,
    pub fan_speed_percent: Option<f64>,
    pub clock_graphics_mhz: Option<f64>,
    pub clock_sm_mhz: Option<f64>,
    pub clock_video_mhz: Option<f64>,
    pub clock_memory_mhz: Option<f64>,
    pub clock_graphics_max_mhz: Option<f64>,
    pub clock_memory_max_mhz: Option<f64>,
    pub util_gpu_percent: Option<f64>,
    pub util_memory_percent: Option<f64>,
    pub util_encoder_percent: Option<f64>,
    pub util_decoder_percent: Option<f64>,
    pub util_jpeg_percent: Option<f64>,
    pub util_ofa_percent: Option<f64>,
    pub vram_used_mib: Option<f64>,
    pub vram_total_mib: Option<f64>,
    pub voltage_mv: Option<f64>,
    pub pstate: Option<String>,
    pub pcie_gen_current: Option<i32>,
    pub pcie_gen_max: Option<i32>,
    pub pcie_width_current: Option<i32>,
    pub pcie_width_max: Option<i32>,
    pub encoder_sessions: Option<i32>,
    pub encoder_fps: Option<f64>,
    pub throttle_reasons: Vec<String>,
    pub processes: Vec<ProcessRow>,
    pub error: Option<String>,
    pub driver_version: Option<String>,
    pub watt_ceiling_w: Option<f64>,
}

pub fn empty_snapshot(id: String, vendor: Vendor, name: String) -> AdapterSnapshot {
    AdapterSnapshot {
        id,
        vendor,
        name,
        pci_bus_id: String::new(),
        status: "OK".into(),
        can_set_limit: false,
        limit: None,
        power_draw_instant_w: None,
        power_draw_average_w: None,
        temperature_gpu_c: None,
        temperature_hotspot_c: None,
        temperature_memory_c: None,
        temperature_target_c: None,
        temperature_max_operating_c: None,
        temperature_slowdown_c: None,
        temperature_shutdown_c: None,
        fan_speed_percent: None,
        clock_graphics_mhz: None,
        clock_sm_mhz: None,
        clock_video_mhz: None,
        clock_memory_mhz: None,
        clock_graphics_max_mhz: None,
        clock_memory_max_mhz: None,
        util_gpu_percent: None,
        util_memory_percent: None,
        util_encoder_percent: None,
        util_decoder_percent: None,
        util_jpeg_percent: None,
        util_ofa_percent: None,
        vram_used_mib: None,
        vram_total_mib: None,
        voltage_mv: None,
        pstate: None,
        pcie_gen_current: None,
        pcie_gen_max: None,
        pcie_width_current: None,
        pcie_width_max: None,
        encoder_sessions: None,
        encoder_fps: None,
        throttle_reasons: Vec::new(),
        processes: Vec::new(),
        error: None,
        driver_version: None,
        watt_ceiling_w: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_snapshot() {
        let snapshot = empty_snapshot("id1".into(), Vendor::Nvidia, "name1".into());

        assert_eq!(snapshot.id, "id1");
        assert_eq!(snapshot.vendor, Vendor::Nvidia);
        assert_eq!(snapshot.name, "name1");
        assert_eq!(snapshot.pci_bus_id, String::new());
        assert_eq!(snapshot.status, "OK");
        assert!(!snapshot.can_set_limit);
        assert_eq!(snapshot.limit, None);
        assert_eq!(snapshot.power_draw_instant_w, None);
        assert_eq!(snapshot.power_draw_average_w, None);
        assert_eq!(snapshot.temperature_gpu_c, None);
        assert_eq!(snapshot.temperature_hotspot_c, None);
        assert_eq!(snapshot.temperature_memory_c, None);
        assert_eq!(snapshot.temperature_target_c, None);
        assert_eq!(snapshot.temperature_max_operating_c, None);
        assert_eq!(snapshot.temperature_slowdown_c, None);
        assert_eq!(snapshot.temperature_shutdown_c, None);
        assert_eq!(snapshot.fan_speed_percent, None);
        assert_eq!(snapshot.clock_graphics_mhz, None);
        assert_eq!(snapshot.clock_sm_mhz, None);
        assert_eq!(snapshot.clock_video_mhz, None);
        assert_eq!(snapshot.clock_memory_mhz, None);
        assert_eq!(snapshot.clock_graphics_max_mhz, None);
        assert_eq!(snapshot.clock_memory_max_mhz, None);
        assert_eq!(snapshot.util_gpu_percent, None);
        assert_eq!(snapshot.util_memory_percent, None);
        assert_eq!(snapshot.util_encoder_percent, None);
        assert_eq!(snapshot.util_decoder_percent, None);
        assert_eq!(snapshot.util_jpeg_percent, None);
        assert_eq!(snapshot.util_ofa_percent, None);
        assert_eq!(snapshot.vram_used_mib, None);
        assert_eq!(snapshot.vram_total_mib, None);
        assert_eq!(snapshot.voltage_mv, None);
        assert_eq!(snapshot.pstate, None);
        assert_eq!(snapshot.pcie_gen_current, None);
        assert_eq!(snapshot.pcie_gen_max, None);
        assert_eq!(snapshot.pcie_width_current, None);
        assert_eq!(snapshot.pcie_width_max, None);
        assert_eq!(snapshot.encoder_sessions, None);
        assert_eq!(snapshot.encoder_fps, None);
        assert_eq!(snapshot.throttle_reasons, Vec::<String>::new());
        assert_eq!(snapshot.processes, Vec::<ProcessRow>::new());
        assert_eq!(snapshot.error, None);
        assert_eq!(snapshot.driver_version, None);
        assert_eq!(snapshot.watt_ceiling_w, None);
    }

    #[test]
    fn test_vendor_serde() {
        assert_eq!(
            serde_json::to_string(&Vendor::Nvidia).unwrap(),
            "\"nvidia\""
        );
        assert_eq!(serde_json::to_string(&Vendor::Amd).unwrap(), "\"amd\"");
        assert_eq!(serde_json::to_string(&Vendor::Other).unwrap(), "\"other\"");
    }

    #[test]
    fn test_clone_and_partialeq() {
        let process_row = ProcessRow {
            pid: 123,
            name: "test".into(),
        };
        assert_eq!(process_row.clone(), process_row);

        let mut snapshot = empty_snapshot("id1".into(), Vendor::Nvidia, "name1".into());
        snapshot.processes.push(process_row.clone());

        let cloned_snapshot = snapshot.clone();
        assert_eq!(snapshot, cloned_snapshot);
    }
}
