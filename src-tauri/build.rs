fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "snapshot",
            "plan_apply",
            "start_recording",
            "stop_recording",
            "export_history",
            "set_icon_mode",
            "open_popout",
            "helper_request",
            "rescan",
            "open_log_folder",
            "set_graph_range",
            "set_start_with_windows",
            "set_detailed_debug",
            "settings",
        ]),
    ))
    .expect("failed to run tauri-build");
}
