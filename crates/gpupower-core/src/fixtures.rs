use crate::{empty_snapshot, AdapterSnapshot, LimitRange, LimitUnit, Vendor};

fn rtx3090(id: &str, pci_bus_id: &str, instant_w: f64) -> AdapterSnapshot {
    let mut card = empty_snapshot(id.into(), Vendor::Nvidia, "NVIDIA GeForce RTX 3090".into());
    card.pci_bus_id = pci_bus_id.into();
    card.can_set_limit = true;
    card.limit = Some(LimitRange {
        unit: LimitUnit::Watts,
        min: 100,
        max: 350,
        default_value: 350,
        current: 350,
        enforced: 350,
    });
    card.power_draw_instant_w = Some(instant_w);
    card
}

pub fn rtx3090_primary() -> AdapterSnapshot {
    let mut card = rtx3090(
        "GPU-d7e740b1-1660-9a36-63d3-4c5357ed5c33",
        "00000000:01:00.0",
        8.24,
    );
    card.power_draw_average_w = Some(8.66);
    card.temperature_gpu_c = Some(37.0);
    card.temperature_memory_c = None;
    card.temperature_target_c = Some(83.0);
    card.temperature_max_operating_c = Some(93.0);
    card.temperature_slowdown_c = Some(95.0);
    card.temperature_shutdown_c = Some(98.0);
    card.fan_speed_percent = Some(30.0);
    card.vram_total_mib = Some(24576.0);
    card.driver_version = Some("32.0.16.1692".into());
    card
}

pub fn rtx3090_secondary() -> AdapterSnapshot {
    rtx3090(
        "GPU-d5409026-7ecf-9d9a-0152-3bbde5a9675e",
        "00000000:03:00.0",
        7.73,
    )
}

pub fn amd_raphael() -> AdapterSnapshot {
    let mut card = empty_snapshot(
        "PCI\\VEN_1002&DEV_164E&SUBSYS_D0001458&REV_CB\\4&16012499&0&0041".into(),
        Vendor::Amd,
        "AMD Radeon Graphics".into(),
    );
    card.driver_version = Some("32.0.21042.62".into());
    card
}

pub fn gtx1050_error() -> AdapterSnapshot {
    let mut card = empty_snapshot(
        "PCI\\VEN_10DE&DEV_1C81&SUBSYS_37471458&REV_A1\\6&58735F5&0&00000011".into(),
        Vendor::Nvidia,
        "NVIDIA GeForce GTX 1050".into(),
    );
    card.status = "Error".into();
    card.driver_version = Some("32.0.15.6094".into());
    card
}
