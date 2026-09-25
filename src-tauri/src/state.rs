#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchAction {
    Run,
    FocusExisting,
}

pub fn launch_action(already_running: bool) -> LaunchAction {
    if already_running {
        LaunchAction::FocusExisting
    } else {
        LaunchAction::Run
    }
}

pub fn mutex_name() -> &'static str {
    "Local\\GpuPower"
}

pub fn run_value_name() -> &'static str {
    "GpuPower"
}

pub fn show_event_name() -> &'static str {
    "Local\\GpuPowerShow"
}
