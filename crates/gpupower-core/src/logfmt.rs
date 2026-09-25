use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

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
        at: utc_now(),
        severity: Severity::Error,
        feature: "sampler".to_owned(),
        message,
    })
}

pub fn should_rotate(byte_len: u64) -> bool {
    byte_len > LOG_ROLL_BYTES
}

// UTC: this crate cannot read the local offset (no unsafe, no timezone crate).
fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format_unix_utc(secs)
}

fn format_unix_utc(secs: u64) -> String {
    let tod = (secs % 86_400) as u32;
    let hour = tod / 3_600;
    let minute = (tod % 3_600) / 60;
    let second = tod % 60;
    let (year, month, day) = civil_from_unix_days(secs / 86_400);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

// Howard Hinnant's civil_from_days. `days` is days since 1970-01-01.
fn civil_from_unix_days(days: u64) -> (i64, u32, u32) {
    let z = days.saturating_add(719_468);
    let era = z / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = u64::from(yoe) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year as i64, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_sample_does_not_log_even_when_detailed() {
        assert!(sample_tick_line(true, false, None, "ok").is_none());
        assert!(sample_tick_line(true, false, Some("NVML_ERROR"), "ok").is_none());
    }

    #[test]
    fn failed_sample_logs_and_detailed_adds_the_code() {
        let plain =
            sample_tick_line(false, true, Some("NVML_ERROR"), "nvidia read failed").unwrap();
        assert_eq!(plain.severity, Severity::Error);
        assert_eq!(plain.feature, "sampler");
        assert_eq!(plain.message, "nvidia read failed");
        assert!(!format_log_line(&plain).contains("NVML_ERROR"));
        let detailed =
            sample_tick_line(true, true, Some("NVML_ERROR"), "nvidia read failed").unwrap();
        assert_eq!(detailed.message, "nvidia read failed (NVML_ERROR)");
        let text = format_log_line(&detailed);
        assert!(text.starts_with(&detailed.at));
        assert!(detailed.at.ends_with('Z') && detailed.at.contains('T'));
        assert!(text.contains("error"));
        assert!(text.contains("sampler"));
        assert!(text.contains("NVML_ERROR"));
        assert!(text.contains(" error sampler "));
        let no_code = sample_tick_line(true, true, None, "nvidia read failed").unwrap();
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
    fn utc_timestamp_matches_known_instants() {
        assert_eq!(format_unix_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_utc(86_400), "1970-01-02T00:00:00Z");
        assert_eq!(format_unix_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_unix_utc(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(format_unix_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(format_unix_utc(1_709_164_799), "2024-02-28T23:59:59Z");
        assert_eq!(format_unix_utc(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(format_unix_utc(1_709_251_200), "2024-03-01T00:00:00Z");
        assert_eq!(format_unix_utc(1_758_840_000), "2025-09-25T22:40:00Z");
        assert_eq!(format_unix_utc(1_790_294_400), "2026-09-25T00:00:00Z");
    }
}
