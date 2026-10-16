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

/// Quote the exe the same way the sign-in task quotes it, so a profile path
/// that contains spaces still starts.
pub fn run_key_command(exe: &str) -> String {
    format!("\"{exe}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_key_quotes_a_path_with_spaces() {
        assert_eq!(
            run_key_command(r"C:\Users\Ada Lovelace\GpuPower.exe"),
            "\"C:\\Users\\Ada Lovelace\\GpuPower.exe\""
        );
    }
}
