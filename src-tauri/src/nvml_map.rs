use gpupower_core::{
    empty_snapshot, AdapterSnapshot, LimitRange, LimitUnit, ProcessRow, Vendor, VendorSource,
};
use nvml_wrapper::bitmasks::device::ThrottleReasons;
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor, TemperatureThreshold};
use nvml_wrapper::enums::device::SampleValue;
use nvml_wrapper::structs::device::FieldId;
use nvml_wrapper::sys_exports::field_id::{NVML_FI_DEV_MEMORY_TEMP, NVML_FI_DEV_POWER_AVERAGE};
use nvml_wrapper::Device;
use nvml_wrapper::Nvml;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};

// nvml-wrapper 0.11 stops at GPU max. This is the acoustic/target threshold.
const NVML_TEMPERATURE_THRESHOLD_ACOUSTIC_CURR: u32 = 5;

#[derive(Clone, Debug, Default)]
pub struct RawNvidia {
    pub uuid: String,
    pub name: String,
    pub pci_bus_id: String,
    pub power_instant_mw: Option<u32>,
    pub power_average_mw: Option<u32>,
    pub limit_mw: Option<u32>,
    pub default_mw: Option<u32>,
    pub min_mw: Option<u32>,
    pub max_mw: Option<u32>,
    pub enforced_mw: Option<u32>,
    pub temp_gpu_c: Option<f64>,
    pub temp_memory_c: Option<f64>,
    pub temp_target_c: Option<u32>,
    pub temp_max_operating_c: Option<u32>,
    pub temp_slowdown_c: Option<u32>,
    pub temp_shutdown_c: Option<u32>,
    pub fan_percent: Option<f64>,
    pub clock_graphics_mhz: Option<u32>,
    pub clock_sm_mhz: Option<u32>,
    pub clock_memory_mhz: Option<u32>,
    pub clock_video_mhz: Option<u32>,
    pub clock_graphics_max_mhz: Option<u32>,
    pub clock_memory_max_mhz: Option<u32>,
    pub util_gpu: Option<u32>,
    pub util_memory: Option<u32>,
    pub util_encoder: Option<u32>,
    pub util_decoder: Option<u32>,
    pub vram_used_mib: Option<u64>,
    pub vram_total_mib: Option<u64>,
    pub pstate: Option<String>,
    pub pcie_gen_current: Option<i32>,
    pub pcie_gen_max: Option<i32>,
    pub pcie_width_current: Option<i32>,
    pub pcie_width_max: Option<i32>,
    pub encoder_sessions: Option<i32>,
    pub encoder_fps: Option<u32>,
    pub throttle_reasons: Vec<String>,
    pub processes: Vec<ProcessRow>,
    pub driver_version: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct RawAmd {
    pub id: String,
    pub name: String,
    pub pci_bus_id: String,
    pub power_w: Option<f64>,
    pub limit_percent: Option<i32>,
    pub limit_min_percent: Option<i32>,
    pub limit_max_percent: Option<i32>,
    pub watt_ceiling_w: Option<f64>,
    pub temp_gpu_c: Option<f64>,
    pub can_set_limit: bool,
}

pub fn snapshot_from_nvml(raw: &RawNvidia) -> AdapterSnapshot {
    let mut snap = empty_snapshot(raw.uuid.clone(), Vendor::Nvidia, raw.name.clone());
    snap.pci_bus_id = normalize_pci_bus_id(&raw.pci_bus_id);
    snap.power_draw_instant_w = raw.power_instant_mw.map(mw_to_w);
    snap.power_draw_average_w = raw.power_average_mw.map(mw_to_w);
    snap.limit = nvidia_limit(raw);
    snap.can_set_limit = raw.min_mw.is_some() && raw.max_mw.is_some() && raw.default_mw.is_some();
    snap.temperature_gpu_c = raw.temp_gpu_c;
    snap.temperature_memory_c = raw.temp_memory_c;
    snap.temperature_target_c = raw.temp_target_c.map(f64::from);
    snap.temperature_max_operating_c = raw.temp_max_operating_c.map(f64::from);
    snap.temperature_slowdown_c = raw.temp_slowdown_c.map(f64::from);
    snap.temperature_shutdown_c = raw.temp_shutdown_c.map(f64::from);
    snap.fan_speed_percent = raw.fan_percent;
    snap.clock_graphics_mhz = raw.clock_graphics_mhz.map(f64::from);
    snap.clock_sm_mhz = raw.clock_sm_mhz.map(f64::from);
    snap.clock_memory_mhz = raw.clock_memory_mhz.map(f64::from);
    snap.clock_video_mhz = raw.clock_video_mhz.map(f64::from);
    snap.clock_graphics_max_mhz = raw.clock_graphics_max_mhz.map(f64::from);
    snap.clock_memory_max_mhz = raw.clock_memory_max_mhz.map(f64::from);
    snap.util_gpu_percent = raw.util_gpu.map(f64::from);
    snap.util_memory_percent = raw.util_memory.map(f64::from);
    snap.util_encoder_percent = raw.util_encoder.map(f64::from);
    snap.util_decoder_percent = raw.util_decoder.map(f64::from);
    snap.vram_used_mib = raw.vram_used_mib.map(|mib| mib as f64);
    snap.vram_total_mib = raw.vram_total_mib.map(|mib| mib as f64);
    snap.pstate = raw.pstate.clone();
    snap.pcie_gen_current = raw.pcie_gen_current;
    snap.pcie_gen_max = raw.pcie_gen_max;
    snap.pcie_width_current = raw.pcie_width_current;
    snap.pcie_width_max = raw.pcie_width_max;
    snap.encoder_sessions = raw.encoder_sessions;
    snap.encoder_fps = raw.encoder_fps.map(f64::from);
    snap.throttle_reasons = raw.throttle_reasons.clone();
    snap.processes = raw.processes.clone();
    snap.driver_version = raw.driver_version.clone();
    snap
}

pub fn snapshot_from_adlx(raw: &RawAmd) -> AdapterSnapshot {
    let mut snap = empty_snapshot(raw.id.clone(), Vendor::Amd, raw.name.clone());
    snap.pci_bus_id = normalize_pci_bus_id(&raw.pci_bus_id);
    snap.power_draw_instant_w = raw.power_w;
    snap.temperature_gpu_c = raw.temp_gpu_c;
    snap.watt_ceiling_w = raw.watt_ceiling_w;
    snap.can_set_limit = raw.can_set_limit;
    snap.limit = amd_limit(raw);
    snap
}

pub fn normalize_pci_bus_id(raw: &str) -> String {
    let raw = raw.trim().trim_matches(char::from(0));
    let mut parts = raw.split(':');
    let domain = parts.next().unwrap_or("");
    let bus = parts.next().unwrap_or("");
    let dev_fn = parts.next().unwrap_or("");
    let mut dev_fn_parts = dev_fn.split('.');
    let device = dev_fn_parts.next().unwrap_or("");
    let function = dev_fn_parts.next().unwrap_or("0");
    match (
        u32::from_str_radix(domain, 16),
        u32::from_str_radix(bus, 16),
        u32::from_str_radix(device, 16),
        u32::from_str_radix(function, 16),
    ) {
        (Ok(domain), Ok(bus), Ok(device), Ok(function)) => {
            format!("{domain:08x}:{bus:02x}:{device:02x}.{function:x}")
        }
        _ => raw.to_ascii_lowercase(),
    }
}

fn mw_to_w(mw: u32) -> f64 {
    f64::from(mw) / 1000.0
}

fn mw_to_watts_i32(mw: u32) -> i32 {
    (f64::from(mw) / 1000.0).round() as i32
}

fn nvidia_limit(raw: &RawNvidia) -> Option<LimitRange> {
    let min = mw_to_watts_i32(raw.min_mw?);
    let max = mw_to_watts_i32(raw.max_mw?);
    let default_value = mw_to_watts_i32(raw.default_mw?);
    let current = raw.limit_mw.map(mw_to_watts_i32).unwrap_or(default_value);
    let enforced = raw.enforced_mw.map(mw_to_watts_i32).unwrap_or(current);
    Some(LimitRange {
        unit: LimitUnit::Watts,
        min,
        max,
        default_value,
        current,
        enforced,
    })
}

fn amd_limit(raw: &RawAmd) -> Option<LimitRange> {
    Some(LimitRange {
        unit: LimitUnit::Percent,
        min: raw.limit_min_percent?,
        max: raw.limit_max_percent?,
        default_value: 0,
        current: raw.limit_percent?,
        enforced: raw.limit_percent?,
    })
}

pub struct NvidiaSource {
    nvml: Option<Nvml>,
    pub last_code: Option<String>,
}

impl Default for NvidiaSource {
    fn default() -> Self {
        Self {
            nvml: None,
            last_code: None,
        }
    }
}

impl VendorSource for NvidiaSource {
    fn sample(&mut self) -> Result<Vec<AdapterSnapshot>, String> {
        self.last_code = None;
        if self.nvml.is_none() {
            match Nvml::init() {
                Ok(nvml) => self.nvml = Some(nvml),
                Err(err) => {
                    self.last_code = Some(format!("{err:?}"));
                    return Err("The NVIDIA driver is not responding.".into());
                }
            }
        }
        // The handle stays for the process lifetime. Shutting NVML down per tick is forbidden.
        let read = {
            let nvml = self
                .nvml
                .as_ref()
                .expect("nvml handle is kept after the first successful init");
            read_nvidia_devices(nvml)
        };
        match read {
            Ok(raws) => Ok(raws.iter().map(snapshot_from_nvml).collect()),
            Err(err) => {
                self.last_code = Some(err);
                Err("The NVIDIA driver is not responding.".into())
            }
        }
    }
}

fn read_nvidia_devices(nvml: &Nvml) -> Result<Vec<RawNvidia>, String> {
    let count = nvml.device_count().map_err(|err| format!("{err:?}"))?;
    let driver_version = nvml.sys_driver_version().ok();
    let mut raws = Vec::with_capacity(count as usize);
    for index in 0..count {
        let Ok(device) = nvml.device_by_index(index) else {
            continue;
        };
        raws.push(read_raw_nvidia(&device, index, driver_version.clone()));
    }
    Ok(raws)
}

fn read_raw_nvidia(device: &Device<'_>, index: u32, driver_version: Option<String>) -> RawNvidia {
    let uuid = device.uuid().unwrap_or_else(|_| format!("nvidia-{index}"));
    let name = device
        .name()
        .unwrap_or_else(|_| format!("NVIDIA GPU {index}"));
    let pci_bus_id = device
        .pci_info()
        .map(|info| info.bus_id)
        .unwrap_or_default();
    let constraints = device.power_management_limit_constraints().ok();
    let encoder = device.encoder_stats().ok();
    let mut raw = RawNvidia {
        uuid,
        name,
        pci_bus_id,
        power_instant_mw: device.power_usage().ok(),
        power_average_mw: field_u32(device, NVML_FI_DEV_POWER_AVERAGE),
        limit_mw: device.power_management_limit().ok(),
        default_mw: device.power_management_limit_default().ok(),
        min_mw: constraints.as_ref().map(|limits| limits.min_limit),
        max_mw: constraints.as_ref().map(|limits| limits.max_limit),
        enforced_mw: device.enforced_power_limit().ok(),
        temp_gpu_c: device
            .temperature(TemperatureSensor::Gpu)
            .ok()
            .map(f64::from),
        temp_memory_c: field_u32(device, NVML_FI_DEV_MEMORY_TEMP).map(f64::from),
        temp_target_c: acoustic_target_c(device),
        temp_max_operating_c: device
            .temperature_threshold(TemperatureThreshold::GpuMax)
            .ok(),
        temp_slowdown_c: device
            .temperature_threshold(TemperatureThreshold::Slowdown)
            .ok(),
        temp_shutdown_c: device
            .temperature_threshold(TemperatureThreshold::Shutdown)
            .ok(),
        fan_percent: device.fan_speed(0).ok().map(f64::from),
        clock_graphics_mhz: device.clock_info(Clock::Graphics).ok(),
        clock_sm_mhz: device.clock_info(Clock::SM).ok(),
        clock_memory_mhz: device.clock_info(Clock::Memory).ok(),
        clock_video_mhz: device.clock_info(Clock::Video).ok(),
        clock_graphics_max_mhz: device.max_clock_info(Clock::Graphics).ok(),
        clock_memory_max_mhz: device.max_clock_info(Clock::Memory).ok(),
        util_gpu: None,
        util_memory: None,
        util_encoder: device
            .encoder_utilization()
            .ok()
            .map(|info| info.utilization),
        util_decoder: device
            .decoder_utilization()
            .ok()
            .map(|info| info.utilization),
        vram_used_mib: None,
        vram_total_mib: None,
        pstate: device.performance_state().ok().map(pstate_label),
        pcie_gen_current: device
            .current_pcie_link_gen()
            .ok()
            .and_then(|gen| i32::try_from(gen).ok()),
        pcie_gen_max: device
            .max_pcie_link_gen()
            .ok()
            .and_then(|gen| i32::try_from(gen).ok()),
        pcie_width_current: device
            .current_pcie_link_width()
            .ok()
            .and_then(|width| i32::try_from(width).ok()),
        pcie_width_max: device
            .max_pcie_link_width()
            .ok()
            .and_then(|width| i32::try_from(width).ok()),
        encoder_sessions: encoder
            .as_ref()
            .and_then(|stats| i32::try_from(stats.session_count).ok()),
        encoder_fps: encoder.as_ref().map(|stats| stats.average_fps),
        throttle_reasons: device
            .current_throttle_reasons()
            .map(throttle_names)
            .unwrap_or_default(),
        processes: device_processes(device),
        driver_version,
    };
    if let Ok(util) = device.utilization_rates() {
        raw.util_gpu = Some(util.gpu);
        raw.util_memory = Some(util.memory);
    }
    if let Ok(memory) = device.memory_info() {
        raw.vram_used_mib = Some(memory.used / (1024 * 1024));
        raw.vram_total_mib = Some(memory.total / (1024 * 1024));
    }
    raw
}

fn acoustic_target_c(device: &Device<'_>) -> Option<u32> {
    type GetThreshold = unsafe extern "C" fn(*mut std::ffi::c_void, u32, *mut u32) -> u32;
    unsafe {
        let module = GetModuleHandleW(wide("nvml.dll").as_ptr());
        if module.is_null() {
            return None;
        }
        let Some(symbol) = GetProcAddress(module, b"nvmlDeviceGetTemperatureThreshold\0".as_ptr())
        else {
            return None;
        };
        let get_threshold: GetThreshold = std::mem::transmute(symbol);
        let mut temp = 0u32;
        let status = get_threshold(
            device.handle().cast::<std::ffi::c_void>(),
            NVML_TEMPERATURE_THRESHOLD_ACOUSTIC_CURR,
            &mut temp,
        );
        if status == 0 {
            Some(temp)
        } else {
            None
        }
    }
}

fn field_u32(device: &Device<'_>, id: u32) -> Option<u32> {
    let samples = device.field_values_for(&[FieldId(id)]).ok()?;
    let sample = samples.into_iter().next()?.ok()?;
    match sample.value.ok()? {
        SampleValue::U32(value) => Some(value),
        SampleValue::U64(value) => u32::try_from(value).ok(),
        SampleValue::I64(value) => u32::try_from(value).ok(),
        SampleValue::F64(value) if value.is_finite() && value >= 0.0 => Some(value.round() as u32),
        _ => None,
    }
}

fn pstate_label(state: nvml_wrapper::enum_wrappers::device::PerformanceState) -> String {
    let name = format!("{state:?}");
    const NAMES: [&str; 16] = [
        "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
        "Eleven", "Twelve", "Thirteen", "Fourteen", "Fifteen",
    ];
    NAMES
        .iter()
        .position(|item| *item == name)
        .map(|index| format!("P{index}"))
        .unwrap_or(name)
}

fn throttle_names(reasons: ThrottleReasons) -> Vec<String> {
    let flags = [
        (ThrottleReasons::GPU_IDLE, "gpu_idle"),
        (
            ThrottleReasons::APPLICATIONS_CLOCKS_SETTING,
            "applications_clocks_setting",
        ),
        (ThrottleReasons::SW_POWER_CAP, "sw_power_cap"),
        (ThrottleReasons::HW_SLOWDOWN, "hw_slowdown"),
        (ThrottleReasons::SW_THERMAL_SLOWDOWN, "sw_thermal_slowdown"),
        (ThrottleReasons::HW_THERMAL_SLOWDOWN, "hw_thermal_slowdown"),
        (
            ThrottleReasons::HW_POWER_BRAKE_SLOWDOWN,
            "hw_power_brake_slowdown",
        ),
        (
            ThrottleReasons::DISPLAY_CLOCK_SETTING,
            "display_clock_setting",
        ),
        (ThrottleReasons::SYNC_BOOST, "sync_boost"),
    ];
    flags
        .into_iter()
        .filter(|(flag, _)| reasons.contains(*flag))
        .map(|(_, name)| name.to_string())
        .collect()
}

fn device_processes(device: &Device<'_>) -> Vec<ProcessRow> {
    let mut rows = Vec::new();
    if let Ok(list) = device.running_compute_processes() {
        push_processes(&mut rows, &list);
    }
    if let Ok(list) = device.running_graphics_processes() {
        push_processes(&mut rows, &list);
    }
    rows
}

fn push_processes(
    rows: &mut Vec<ProcessRow>,
    list: &[nvml_wrapper::struct_wrappers::device::ProcessInfo],
) {
    for process in list {
        if rows.iter().any(|row| row.pid == process.pid) {
            continue;
        }
        rows.push(ProcessRow {
            pid: process.pid,
            name: process_name(process.pid),
        });
    }
}

fn process_name(pid: u32) -> String {
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 32768];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        Path::new(&full)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or(full)
    }
}

pub struct AmdSource {
    library: usize,
    announced: bool,
    pub notice: Option<String>,
}

impl Default for AmdSource {
    fn default() -> Self {
        Self {
            library: 0,
            announced: false,
            notice: None,
        }
    }
}

impl VendorSource for AmdSource {
    fn sample(&mut self) -> Result<Vec<AdapterSnapshot>, String> {
        if self.library == 0 {
            match load_adlx() {
                Some(handle) => {
                    self.library = handle;
                    if !self.announced {
                        self.announced = true;
                        self.notice = Some(
                            "ADLX runtime loaded, but adapter sensors are unavailable because the ADLX vtable is not bound."
                                .into(),
                        );
                    }
                }
                None => {
                    if !self.announced {
                        self.announced = true;
                        self.notice = Some("ADLX runtime is not installed.".into());
                    }
                    return Ok(Vec::new());
                }
            }
        }
        Ok(query_adlx_adapters(self.library))
    }
}

impl AmdSource {
    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }
}

fn load_adlx() -> Option<usize> {
    let name = wide("amdadlx64.dll");
    let handle = unsafe { windows_sys::Win32::System::LibraryLoader::LoadLibraryW(name.as_ptr()) };
    if handle.is_null() {
        None
    } else {
        Some(handle as usize)
    }
}

// ADLX ships a C++ vtable. Calling it without the SDK headers is not safe.
// An empty list lets Windows merge keep the adapter name and mark.
fn query_adlx_adapters(_library: usize) -> Vec<AdapterSnapshot> {
    Vec::new()
}

pub fn write_nvidia_watts(id: &str, watts: i32) -> Result<u32, String> {
    let nvml = Nvml::init().map_err(|_| "The NVIDIA driver is not responding.".to_string())?;
    let count = nvml
        .device_count()
        .map_err(|_| "The NVIDIA driver is not responding.".to_string())?;
    for index in 0..count {
        let mut device = nvml
            .device_by_index(index)
            .map_err(|err| format!("{err:?}"))?;
        let uuid = device.uuid().map_err(|err| format!("{err:?}"))?;
        if uuid != id {
            continue;
        }
        let mw = u32::try_from(watts)
            .map_err(|_| format!("watt value {watts} is out of range"))?
            .saturating_mul(1000);
        device
            .set_power_management_limit(mw)
            .map_err(|err| format!("{err:?}"))?;
        return device
            .enforced_power_limit()
            .map_err(|err| format!("{err:?}"));
    }
    Err(format!("NVIDIA card {id} is not present"))
}

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{launch_action, mutex_name, LaunchAction};

    #[test]
    fn milliwatts_become_watts_and_second_launch_focuses() {
        let snap = snapshot_from_nvml(&RawNvidia {
            uuid: "GPU-a".into(),
            name: "NVIDIA GeForce RTX 3090".into(),
            pci_bus_id: "00000000:01:00.0".into(),
            power_instant_mw: Some(8240),
            power_average_mw: Some(8660),
            limit_mw: Some(350_000),
            default_mw: Some(350_000),
            min_mw: Some(100_000),
            max_mw: Some(350_000),
            enforced_mw: Some(350_000),
            temp_gpu_c: Some(37.0),
            ..RawNvidia::default()
        });
        assert!((snap.power_draw_instant_w.unwrap() - 8.24).abs() < 0.001);
        assert!(snap.temperature_memory_c.is_none());
        assert!(snap.can_set_limit);
        let amd = snapshot_from_adlx(&RawAmd {
            id: "amd".into(),
            name: "AMD Radeon Graphics".into(),
            pci_bus_id: "00000000:0c:00.0".into(),
            power_w: Some(22.0),
            limit_percent: Some(0),
            limit_min_percent: Some(-30),
            limit_max_percent: Some(15),
            watt_ceiling_w: Some(80.0),
            temp_gpu_c: Some(48.0),
            can_set_limit: true,
        });
        assert!(matches!(
            amd.limit.unwrap().unit,
            gpupower_core::LimitUnit::Percent
        ));
        assert!(matches!(launch_action(true), LaunchAction::FocusExisting));
        assert_eq!(mutex_name(), "Local\\GpuPower");
    }
}
