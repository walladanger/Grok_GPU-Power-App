#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod nvml_map;
mod state;

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpupower_core::{
    approval_cancelled_message, combined_power, csv_header, csv_row, default_settings, follow_up,
    format_helper_stdout, format_log_line, icon_number, is_stale, jsonl_line, load_settings,
    parse_helper_args, partial_apply_message, plan_all, plan_reapply, reconcile, sample_tick_line,
    should_rotate, AdapterSnapshot, CardLimitInput, CardPlan, GraphRange, HelperCommand,
    HelperReport, IconMode, LimitRange, LimitUnit, LoadSettings, LogLine, PopoutBounds,
    ReapplyPlan, RecordFormat, RecordStep, Recorder, SampleWriter, Sampler, SavedLimit, Session,
    Settings, Severity, Target, Vendor, WindowBounds, WindowsAdapter, WriteError, XmlExport,
};
use nvml_map::{normalize_pci_bus_id, write_nvidia_watts, AmdSource, NvidiaSource};
use serde::Serialize;
use state::{
    launch_action, mutex_name, run_key_command, run_value_name, show_event_name, LaunchAction,
};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    Emitter, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_Status, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo,
    SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceRegistryPropertyW,
    DIGCF_PRESENT, SPDRP_ADDRESS, SPDRP_BUSNUMBER, SPDRP_DEVICEDESC, SPDRP_DRIVER, SP_DEVINFO_DATA,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, FreeLibrary, GetLastError, ERROR_ALREADY_EXISTS, ERROR_CANCELLED,
    ERROR_FILE_NOT_FOUND, FILETIME, HWND, SYSTEMTIME, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
    INFINITE,
};
use windows_sys::Win32::System::Time::{
    FileTimeToSystemTime, GetTimeZoneInformation, SystemTimeToTzSpecificLocalTime,
    TIME_ZONE_INFORMATION,
};
use windows_sys::Win32::UI::Shell::{
    ShellExecuteExW, ShellExecuteW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, MB_ICONWARNING, MB_OK, SW_HIDE, SW_SHOWNORMAL,
};

const DASHBOARD_LABEL: &str = "dashboard";
const TRAY_ID: &str = "gpupower";
const REAPPLY_TASK: &str = "GpuPower Reapply Limits";
static INSTANCE_MUTEX: OnceLock<usize> = OnceLock::new();
static SHOW_EVENT: OnceLock<usize> = OnceLock::new();

struct FileWriter {
    file: fs::File,
}

impl SampleWriter for FileWriter {
    fn write_chunk(&mut self, text: &str) -> Result<(), WriteError> {
        match self
            .file
            .write_all(text.as_bytes())
            .and_then(|_| self.file.flush())
        {
            Ok(()) => Ok(()),
            Err(err) => Err(io_to_write(err)),
        }
    }
}

fn io_to_write(err: io::Error) -> WriteError {
    const ERROR_DISK_FULL: i32 = 112;
    const ERROR_HANDLE_DISK_FULL: i32 = 39;
    match err.raw_os_error() {
        Some(ERROR_DISK_FULL | ERROR_HANDLE_DISK_FULL) => WriteError::DiskFull,
        _ if err.kind() == io::ErrorKind::StorageFull => WriteError::DiskFull,
        _ => WriteError::Unavailable(err.to_string()),
    }
}

struct ActiveRecording {
    format: RecordFormat,
    path: PathBuf,
    started: Instant,
    recorder: Recorder<FileWriter>,
}

struct WindowsMeta {
    adapter: WindowsAdapter,
    inf: Option<String>,
}

struct AppState {
    session: Mutex<Session>,
    settings: Mutex<Settings>,
    recorder: Mutex<Option<ActiveRecording>>,
    sampler: Mutex<Sampler<NvidiaSource, AmdSource>>,
    windows_meta: Mutex<Vec<WindowsMeta>>,
    last_tick: Mutex<Option<Instant>>,
    last_icon: Mutex<Option<i32>>,
}

impl AppState {
    fn new() -> Self {
        let settings = load_settings_file();
        Self {
            session: Mutex::new(Session { cards: Vec::new() }),
            settings: Mutex::new(settings),
            recorder: Mutex::new(None),
            sampler: Mutex::new(Sampler {
                nvidia: NvidiaSource::default(),
                amd: AmdSource::default(),
                windows: Vec::new(),
            }),
            windows_meta: Mutex::new(Vec::new()),
            last_tick: Mutex::new(None),
            last_icon: Mutex::new(None),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg.starts_with("--")) {
        std::process::exit(run_helper(&args));
    }
    match launch_action(instance_already_running()) {
        LaunchAction::FocusExisting => {
            signal_show_dashboard();
            append_log(Severity::Info, "app", "second launch");
            return;
        }
        LaunchAction::Run => {}
    }

    let state = AppState::new();
    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            if let Err(err) = install_tray(app.handle()) {
                append_log(
                    Severity::Error,
                    "app",
                    &format!("tray could not be created: {err}"),
                );
            }
            let enabled = lock(&app.state::<AppState>().settings).start_with_windows;
            sync_run_key(enabled);
            append_log(Severity::Info, "app", "startup");
            spawn_show_listener(app.handle().clone());
            spawn_sampler(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            plan_apply,
            start_recording,
            stop_recording,
            export_history,
            set_icon_mode,
            open_popout,
            helper_request,
            rescan,
            open_log_folder,
            set_graph_range,
            set_start_with_windows,
            set_detailed_debug,
            settings
        ])
        .build(tauri::generate_context!())
        .expect("error while building GpuPower")
        .run(|_app, event| {
            if let RunEvent::ExitRequested { api, code, .. } = event {
                // Closing the dashboard or the last popout must leave the tray running.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

fn instance_already_running() -> bool {
    unsafe {
        let name = wide(mutex_name());
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            return false;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(handle);
            true
        } else {
            let _ = INSTANCE_MUTEX.set(handle as usize);
            // Hold the show event before Tauri starts so a second launch cannot
            // create, signal, and destroy it before this process is listening.
            let event_name = wide(show_event_name());
            let event = CreateEventW(std::ptr::null(), 0, 0, event_name.as_ptr());
            if !event.is_null() {
                let _ = SHOW_EVENT.set(event as usize);
            }
            false
        }
    }
}

fn signal_show_dashboard() {
    unsafe {
        let name = wide(show_event_name());
        let handle = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
        if !handle.is_null() {
            SetEvent(handle);
            CloseHandle(handle);
        }
    }
}

fn spawn_show_listener(app: tauri::AppHandle) {
    let Some(&handle) = SHOW_EVENT.get() else {
        return;
    };
    std::thread::spawn(move || loop {
        if unsafe { WaitForSingleObject(handle as _, INFINITE) } == WAIT_OBJECT_0 {
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Err(err) = open_dashboard(&app2) {
                    append_log(Severity::Error, "window", &err);
                    say(&err);
                }
            });
        }
    });
}

fn spawn_sampler(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        let app = app.clone();
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_tick(&app))).is_err() {
            append_log(Severity::Error, "sampler", "the sampler tick panicked");
        }
        std::thread::sleep(Duration::from_secs(1));
    });
}

fn run_tick(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let sampled_at = Instant::now();
    let (tick, nvidia_code, amd_notice) = {
        let mut sampler = lock(&state.sampler);
        let meta = enumerate_display_adapters();
        sampler.windows = meta.iter().map(|row| row.adapter.clone()).collect();
        let tick = sampler.tick();
        let nvidia_code = sampler.nvidia.last_code.clone();
        let amd_notice = sampler.amd.take_notice();
        drop(sampler);
        *lock(&state.windows_meta) = meta;
        (tick, nvidia_code, amd_notice)
    };
    let detailed = lock(&state.settings).detailed_debug;
    let iso = local_rfc3339();
    if let Some(message) = &tick.nvidia_error {
        if let Some(line) = sample_tick_line(&iso, detailed, true, nvidia_code.as_deref(), message)
        {
            append_log_line(&line);
        }
    }
    if let Some(message) = amd_notice {
        append_log(Severity::Warn, "adlx", &message);
    }
    let stamp = unix_now().to_string();
    let mut appeared = Vec::new();
    let mut dropped = Vec::new();
    let snapshots = {
        let mut session = lock(&state.session);
        let seen: BTreeSet<_> = tick.cards.iter().map(|card| card.id.clone()).collect();
        let previous: BTreeSet<_> = session
            .cards
            .iter()
            .map(|card| card.snapshot.id.clone())
            .collect();
        for id in seen.difference(&previous) {
            appeared.push(id.clone());
        }
        for card in &session.cards {
            if card.offline_since.is_none() && !seen.contains(&card.snapshot.id) {
                dropped.push(card.snapshot.id.clone());
            }
        }
        let next = reconcile(&session, &tick.cards, &stamp);
        *session = next;
        session
            .cards
            .iter()
            .map(|card| card.snapshot.clone())
            .collect::<Vec<_>>()
    };
    for id in appeared {
        append_log(Severity::Info, "session", &format!("{id} appeared"));
    }
    for id in dropped {
        append_log(Severity::Info, "session", &format!("{id} dropped out"));
    }
    {
        let mut recorder = lock(&state.recorder);
        if let Some(active) = recorder.as_mut() {
            if let RecordStep::Stopped { reason } = active.recorder.push(&iso, &snapshots) {
                append_log(Severity::Error, "record", &reason);
                *recorder = None;
            }
        }
    }
    *lock(&state.last_tick) = Some(sampled_at);
    let payload = snapshot_payload(&state);
    let tooltip = tooltip_from(&payload);
    let number = payload.icon_number;
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        update_tray(&app2, number, &tooltip, false);
        let _ = app2.emit("snapshot", payload);
    });
}

fn install_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let mode = lock(&app.state::<AppState>().settings).icon_mode;
    let menu = tray_menu(app, mode)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_image(None))
        .tooltip("GpuPower")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle_tray_menu(app, &event))
        .build(app)?;
    Ok(())
}

fn tray_menu(app: &tauri::AppHandle, mode: IconMode) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let combined = CheckMenuItem::with_id(
        app,
        "icon-combined",
        "Combined watts",
        true,
        mode == IconMode::Combined,
        None::<&str>,
    )?;
    let busiest = CheckMenuItem::with_id(
        app,
        "icon-busiest",
        "Busiest card",
        true,
        mode == IconMode::Busiest,
        None::<&str>,
    )?;
    let highest = CheckMenuItem::with_id(
        app,
        "icon-highest",
        "Highest percent",
        true,
        mode == IconMode::HighestPercent,
        None::<&str>,
    )?;
    let logs = MenuItem::with_id(app, "logs", "Open log folder", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    Menu::with_items(app, &[&open, &combined, &busiest, &highest, &logs, &quit])
}

fn handle_tray_menu(app: &tauri::AppHandle, event: &MenuEvent) {
    match event.id().0.as_str() {
        "open" => {
            if let Err(err) = open_dashboard(app) {
                append_log(Severity::Error, "window", &err);
                say(&err);
            }
        }
        "icon-combined" => apply_icon_mode(app, IconMode::Combined),
        "icon-busiest" => apply_icon_mode(app, IconMode::Busiest),
        "icon-highest" => apply_icon_mode(app, IconMode::HighestPercent),
        "logs" => {
            if let Err(err) = open_log_folder() {
                append_log(Severity::Error, "app", &err);
            }
        }
        "quit" => quit_app(app),
        _ => {}
    }
}

fn apply_icon_mode(app: &tauri::AppHandle, mode: IconMode) {
    {
        let state = app.state::<AppState>();
        lock(&state.settings).icon_mode = mode;
        let settings = lock(&state.settings).clone();
        save_settings_file(&settings);
    }
    if let Ok(menu) = tray_menu(app, mode) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(menu));
        }
    }
    let payload = snapshot_payload(&app.state::<AppState>());
    let tooltip = tooltip_from(&payload);
    update_tray(app, payload.icon_number, &tooltip, true);
    let _ = app.emit("snapshot", payload);
}

fn quit_app(app: &tauri::AppHandle) {
    {
        let state = app.state::<AppState>();
        let mut recorder = lock(&state.recorder);
        if let Some(active) = recorder.as_mut() {
            if active.format == RecordFormat::Xml && active.recorder.finish_document().is_err() {
                append_log(Severity::Error, "record", "the XML file may be unclosed");
            }
        }
        *recorder = None;
    }
    append_log(Severity::Info, "app", "shutdown");
    app.exit(0);
}

fn open_dashboard(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(DASHBOARD_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let bounds = lock(&app.state::<AppState>().settings).window;
    let window =
        WebviewWindowBuilder::new(app, DASHBOARD_LABEL, WebviewUrl::App("index.html".into()))
            .title("GpuPower")
            .decorations(false)
            .resizable(true)
            .inner_size(bounds.width.max(480) as f64, bounds.height.max(640) as f64)
            .min_inner_size(480.0, 640.0)
            .position(bounds.x as f64, bounds.y as f64)
            .focused(true)
            .initialization_script("window.__GPUPOWER_POPOUT__ = null;")
            .build()
            .map_err(webview_error)?;
    remember_on_close(&window, None);
    Ok(())
}

fn open_popout_window(app: &tauri::AppHandle, kind: &str, id: &str) -> Result<(), String> {
    let label = popout_label(kind, id);
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let (min_w, min_h) = if kind == "graph" {
        (480.0, 320.0)
    } else {
        (480.0, 640.0)
    };
    let saved = lock(&app.state::<AppState>().settings)
        .popouts
        .iter()
        .find(|pop| pop.kind == kind && pop.card_id == id)
        .map(|pop| pop.bounds);
    let bounds = saved.unwrap_or(WindowBounds {
        x: 120,
        y: 120,
        width: min_w as u32,
        height: min_h as u32,
    });
    let script = format!(
        "window.__GPUPOWER_POPOUT__ = {{ kind: {}, id: {} }};",
        json_string(kind),
        json_string(id)
    );
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("GpuPower")
        .decorations(false)
        .resizable(true)
        .inner_size(
            bounds.width.max(min_w as u32) as f64,
            bounds.height.max(min_h as u32) as f64,
        )
        .min_inner_size(min_w, min_h)
        .position(bounds.x as f64, bounds.y as f64)
        .focused(true)
        .initialization_script(&script)
        .build()
        .map_err(webview_error)?;
    remember_on_close(&window, Some((kind.to_string(), id.to_string())));
    Ok(())
}

fn popout_label(kind: &str, id: &str) -> String {
    let raw = format!("popout-{kind}-{id}");
    let mut label: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '/') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    label.truncate(200);
    label
}

fn webview_error(err: tauri::Error) -> String {
    let text = err.to_string();
    if text.to_ascii_lowercase().contains("webview") {
        "The Windows web view is unavailable.".into()
    } else {
        text
    }
}

fn remember_on_close(window: &WebviewWindow, popout: Option<(String, String)>) {
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { .. } = event {
            if let Some(window) = app.get_webview_window(&label) {
                save_bounds(&app, &window, popout.clone());
            }
        }
    });
}

fn save_bounds(app: &tauri::AppHandle, window: &WebviewWindow, popout: Option<(String, String)>) {
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    let bounds = WindowBounds {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };
    let state = app.state::<AppState>();
    let copy = {
        let mut settings = lock(&state.settings);
        match &popout {
            None => settings.window = bounds,
            Some((kind, id)) => {
                if let Some(existing) = settings
                    .popouts
                    .iter_mut()
                    .find(|pop| pop.kind == *kind && pop.card_id == *id)
                {
                    existing.bounds = bounds;
                } else {
                    settings.popouts.push(PopoutBounds {
                        card_id: id.clone(),
                        kind: kind.clone(),
                        bounds,
                    });
                }
            }
        }
        settings.clone()
    };
    save_settings_file(&copy);
}

fn update_tray(app: &tauri::AppHandle, number: Option<i32>, tooltip: &str, force: bool) {
    let state = app.state::<AppState>();
    let previous = *lock(&state.last_icon);
    let changed = force || previous != number;
    if changed {
        *lock(&state.last_icon) = number;
    }
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if changed {
        let _ = tray.set_icon(Some(tray_image(number)));
    }
    if tray.set_tooltip(Some(tooltip)).is_err() {
        let short: String = tooltip.chars().take(120).collect();
        let _ = tray.set_tooltip(Some(&short));
    }
}

fn tray_image(number: Option<i32>) -> tauri::image::Image<'static> {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[10, 10, 10, 255]);
    }
    let text = match number {
        Some(value) => value.to_string(),
        None => "-".into(),
    };
    draw_text(&mut rgba, SIZE, &text);
    tauri::image::Image::new_owned(rgba, SIZE, SIZE)
}

fn draw_text(rgba: &mut [u8], width: u32, text: &str) {
    let glyphs: Vec<[u8; 5]> = text.chars().filter_map(|ch| digit_glyph(ch)).collect();
    if glyphs.is_empty() {
        return;
    }
    let digit_w = 4i32;
    let gap = 1i32;
    let total = glyphs.len() as i32 * digit_w + (glyphs.len() as i32 - 1) * gap;
    let origin_x = ((width as i32) - total) / 2;
    let origin_y = 8i32;
    for (index, glyph) in glyphs.iter().enumerate() {
        let x0 = origin_x + index as i32 * (digit_w + gap);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..3 {
                if bits & (1 << (2 - col)) == 0 {
                    continue;
                }
                fill_pixel(rgba, width, x0 + col, origin_y + row as i32 * 3, 2);
            }
        }
    }
}

fn fill_pixel(rgba: &mut [u8], width: u32, x: i32, y: i32, scale: i32) {
    for dy in 0..scale {
        for dx in 0..scale {
            let px = x + dx;
            let py = y + dy;
            if px < 0 || py < 0 || px >= width as i32 || py >= width as i32 {
                continue;
            }
            let offset = ((py as u32 * width + px as u32) * 4) as usize;
            rgba[offset..offset + 4].copy_from_slice(&[250, 250, 250, 255]);
        }
    }
}

fn digit_glyph(ch: char) -> Option<[u8; 5]> {
    Some(match ch {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        _ => return None,
    })
}

fn tooltip_from(payload: &SnapshotPayload) -> String {
    let mut lines = Vec::new();
    for card in &payload.cards {
        let snap = &card.snapshot;
        if snap.power_draw_instant_w.is_none() {
            lines.push(format!("{}: no power reading", snap.name));
            continue;
        }
        let watts = snap
            .power_draw_instant_w
            .map(|watts| format!("{watts:.1} W"))
            .unwrap_or_else(|| "no power reading".into());
        let limit = match &snap.limit {
            Some(limit) if limit.unit == "watts" => format!("{} W", limit.enforced),
            Some(limit) => format!("{}%", limit.enforced),
            None => "no limit".into(),
        };
        let temp = snap
            .temperature_gpu_c
            .map(|temp| format!("{temp:.0} C"))
            .unwrap_or_else(|| "no temperature".into());
        lines.push(format!("{}: {watts}, limit {limit}, {temp}", snap.name));
    }
    if let Some(recording) = &payload.recording {
        lines.push(format!(
            "Recording {} {}, {} bytes",
            recording.format, recording.elapsed, recording.bytes
        ));
    }
    if !payload.stale_ids.is_empty() {
        lines.push("Readings are updating".into());
    }
    if lines.is_empty() {
        "GpuPower".into()
    } else {
        lines.join("\n")
    }
}

#[derive(Clone, Serialize)]
struct SnapshotPayload {
    cards: Vec<CardDto>,
    combined_power: CombinedDto,
    icon_number: Option<i32>,
    stale_ids: Vec<String>,
    age_ms: u64,
    recording: Option<RecordingDto>,
    icon_mode: String,
    graph_range: String,
    start_with_windows: bool,
    reapply_at_sign_in: bool,
    detailed_debug: bool,
}

#[derive(Clone, Serialize)]
struct CardDto {
    snapshot: SnapshotDto,
    offline_since: Option<String>,
    history: Vec<HistoryPointDto>,
    device_id: Option<String>,
    display_inf: Option<String>,
}

#[derive(Clone, Serialize)]
struct SnapshotDto {
    id: String,
    vendor: Vendor,
    name: String,
    pci_bus_id: String,
    status: String,
    can_set_limit: bool,
    limit: Option<LimitDto>,
    power_draw_instant_w: Option<f64>,
    power_draw_average_w: Option<f64>,
    temperature_gpu_c: Option<f64>,
    temperature_hotspot_c: Option<f64>,
    temperature_memory_c: Option<f64>,
    temperature_target_c: Option<f64>,
    temperature_max_operating_c: Option<f64>,
    temperature_slowdown_c: Option<f64>,
    temperature_shutdown_c: Option<f64>,
    fan_speed_percent: Option<f64>,
    clock_graphics_mhz: Option<f64>,
    clock_sm_mhz: Option<f64>,
    clock_video_mhz: Option<f64>,
    clock_memory_mhz: Option<f64>,
    clock_graphics_max_mhz: Option<f64>,
    clock_memory_max_mhz: Option<f64>,
    util_gpu_percent: Option<f64>,
    util_memory_percent: Option<f64>,
    util_encoder_percent: Option<f64>,
    util_decoder_percent: Option<f64>,
    util_jpeg_percent: Option<f64>,
    util_ofa_percent: Option<f64>,
    vram_used_mib: Option<f64>,
    vram_total_mib: Option<f64>,
    voltage_mv: Option<f64>,
    pstate: Option<String>,
    pcie_gen_current: Option<i32>,
    pcie_gen_max: Option<i32>,
    pcie_width_current: Option<i32>,
    pcie_width_max: Option<i32>,
    encoder_sessions: Option<i32>,
    encoder_fps: Option<f64>,
    throttle_reasons: Vec<String>,
    processes: Vec<ProcessDto>,
    error: Option<String>,
    driver_version: Option<String>,
    watt_ceiling_w: Option<f64>,
}

#[derive(Clone, Serialize)]
struct LimitDto {
    unit: String,
    min: i32,
    max: i32,
    default_value: i32,
    current: i32,
    enforced: i32,
}

#[derive(Clone, Serialize)]
struct ProcessDto {
    pid: u32,
    name: String,
}

#[derive(Clone, Serialize)]
struct HistoryPointDto {
    at: String,
    unix: i64,
    power_draw_instant_w: Option<f64>,
    power_draw_average_w: Option<f64>,
    util_gpu_percent: Option<f64>,
    temperature_gpu_c: Option<f64>,
    fan_speed_percent: Option<f64>,
    clock_graphics_mhz: Option<f64>,
    clock_memory_mhz: Option<f64>,
    throttle_reasons: Vec<String>,
}

#[derive(Clone, Serialize)]
struct CombinedDto {
    watts: f64,
    missing_names: Vec<String>,
}

#[derive(Clone, Serialize)]
struct RecordingDto {
    format: String,
    elapsed: String,
    bytes: u64,
    path: String,
}

#[derive(Clone, Serialize)]
struct PlanView {
    kind: String,
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<i32>,
}

#[derive(Clone, Serialize)]
struct HelperReportDto {
    ok: bool,
    message: String,
    reboot_required: bool,
}

fn snapshot_payload(state: &AppState) -> SnapshotPayload {
    let settings = lock(&state.settings).clone();
    let session = lock(&state.session).clone();
    let meta = lock(&state.windows_meta);
    let age_ms = lock(&state.last_tick)
        .map(|started| started.elapsed().as_millis() as u64)
        .unwrap_or(0);
    let cards_now: Vec<AdapterSnapshot> = session
        .cards
        .iter()
        .map(|card| card.snapshot.clone())
        .collect();
    let combined = combined_power(&cards_now);
    let number = icon_number(settings.icon_mode, &cards_now);
    let stale = is_stale(age_ms);
    let stale_ids = if stale {
        cards_now.iter().map(|card| card.id.clone()).collect()
    } else {
        Vec::new()
    };
    let recording = lock(&state.recorder).as_ref().map(recording_dto);
    let cards = session
        .cards
        .iter()
        .map(|card| {
            let matched = meta.iter().find(|row| {
                row.adapter.pnp_id == card.snapshot.id
                    || row
                        .adapter
                        .pci_bus_id
                        .as_deref()
                        .is_some_and(|pci| !pci.is_empty() && pci == card.snapshot.pci_bus_id)
            });
            CardDto {
                snapshot: snapshot_dto(&card.snapshot),
                offline_since: card.offline_since.clone(),
                history: card
                    .history
                    .as_slice()
                    .iter()
                    .map(|sample| HistoryPointDto {
                        at: sample.at.clone(),
                        unix: sample.unix,
                        power_draw_instant_w: sample.snapshot.power_draw_instant_w,
                        power_draw_average_w: sample.snapshot.power_draw_average_w,
                        util_gpu_percent: sample.snapshot.util_gpu_percent,
                        temperature_gpu_c: sample.snapshot.temperature_gpu_c,
                        fan_speed_percent: sample.snapshot.fan_speed_percent,
                        clock_graphics_mhz: sample.snapshot.clock_graphics_mhz,
                        clock_memory_mhz: sample.snapshot.clock_memory_mhz,
                        throttle_reasons: sample.snapshot.throttle_reasons.clone(),
                    })
                    .collect(),
                device_id: matched.map(|row| row.adapter.pnp_id.clone()),
                display_inf: matched.and_then(|row| row.inf.clone()),
            }
        })
        .collect();
    SnapshotPayload {
        cards,
        combined_power: CombinedDto {
            watts: combined.watts,
            missing_names: combined.missing_names,
        },
        icon_number: number,
        stale_ids,
        age_ms,
        recording,
        icon_mode: icon_mode_name(settings.icon_mode).into(),
        graph_range: graph_range_name(settings.graph_range).into(),
        start_with_windows: settings.start_with_windows,
        reapply_at_sign_in: settings.reapply_at_sign_in,
        detailed_debug: settings.detailed_debug,
    }
}

fn snapshot_dto(card: &AdapterSnapshot) -> SnapshotDto {
    SnapshotDto {
        id: card.id.clone(),
        vendor: card.vendor.clone(),
        name: card.name.clone(),
        pci_bus_id: card.pci_bus_id.clone(),
        status: card.status.clone(),
        can_set_limit: card.can_set_limit,
        limit: card.limit.as_ref().map(limit_dto),
        power_draw_instant_w: card.power_draw_instant_w,
        power_draw_average_w: card.power_draw_average_w,
        temperature_gpu_c: card.temperature_gpu_c,
        temperature_hotspot_c: card.temperature_hotspot_c,
        temperature_memory_c: card.temperature_memory_c,
        temperature_target_c: card.temperature_target_c,
        temperature_max_operating_c: card.temperature_max_operating_c,
        temperature_slowdown_c: card.temperature_slowdown_c,
        temperature_shutdown_c: card.temperature_shutdown_c,
        fan_speed_percent: card.fan_speed_percent,
        clock_graphics_mhz: card.clock_graphics_mhz,
        clock_sm_mhz: card.clock_sm_mhz,
        clock_video_mhz: card.clock_video_mhz,
        clock_memory_mhz: card.clock_memory_mhz,
        clock_graphics_max_mhz: card.clock_graphics_max_mhz,
        clock_memory_max_mhz: card.clock_memory_max_mhz,
        util_gpu_percent: card.util_gpu_percent,
        util_memory_percent: card.util_memory_percent,
        util_encoder_percent: card.util_encoder_percent,
        util_decoder_percent: card.util_decoder_percent,
        util_jpeg_percent: card.util_jpeg_percent,
        util_ofa_percent: card.util_ofa_percent,
        vram_used_mib: card.vram_used_mib,
        vram_total_mib: card.vram_total_mib,
        voltage_mv: card.voltage_mv,
        pstate: card.pstate.clone(),
        pcie_gen_current: card.pcie_gen_current,
        pcie_gen_max: card.pcie_gen_max,
        pcie_width_current: card.pcie_width_current,
        pcie_width_max: card.pcie_width_max,
        encoder_sessions: card.encoder_sessions,
        encoder_fps: card.encoder_fps,
        throttle_reasons: card.throttle_reasons.clone(),
        processes: card
            .processes
            .iter()
            .map(|process| ProcessDto {
                pid: process.pid,
                name: process.name.clone(),
            })
            .collect(),
        error: card.error.clone(),
        driver_version: card.driver_version.clone(),
        watt_ceiling_w: card.watt_ceiling_w,
    }
}

fn limit_dto(limit: &LimitRange) -> LimitDto {
    LimitDto {
        unit: limit_unit_name(limit.unit).into(),
        min: limit.min,
        max: limit.max,
        default_value: limit.default_value,
        current: limit.current,
        enforced: limit.enforced,
    }
}

fn recording_dto(active: &ActiveRecording) -> RecordingDto {
    let bytes = fs::metadata(&active.path)
        .map(|meta| meta.len())
        .unwrap_or(0);
    RecordingDto {
        format: format_name(active.format).into(),
        elapsed: format_elapsed(active.started.elapsed()),
        bytes,
        path: active.path.display().to_string(),
    }
}

fn format_elapsed(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

#[tauri::command]
fn snapshot(app: tauri::AppHandle) -> SnapshotPayload {
    snapshot_payload(&app.state::<AppState>())
}

#[tauri::command]
fn settings(app: tauri::AppHandle) -> Settings {
    lock(&app.state::<AppState>().settings).clone()
}

#[tauri::command]
fn plan_apply(
    app: tauri::AppHandle,
    id: Option<String>,
    preset_percent: Option<u8>,
    value: Option<i32>,
    unit: Option<String>,
) -> Result<Vec<PlanView>, String> {
    let target = if let Some(percent) = preset_percent {
        Target::PresetPercent(percent)
    } else if let (Some(value), Some(unit)) = (value, unit) {
        Target::Custom {
            value,
            unit: parse_limit_unit(&unit)?,
        }
    } else {
        return Err("plan_apply needs a preset percent or a custom value and unit".into());
    };
    let state = app.state::<AppState>();
    let session = lock(&state.session);
    let inputs = session
        .cards
        .iter()
        .filter(|card| match &id {
            Some(only) => only == &card.snapshot.id,
            None => true,
        })
        .filter(|card| card.snapshot.can_set_limit)
        .filter_map(|card| {
            card.snapshot.limit.map(|range| CardLimitInput {
                id: card.snapshot.id.clone(),
                name: card.snapshot.name.clone(),
                range,
            })
        })
        .collect::<Vec<_>>();
    drop(session);
    Ok(plan_all(&inputs, target)
        .into_iter()
        .map(plan_view)
        .collect())
}

fn plan_view(plan: CardPlan) -> PlanView {
    match plan {
        CardPlan::Write {
            id,
            name,
            unit,
            value,
        } => PlanView {
            kind: "write".into(),
            id,
            name,
            unit: Some(limit_unit_name(unit).into()),
            value: Some(value),
            min: None,
            max: None,
        },
        CardPlan::RefusedOutOfRange {
            id,
            name,
            min,
            max,
            unit,
        } => PlanView {
            kind: "refused".into(),
            id,
            name,
            unit: Some(limit_unit_name(unit).into()),
            value: None,
            min: Some(min),
            max: Some(max),
        },
        CardPlan::RefusedInvalidPreset { id, name } => PlanView {
            kind: "refused".into(),
            id,
            name,
            unit: None,
            value: None,
            min: None,
            max: None,
        },
        CardPlan::SkippedDifferentUnit { id, name } => PlanView {
            kind: "skipped".into(),
            id,
            name,
            unit: None,
            value: None,
            min: None,
            max: None,
        },
    }
}

#[tauri::command]
fn start_recording(app: tauri::AppHandle, path: String, format: String) -> Result<(), String> {
    let format = parse_format(&format)?;
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    let state = app.state::<AppState>();
    if lock(&state.recorder).is_some() {
        return Err("a recording is already running".into());
    }
    let file = fs::File::create(&path).map_err(|err| err.to_string())?;
    let recorder = Recorder::start(format, FileWriter { file });
    {
        let mut slot = lock(&state.recorder);
        if slot.is_some() {
            return Err("a recording is already running".into());
        }
        *slot = Some(ActiveRecording {
            format,
            path: path.clone(),
            started: Instant::now(),
            recorder,
        });
    }
    {
        let mut settings = lock(&state.settings);
        if let Some(parent) = path.parent() {
            settings.last_recording_directory = Some(parent.display().to_string());
        }
        let copy = settings.clone();
        drop(settings);
        save_settings_file(&copy);
    }
    append_log(
        Severity::Info,
        "record",
        &format!("recording started {}", path.display()),
    );
    Ok(())
}

#[tauri::command]
fn stop_recording(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut slot = lock(&state.recorder);
    let Some(mut active) = slot.take() else {
        return Err("recording is not running".into());
    };
    let path = active.path.display().to_string();
    if let Err(err) = active.recorder.finish_document() {
        if active.format == RecordFormat::Xml {
            append_log(Severity::Error, "record", "the XML file may be unclosed");
        }
        append_log(Severity::Error, "record", &write_err_string(err));
    }
    drop(slot);
    append_log(
        Severity::Info,
        "record",
        &format!("recording stopped {path}"),
    );
    Ok(())
}

#[tauri::command]
fn export_history(app: tauri::AppHandle, path: String, format: String) -> Result<(), String> {
    let format = parse_format(&format)?;
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    let state = app.state::<AppState>();
    let session = lock(&state.session).clone();
    let mut writer = FileWriter {
        file: fs::File::create(&path).map_err(|err| err.to_string())?,
    };
    match format {
        RecordFormat::Csv => {
            writer
                .write_chunk(&with_newline(csv_header()))
                .map_err(write_err_string)?;
            for card in &session.cards {
                for sample in card.history.as_slice() {
                    let row = csv_row(&display_stamp(&sample.at), &sample.snapshot);
                    writer
                        .write_chunk(&with_newline(row))
                        .map_err(write_err_string)?;
                }
            }
        }
        RecordFormat::Jsonl => {
            for card in &session.cards {
                for sample in card.history.as_slice() {
                    let line = jsonl_line(&display_stamp(&sample.at), &sample.snapshot);
                    writer
                        .write_chunk(&with_newline(line))
                        .map_err(write_err_string)?;
                }
            }
        }
        RecordFormat::Xml => {
            let mut xml = XmlExport::new();
            for card in &session.cards {
                for sample in card.history.as_slice() {
                    xml.push(&display_stamp(&sample.at), &[sample.snapshot.clone()]);
                }
            }
            writer
                .write_chunk(&xml.finish())
                .map_err(write_err_string)?;
        }
    }
    if let Some(parent) = path.parent() {
        let mut settings = lock(&state.settings);
        settings.last_recording_directory = Some(parent.display().to_string());
        let copy = settings.clone();
        drop(settings);
        save_settings_file(&copy);
    }
    append_log(
        Severity::Info,
        "export",
        &format!("exported history to {}", path.display()),
    );
    Ok(())
}

#[tauri::command]
fn set_icon_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    let mode = parse_icon_mode(&mode)?;
    apply_icon_mode(&app, mode);
    Ok(())
}

#[tauri::command]
fn set_graph_range(app: tauri::AppHandle, range: String) -> Result<(), String> {
    let range = parse_graph_range(&range)?;
    let state = app.state::<AppState>();
    lock(&state.settings).graph_range = range;
    let settings = lock(&state.settings).clone();
    save_settings_file(&settings);
    let _ = app.emit("snapshot", snapshot_payload(&state));
    Ok(())
}

#[tauri::command]
fn set_start_with_windows(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    lock(&state.settings).start_with_windows = enabled;
    let settings = lock(&state.settings).clone();
    save_settings_file(&settings);
    sync_run_key(enabled);
    Ok(())
}

#[tauri::command]
fn set_detailed_debug(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    lock(&state.settings).detailed_debug = enabled;
    let settings = lock(&state.settings).clone();
    save_settings_file(&settings);
    Ok(())
}

#[tauri::command]
fn open_popout(app: tauri::AppHandle, kind: String, id: String) -> Result<(), String> {
    open_popout_window(&app, &kind, &id).map_err(|err| {
        append_log(Severity::Error, "window", &err);
        err
    })
}

#[tauri::command]
fn rescan(app: tauri::AppHandle) -> Result<(), String> {
    run_tick(&app);
    Ok(())
}

#[tauri::command]
fn open_log_folder() -> Result<(), String> {
    let dir = logs_dir();
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let operation = wide("open");
    let path = wide(&dir.display().to_string());
    let code = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if (code as isize) <= 32 {
        Err("the log folder could not be opened".into())
    } else {
        Ok(())
    }
}

#[tauri::command]
fn helper_request(app: tauri::AppHandle, args: Vec<String>) -> Result<HelperReportDto, String> {
    parse_helper_args(&args)?;
    for path in helper_result_paths() {
        let _ = fs::remove_file(path);
    }
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    match shell_execute_runas(&exe, &args) {
        Err(LaunchError::Cancelled) => {
            let message = approval_cancelled_message();
            append_log(Severity::Warn, "helper", message);
            Err(message.to_string())
        }
        Err(LaunchError::Failed(err)) => Err(err),
        Ok(()) => {
            let json = read_helper_stdout()?;
            let report = parse_report(&json)?;
            let state = app.state::<AppState>();
            *lock(&state.settings) = load_settings_file();
            run_tick(&app);
            Ok(report)
        }
    }
}

enum LaunchError {
    Cancelled,
    Failed(String),
}

fn shell_execute_runas(exe: &Path, args: &[String]) -> Result<(), LaunchError> {
    let verb = wide("runas");
    let file = wide(&exe.display().to_string());
    let params = wide(
        &args
            .iter()
            .map(|arg| quote_windows(arg))
            .collect::<Vec<_>>()
            .join(" "),
    );
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;
        if ShellExecuteExW(&mut info) == 0 {
            let code = GetLastError();
            if code == ERROR_CANCELLED {
                return Err(LaunchError::Cancelled);
            }
            return Err(LaunchError::Failed(format!(
                "administrator approval failed ({code})"
            )));
        }
        if !info.hProcess.is_null() {
            WaitForSingleObject(info.hProcess, INFINITE);
            CloseHandle(info.hProcess);
        }
    }
    Ok(())
}

fn parse_report(json: &str) -> Result<HelperReportDto, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|err| err.to_string())?;
    Ok(HelperReportDto {
        ok: value
            .get("ok")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        message: value
            .get("message")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
        reboot_required: value
            .get("reboot_required")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
    })
}

fn run_helper(args: &[String]) -> i32 {
    let command = match parse_helper_args(args) {
        Ok(command) => command,
        Err(err) => return finish_helper(false, err, false),
    };
    match command {
        HelperCommand::ApplySaved => apply_saved(),
        HelperCommand::ApplyLimit { id, unit, value } => apply_one(&id, unit, value),
        HelperCommand::Reinstall { inf, device_ids } => reinstall(&inf, &device_ids),
        HelperCommand::Rollback { device_id } => rollback(&device_id),
        HelperCommand::InstallReapplyTask => install_reapply_task(),
        HelperCommand::RemoveReapplyTask => remove_reapply_task(),
    }
}

fn apply_saved() -> i32 {
    let settings = load_settings_file();
    let cards = sample_cards();
    let mut results = Vec::new();
    let mut notes = Vec::new();
    for (id, saved) in &settings.limits {
        let card = cards.iter().find(|card| &card.id == id);
        let live = card.and_then(|card| card.limit.as_ref());
        match plan_reapply(saved, live) {
            ReapplyPlan::SkipAbsent => notes.push(format!("{id} skipped")),
            ReapplyPlan::Write { value, clamped } => {
                let name = card
                    .map(|card| card.name.clone())
                    .unwrap_or_else(|| id.clone());
                if clamped {
                    notes.push(format!("{name} clamped to {value}"));
                }
                let wrote = write_limit(id, saved.unit, value);
                if let Ok(readback) = &wrote {
                    notes.push(format!("{name} read back {readback}"));
                }
                results.push((name, wrote.map(|_| ())));
            }
        }
    }
    finish_helper(
        results.iter().all(|(_, result)| result.is_ok()),
        result_message(&results, &notes),
        false,
    )
}

fn apply_one(id: &str, unit: LimitUnit, value: i32) -> i32 {
    let cards = sample_cards();
    let name = cards
        .iter()
        .find(|card| card.id == id)
        .map(|card| card.name.clone())
        .unwrap_or_else(|| id.to_string());
    match write_limit(id, unit, value) {
        Ok(readback) => {
            remember_limit(id, unit, value);
            finish_helper(true, format!("Applied. {name} read back {readback}"), false)
        }
        Err(err) => finish_helper(false, format!("{name}: {err}"), false),
    }
}

fn write_limit(id: &str, unit: LimitUnit, value: i32) -> Result<String, String> {
    match unit {
        LimitUnit::Watts => {
            let mw = write_nvidia_watts(id, value)?;
            Ok(format!("{mw} mW"))
        }
        LimitUnit::Percent => Err("ADLX power limit writes are not available.".into()),
    }
}

fn reinstall(inf: &str, device_ids: &[String]) -> i32 {
    let before = versions_for(device_ids);
    let mut results = Vec::new();
    let mut reboot = false;
    for id in device_ids {
        match run_pnputil(inf, id) {
            Ok(needs_reboot) => {
                reboot |= needs_reboot;
                results.push((id.clone(), Ok(())));
            }
            Err(err) => results.push((id.clone(), Err(err))),
        }
    }
    let after = versions_for(device_ids);
    let notes = vec![format!("inf {inf} before [{before}] after [{after}]")];
    let ok = results.iter().all(|(_, result)| result.is_ok());
    finish_helper(
        ok,
        with_reboot_note(result_message(&results, &notes), reboot),
        reboot,
    )
}

fn rollback(device_id: &str) -> i32 {
    let before = versions_for(&[device_id.to_string()]);
    match rollback_device(device_id) {
        Ok(reboot) => {
            let after = versions_for(&[device_id.to_string()]);
            let message = with_reboot_note(
                format!("Applied. {device_id} before [{before}] after [{after}]"),
                reboot,
            );
            finish_helper(true, message, reboot)
        }
        Err(err) => finish_helper(false, format!("{device_id}: {err}"), false),
    }
}

fn install_reapply_task() -> i32 {
    let exe = match std::env::current_exe() {
        Ok(path) => path.display().to_string(),
        Err(err) => return finish_helper(false, err.to_string(), false),
    };
    let tr = format!("\"{exe}\" --apply-saved");
    match Command::new("schtasks")
        .args([
            "/Create",
            "/TN",
            REAPPLY_TASK,
            "/SC",
            "ONLOGON",
            "/RL",
            "HIGHEST",
            "/TR",
            &tr,
            "/F",
        ])
        .output()
    {
        Ok(output) if output.status.success() => {
            set_reapply_flag(true);
            finish_helper(
                true,
                "Created the GpuPower Reapply Limits task.".into(),
                false,
            )
        }
        Ok(output) => finish_helper(false, command_failure("schtasks", &output), false),
        Err(err) => finish_helper(false, err.to_string(), false),
    }
}

fn remove_reapply_task() -> i32 {
    match Command::new("schtasks")
        .args(["/Delete", "/TN", REAPPLY_TASK, "/F"])
        .output()
    {
        Ok(output) if output.status.success() => {
            set_reapply_flag(false);
            finish_helper(
                true,
                "Removed the GpuPower Reapply Limits task.".into(),
                false,
            )
        }
        Ok(output) => finish_helper(false, command_failure("schtasks", &output), false),
        Err(err) => finish_helper(false, err.to_string(), false),
    }
}

fn with_reboot_note(message: String, reboot_required: bool) -> String {
    // follow_up has no reboot action. The helper must not reboot Windows.
    let _steps = follow_up(reboot_required);
    if reboot_required {
        format!("{message} A reboot is required. GpuPower will not reboot the PC.")
    } else {
        message
    }
}

fn result_message(results: &[(String, Result<(), String>)], notes: &[String]) -> String {
    let base = if results.is_empty() {
        "Applied.".to_string()
    } else {
        partial_apply_message(results)
    };
    let mut extra = notes.to_vec();
    for (name, result) in results {
        if let Err(err) = result {
            extra.push(format!("{name}: {err}"));
        }
    }
    if extra.is_empty() {
        base
    } else {
        format!("{base} {}", extra.join("; "))
    }
}

fn finish_helper(ok: bool, message: String, reboot_required: bool) -> i32 {
    let report = HelperReport {
        ok,
        message: message.clone(),
        reboot_required,
    };
    let json = format_helper_stdout(&report);
    println!("{json}");
    let _ = io::stdout().flush();
    write_helper_stdout(&json);
    let severity = if ok { Severity::Info } else { Severity::Error };
    append_log(severity, "helper", &message);
    if ok {
        0
    } else {
        1
    }
}

fn sample_cards() -> Vec<AdapterSnapshot> {
    let mut sampler = Sampler {
        nvidia: NvidiaSource::default(),
        amd: AmdSource::default(),
        windows: enumerate_display_adapters()
            .into_iter()
            .map(|row| row.adapter)
            .collect(),
    };
    sampler.tick().cards
}

fn run_pnputil(inf: &str, device_id: &str) -> Result<bool, String> {
    let output = Command::new("pnputil")
        .args(["/update-driver", inf, "/devices", device_id])
        .output()
        .map_err(|err| err.to_string())?;
    let code = output.status.code().unwrap_or(1);
    let text = format!(
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if code == 0 || code == 3010 {
        Ok(code == 3010 || text.to_ascii_lowercase().contains("reboot"))
    } else {
        Err(format!("pnputil exited {code}: {}", text.trim()))
    }
}

fn rollback_device(device_id: &str) -> Result<bool, String> {
    unsafe {
        let library = LoadLibraryW(wide("newdev.dll").as_ptr());
        if library.is_null() {
            return Err("newdev.dll did not load.".into());
        }
        let Some(symbol) = GetProcAddress(library, b"DiRollbackDriver\0".as_ptr()) else {
            FreeLibrary(library);
            return Err("DiRollbackDriver was not found in newdev.dll.".into());
        };
        let rollback: DiRollbackDriverFn = std::mem::transmute(symbol);
        let devs = SetupDiGetClassDevsW(
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | 0x00000004,
        );
        if devs == -1 {
            FreeLibrary(library);
            return Err("the device list could not be opened.".into());
        }
        let mut index = 0u32;
        loop {
            let mut info: SP_DEVINFO_DATA = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
            if SetupDiEnumDeviceInfo(devs, index, &mut info) == 0 {
                break;
            }
            index += 1;
            if !device_instance_id(devs, &info).eq_ignore_ascii_case(device_id) {
                continue;
            }
            let mut need_reboot = 0i32;
            let ok = rollback(
                devs,
                &mut info,
                std::ptr::null_mut(),
                0x0000_0001,
                &mut need_reboot,
            );
            SetupDiDestroyDeviceInfoList(devs);
            FreeLibrary(library);
            if ok == 0 {
                return Err(format!("DiRollbackDriver failed ({})", GetLastError()));
            }
            return Ok(need_reboot != 0);
        }
        SetupDiDestroyDeviceInfoList(devs);
        FreeLibrary(library);
        Err(format!("device {device_id} was not found"))
    }
}

type DiRollbackDriverFn =
    unsafe extern "system" fn(HDEVINFO, *mut SP_DEVINFO_DATA, HWND, u32, *mut i32) -> i32;

fn versions_for(ids: &[String]) -> String {
    let rows = enumerate_display_adapters();
    ids.iter()
        .map(|id| {
            let version = rows
                .iter()
                .find(|row| row.adapter.pnp_id == *id)
                .and_then(|row| row.adapter.driver_version.clone())
                .unwrap_or_else(|| "unknown".into());
            format!("{id} {version}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn command_failure(name: &str, output: &std::process::Output) -> String {
    let code = output.status.code().unwrap_or(1);
    let text = format!(
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!("{name} exited {code}: {}", text.trim())
}

fn remember_limit(id: &str, unit: LimitUnit, value: i32) {
    let mut settings = load_settings_file();
    settings
        .limits
        .insert(id.to_string(), SavedLimit { unit, value });
    save_settings_file(&settings);
}

fn set_reapply_flag(enabled: bool) {
    let mut settings = load_settings_file();
    settings.reapply_at_sign_in = enabled;
    save_settings_file(&settings);
}

fn enumerate_display_adapters() -> Vec<WindowsMeta> {
    enumerate_display_adapters_inner()
}

fn enumerate_display_adapters_inner() -> Vec<WindowsMeta> {
    unsafe {
        let class = guid_devclass_display();
        let devs = SetupDiGetClassDevsW(
            &class,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT,
        );
        if devs == -1 {
            return Vec::new();
        }
        let mut rows = Vec::new();
        let mut index = 0u32;
        loop {
            let mut info: SP_DEVINFO_DATA = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
            if SetupDiEnumDeviceInfo(devs, index, &mut info) == 0 {
                break;
            }
            index += 1;
            let pnp = device_instance_id(devs, &info);
            if pnp.is_empty() {
                continue;
            }
            let name =
                registry_string(devs, &info, SPDRP_DEVICEDESC).unwrap_or_else(|| pnp.clone());
            let driver_key = registry_string(devs, &info, SPDRP_DRIVER);
            let (driver_version, inf) = driver_key
                .as_deref()
                .map(driver_key_details)
                .unwrap_or((None, None));
            let pci = pci_location(
                registry_dword(devs, &info, SPDRP_BUSNUMBER),
                registry_dword(devs, &info, SPDRP_ADDRESS),
            )
            .map(|bus| normalize_pci_bus_id(&bus));
            rows.push(WindowsMeta {
                adapter: WindowsAdapter {
                    name,
                    pnp_id: pnp.clone(),
                    vendor_id: vendor_from_pnp(&pnp),
                    status: device_status(info.DevInst),
                    driver_version,
                    pci_bus_id: pci,
                },
                inf,
            });
        }
        SetupDiDestroyDeviceInfoList(devs);
        rows
    }
}

fn guid_devclass_display() -> windows_sys::core::GUID {
    windows_sys::core::GUID {
        data1: 0x4d36e968,
        data2: 0xe325,
        data3: 0x11ce,
        data4: [0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18],
    }
}

fn device_instance_id(devs: HDEVINFO, info: &SP_DEVINFO_DATA) -> String {
    unsafe {
        let mut buf = [0u16; 512];
        let mut required = 0u32;
        let ok = SetupDiGetDeviceInstanceIdW(
            devs,
            info as *const SP_DEVINFO_DATA as *mut SP_DEVINFO_DATA,
            buf.as_mut_ptr(),
            buf.len() as u32,
            &mut required,
        );
        if ok == 0 {
            return String::new();
        }
        utf16_prefix(&buf)
    }
}

fn registry_string(devs: HDEVINFO, info: &SP_DEVINFO_DATA, property: u32) -> Option<String> {
    unsafe {
        let mut buf = vec![0u16; 512];
        let mut reg_type = 0u32;
        let mut required = 0u32;
        let ok = SetupDiGetDeviceRegistryPropertyW(
            devs,
            info as *const SP_DEVINFO_DATA as *mut SP_DEVINFO_DATA,
            property,
            &mut reg_type,
            buf.as_mut_ptr() as *mut u8,
            (buf.len() * 2) as u32,
            &mut required,
        );
        if ok == 0 {
            return None;
        }
        let text = utf16_prefix(&buf);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

fn registry_dword(devs: HDEVINFO, info: &SP_DEVINFO_DATA, property: u32) -> Option<u32> {
    unsafe {
        let mut value = 0u32;
        let mut reg_type = 0u32;
        let mut required = 0u32;
        let ok = SetupDiGetDeviceRegistryPropertyW(
            devs,
            info as *const SP_DEVINFO_DATA as *mut SP_DEVINFO_DATA,
            property,
            &mut reg_type,
            &mut value as *mut u32 as *mut u8,
            4,
            &mut required,
        );
        if ok == 0 {
            None
        } else {
            Some(value)
        }
    }
}

fn driver_key_details(driver_key: &str) -> (Option<String>, Option<String>) {
    let path = format!("SYSTEM\\CurrentControlSet\\Control\\Class\\{driver_key}");
    let version = reg_sz(HKEY_LOCAL_MACHINE, &path, "DriverVersion");
    let inf = reg_sz(HKEY_LOCAL_MACHINE, &path, "InfPath").and_then(|name| {
        let published = PathBuf::from(r"C:\Windows\INF").join(&name);
        if published.exists() {
            Some(published.display().to_string())
        } else if name.is_empty() {
            None
        } else {
            Some(name)
        }
    });
    (version, inf)
}

fn reg_sz(root: HKEY, subkey: &str, value: &str) -> Option<String> {
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let path = wide(subkey);
        if RegOpenKeyExW(root, path.as_ptr(), 0, KEY_QUERY_VALUE, &mut key) != 0 {
            return None;
        }
        let mut buf = vec![0u16; 512];
        let mut size = (buf.len() * 2) as u32;
        let mut reg_type = 0u32;
        let name = wide(value);
        let status = RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null(),
            &mut reg_type,
            buf.as_mut_ptr() as *mut u8,
            &mut size,
        );
        RegCloseKey(key);
        if status != 0 {
            return None;
        }
        let text = utf16_prefix(&buf);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

fn device_status(devinst: u32) -> String {
    let mut status = 0u32;
    let mut problem = 0u32;
    let cr = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, devinst, 0) };
    if cr == 0 && problem == 0 {
        "OK".into()
    } else {
        "Error".into()
    }
}

fn vendor_from_pnp(pnp: &str) -> Option<u16> {
    let upper = pnp.to_ascii_uppercase();
    let start = upper.find("VEN_")?;
    let hex = upper.get(start + 4..start + 8)?;
    u16::from_str_radix(hex, 16).ok()
}

fn pci_location(bus: Option<u32>, address: Option<u32>) -> Option<String> {
    let bus = bus?;
    let address = address?;
    let device = (address >> 16) & 0xffff;
    let function = address & 0xffff;
    Some(format!("00000000:{bus:02x}:{device:02x}.{function:x}"))
}

fn utf16_prefix(buf: &[u16]) -> String {
    let len = buf.iter().position(|unit| *unit == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn app_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("GpuPower")
}

fn settings_path() -> PathBuf {
    app_dir().join("settings.json")
}

fn logs_dir() -> PathBuf {
    app_dir().join("logs")
}

fn helper_result_paths() -> Vec<PathBuf> {
    let mut paths = vec![app_dir().join("helper-stdout.json")];
    if let Ok(program_data) = std::env::var("PROGRAMDATA") {
        paths.push(
            PathBuf::from(program_data)
                .join("GpuPower")
                .join("helper-stdout.json"),
        );
    }
    paths
}

fn write_helper_stdout(json: &str) {
    for path in helper_result_paths() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, json);
    }
}

fn read_helper_stdout() -> Result<String, String> {
    for path in helper_result_paths() {
        if let Ok(text) = fs::read_to_string(&path) {
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }
    Err("the elevated helper did not return a result".into())
}

fn load_settings_file() -> Settings {
    let path = settings_path();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return default_settings(),
    };
    match load_settings(&text) {
        LoadSettings::Ready(settings) => settings,
        LoadSettings::Corrupt => {
            let bad = PathBuf::from(format!("{}.bad", path.display()));
            let _ = fs::rename(&path, bad);
            append_log(
                Severity::Warn,
                "settings",
                "settings were unreadable and were renamed",
            );
            default_settings()
        }
    }
}

fn save_settings_file(settings: &Settings) {
    let path = settings_path();
    if let Err(err) = (|| -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(settings)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        fs::write(path, text)
    })() {
        append_log(
            Severity::Error,
            "settings",
            &format!("settings could not be saved: {err}"),
        );
    }
}

fn sync_run_key(enabled: bool) {
    if let Err(err) = set_run_key(enabled) {
        append_log(
            Severity::Error,
            "startup",
            &format!("Run key could not be updated: {err}"),
        );
    }
}

fn set_run_key(enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let path = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if status != 0 {
            return Err(format!("Run key could not be opened ({status})"));
        }
        let name = wide(run_value_name());
        let result = if enabled {
            let value = wide(&run_key_command(&exe.display().to_string()));
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr() as *const u8,
                (value.len() * 2) as u32,
            )
        } else {
            let deleted = RegDeleteValueW(key, name.as_ptr());
            if deleted == ERROR_FILE_NOT_FOUND {
                0
            } else {
                deleted
            }
        };
        RegCloseKey(key);
        if result != 0 {
            return Err(format!("Run key could not be updated ({result})"));
        }
    }
    Ok(())
}

fn append_log(severity: Severity, feature: &str, message: &str) {
    append_log_line(&LogLine {
        at: local_rfc3339(),
        severity,
        feature: feature.to_string(),
        message: message.to_string(),
    });
}

fn append_log_line(line: &LogLine) {
    let dir = logs_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("gpu-power.log");
    if let Ok(meta) = fs::metadata(&path) {
        if should_rotate(meta.len()) {
            rotate_logs(&dir);
        }
    }
    let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let _ = writeln!(file, "{}", format_log_line(line));
    let _ = file.flush();
}

fn rotate_logs(dir: &Path) {
    let current = dir.join("gpu-power.log");
    let older = dir.join("gpu-power.log.1");
    let oldest = dir.join("gpu-power.log.2");
    let _ = fs::remove_file(&oldest);
    let _ = fs::rename(&older, &oldest);
    let _ = fs::rename(&current, &older);
}

fn local_rfc3339() -> String {
    unsafe {
        let mut local: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut local);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
            local.wYear,
            local.wMonth,
            local.wDay,
            local.wHour,
            local.wMinute,
            local.wSecond,
            local_offset_suffix()
        )
    }
}

fn unix_to_local_rfc3339(unix: i64) -> String {
    unsafe {
        let ticks = unix
            .saturating_add(11_644_473_600)
            .saturating_mul(10_000_000);
        let file = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut utc: SYSTEMTIME = std::mem::zeroed();
        let mut local: SYSTEMTIME = std::mem::zeroed();
        if FileTimeToSystemTime(&file, &mut utc) == 0
            || SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) == 0
        {
            return unix.to_string();
        }
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
            local.wYear,
            local.wMonth,
            local.wDay,
            local.wHour,
            local.wMinute,
            local.wSecond,
            local_offset_suffix()
        )
    }
}

fn local_offset_suffix() -> String {
    unsafe {
        let mut info: TIME_ZONE_INFORMATION = std::mem::zeroed();
        let kind = GetTimeZoneInformation(&mut info);
        let mut bias = info.Bias;
        if kind == 2 {
            bias += info.DaylightBias;
        } else if kind == 1 {
            bias += info.StandardBias;
        }
        let offset_min = -bias;
        let sign = if offset_min >= 0 { '+' } else { '-' };
        let abs = offset_min.abs();
        format!("{sign}{:02}:{:02}", abs / 60, abs % 60)
    }
}

fn display_stamp(at: &str) -> String {
    match at.parse::<i64>() {
        Ok(unix) => unix_to_local_rfc3339(unix),
        Err(_) => at.to_string(),
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn say(message: &str) {
    let text = wide(message);
    let caption = wide("GpuPower");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONWARNING,
        );
    }
}

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn quote_windows(arg: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0usize;
    for ch in arg.chars() {
        match ch {
            '\\' => slashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(slashes));
                out.push(ch);
                slashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}

fn with_newline(mut text: String) -> String {
    text.push('\n');
    text
}

fn write_err_string(err: WriteError) -> String {
    match err {
        WriteError::DiskFull => {
            "Recording stopped because the disk is full. Monitoring is still running.".into()
        }
        WriteError::Unavailable(text) => text,
    }
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

fn parse_limit_unit(text: &str) -> Result<LimitUnit, String> {
    match text.to_ascii_lowercase().as_str() {
        "watts" => Ok(LimitUnit::Watts),
        "percent" => Ok(LimitUnit::Percent),
        other => Err(format!("unknown unit: {other}")),
    }
}

fn limit_unit_name(unit: LimitUnit) -> &'static str {
    match unit {
        LimitUnit::Watts => "watts",
        LimitUnit::Percent => "percent",
    }
}

fn parse_icon_mode(text: &str) -> Result<IconMode, String> {
    match text.to_ascii_lowercase().as_str() {
        "combined" => Ok(IconMode::Combined),
        "busiest" => Ok(IconMode::Busiest),
        "highestpercent" | "highest_percent" | "highest-percent" => Ok(IconMode::HighestPercent),
        other => Err(format!("unknown icon mode: {other}")),
    }
}

fn icon_mode_name(mode: IconMode) -> &'static str {
    match mode {
        IconMode::Combined => "combined",
        IconMode::Busiest => "busiest",
        IconMode::HighestPercent => "highest_percent",
    }
}

fn parse_graph_range(text: &str) -> Result<GraphRange, String> {
    match text.to_ascii_lowercase().as_str() {
        "1" | "min1" => Ok(GraphRange::Min1),
        "5" | "min5" => Ok(GraphRange::Min5),
        "15" | "min15" => Ok(GraphRange::Min15),
        other => Err(format!("unknown graph range: {other}")),
    }
}

fn graph_range_name(range: GraphRange) -> &'static str {
    match range {
        GraphRange::Min1 => "1",
        GraphRange::Min5 => "5",
        GraphRange::Min15 => "15",
    }
}

fn parse_format(text: &str) -> Result<RecordFormat, String> {
    match text.to_ascii_lowercase().as_str() {
        "csv" => Ok(RecordFormat::Csv),
        "xml" => Ok(RecordFormat::Xml),
        "jsonl" | "json" => Ok(RecordFormat::Jsonl),
        other => Err(format!("unknown recording format: {other}")),
    }
}

fn format_name(format: RecordFormat) -> &'static str {
    match format {
        RecordFormat::Csv => "csv",
        RecordFormat::Xml => "xml",
        RecordFormat::Jsonl => "jsonl",
    }
}
