use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warn => "warn",
            Severity::Info => "info",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub at: String,
    pub severity: Severity,
    pub feature: String,
    pub message: String,
}

pub const LOG_ROLL_BYTES: u64 = 2 * 1024 * 1024;
pub const LOG_KEEP_FILES: u8 = 3;

pub fn format_log_line(line: &LogLine) -> String {
    format!(
        "{} {} {} {}",
        line.at, line.severity, line.feature, line.message
    )
}

pub fn sample_tick_line(
    at: &str,
    detailed: bool,
    failed: bool,
    code: Option<&str>,
    message: &str,
) -> Option<LogLine> {
    if !failed {
        return None;
    }
    let message = match code {
        Some(code) if detailed => format!("{message} ({code})"),
        _ => message.to_owned(),
    };
    Some(LogLine {
        at: at.to_owned(),
        severity: Severity::Error,
        feature: "sampler".to_owned(),
        message,
    })
}

pub fn should_rotate(byte_len: u64) -> bool {
    byte_len > LOG_ROLL_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: &str = "2026-09-24T21:29:08-07:00";

    #[test]
    fn ordinary_sample_does_not_log_even_when_detailed() {
        assert!(sample_tick_line(AT, true, false, None, "ok").is_none());
        assert!(sample_tick_line(AT, true, false, Some("NVML_ERROR"), "ok").is_none());
    }

    #[test]
    fn failed_sample_logs_and_detailed_adds_the_code() {
        let plain =
            sample_tick_line(AT, false, true, Some("NVML_ERROR"), "nvidia read failed").unwrap();
        assert_eq!(plain.at, AT);
        assert_eq!(plain.severity, Severity::Error);
        assert_eq!(plain.feature, "sampler");
        assert_eq!(plain.message, "nvidia read failed");
        assert!(!format_log_line(&plain).contains("NVML_ERROR"));
        let detailed =
            sample_tick_line(AT, true, true, Some("NVML_ERROR"), "nvidia read failed").unwrap();
        assert_eq!(detailed.at, AT);
        assert_eq!(detailed.message, "nvidia read failed (NVML_ERROR)");
        let text = format_log_line(&detailed);
        assert!(text.starts_with(AT));
        assert!(text.contains("error"));
        assert!(text.contains("sampler"));
        assert!(text.contains("NVML_ERROR"));
        assert!(text.contains(" error sampler "));
        let no_code = sample_tick_line(AT, true, true, None, "nvidia read failed").unwrap();
        assert_eq!(no_code.message, "nvidia read failed");
    }

    #[test]
    fn rotation_starts_at_two_megabytes() {
        assert_eq!(LOG_ROLL_BYTES, 2 * 1024 * 1024);
        assert!(!should_rotate(2 * 1024 * 1024));
        assert!(should_rotate(2 * 1024 * 1024 + 1));
        assert_eq!(LOG_KEEP_FILES, 3);
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warn.to_string(), "warn");
        assert_eq!(Severity::Info.to_string(), "info");
    }

    #[test]
    fn test_format_log_line() {
        let info_line = LogLine {
            at: AT.to_owned(),
            severity: Severity::Info,
            feature: "sampler".to_owned(),
            message: "ok".to_owned(),
        };
        assert_eq!(
            format_log_line(&info_line),
            "2026-09-24T21:29:08-07:00 info sampler ok"
        );

        let warn_line = LogLine {
            at: AT.to_owned(),
            severity: Severity::Warn,
            feature: "sampler".to_owned(),
            message: "warning".to_owned(),
        };
        assert_eq!(
            format_log_line(&warn_line),
            "2026-09-24T21:29:08-07:00 warn sampler warning"
        );

        let error_line = LogLine {
            at: AT.to_owned(),
            severity: Severity::Error,
            feature: "sampler".to_owned(),
            message: "failed".to_owned(),
        };
        assert_eq!(
            format_log_line(&error_line),
            "2026-09-24T21:29:08-07:00 error sampler failed"
        );
    }

    #[test]
    fn test_sample_tick_line_exact_contents() {
        assert!(sample_tick_line(AT, false, false, None, "ok").is_none());
        assert!(sample_tick_line(AT, true, false, None, "ok").is_none());

        let nvidia_err =
            sample_tick_line(AT, true, true, Some("NVML_ERROR_UNKNOWN"), "nvidia error").unwrap();
        assert_eq!(
            nvidia_err,
            LogLine {
                at: AT.to_owned(),
                severity: Severity::Error,
                feature: "sampler".to_owned(),
                message: "nvidia error (NVML_ERROR_UNKNOWN)".to_owned(),
            }
        );

        let amd_err = sample_tick_line(AT, true, true, Some("ADLX_FAIL"), "amd error").unwrap();
        assert_eq!(
            amd_err,
            LogLine {
                at: AT.to_owned(),
                severity: Severity::Error,
                feature: "sampler".to_owned(),
                message: "amd error (ADLX_FAIL)".to_owned(),
            }
        );

        let both_err = sample_tick_line(
            AT,
            true,
            true,
            Some("NVML_ERROR_UNKNOWN, ADLX_FAIL"),
            "both error",
        )
        .unwrap();
        assert_eq!(
            both_err,
            LogLine {
                at: AT.to_owned(),
                severity: Severity::Error,
                feature: "sampler".to_owned(),
                message: "both error (NVML_ERROR_UNKNOWN, ADLX_FAIL)".to_owned(),
            }
        );
    }

    #[test]
    fn test_should_rotate_boundaries() {
        assert!(!should_rotate(0));
        assert!(!should_rotate(LOG_ROLL_BYTES - 1));
        assert!(!should_rotate(LOG_ROLL_BYTES));
        assert!(should_rotate(LOG_ROLL_BYTES + 1));
    }
}
