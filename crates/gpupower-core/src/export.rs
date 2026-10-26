use crate::{AdapterSnapshot, LimitUnit, ProcessRow, Vendor};

pub const EXPORT_COLUMNS: [&str; 47] = [
    "timestamp_iso",
    "vendor",
    "name",
    "uuid",
    "pci_bus_id",
    "status",
    "power_draw_instant_w",
    "power_draw_average_w",
    "power_limit_w",
    "power_default_w",
    "power_min_w",
    "power_max_w",
    "power_enforced_w",
    "power_limit_percent",
    "temperature_gpu_c",
    "temperature_hotspot_c",
    "temperature_memory_c",
    "temperature_target_c",
    "temperature_max_operating_c",
    "temperature_slowdown_c",
    "temperature_shutdown_c",
    "fan_speed_percent",
    "clock_graphics_mhz",
    "clock_sm_mhz",
    "clock_video_mhz",
    "clock_memory_mhz",
    "clock_graphics_max_mhz",
    "clock_memory_max_mhz",
    "util_gpu_percent",
    "util_memory_percent",
    "util_encoder_percent",
    "util_decoder_percent",
    "util_jpeg_percent",
    "util_ofa_percent",
    "vram_used_mib",
    "vram_total_mib",
    "voltage_mv",
    "pstate",
    "pcie_gen_current",
    "pcie_gen_max",
    "pcie_width_current",
    "pcie_width_max",
    "encoder_sessions",
    "encoder_fps",
    "throttle_reasons",
    "processes",
    "error",
];

pub fn csv_header() -> String {
    EXPORT_COLUMNS.join(",")
}

pub fn csv_row(timestamp_iso: &str, card: &AdapterSnapshot) -> String {
    export_fields(timestamp_iso, card)
        .iter()
        .map(|field| csv_escape(field))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn jsonl_line(timestamp_iso: &str, card: &AdapterSnapshot) -> String {
    let fields = export_fields(timestamp_iso, card);
    let mut line = String::from('{');
    // serde_json::Map sorts keys unless preserve_order is on. Insert in column order.
    for (index, (column, value)) in EXPORT_COLUMNS.iter().zip(fields.iter()).enumerate() {
        if index > 0 {
            line.push(',');
        }
        push_json_string(&mut line, column);
        line.push(':');
        push_json_string(&mut line, value);
    }
    line.push('}');
    line
}

pub struct XmlExport {
    buffer: String,
}

impl XmlExport {
    pub fn new() -> Self {
        Self {
            buffer: String::from("<samples>"),
        }
    }

    pub fn push(&mut self, timestamp_iso: &str, cards: &[AdapterSnapshot]) {
        self.buffer.push_str("<sample time=\"");
        push_xml_escaped(&mut self.buffer, timestamp_iso);
        self.buffer.push_str("\">");
        for card in cards {
            self.buffer.push_str("<gpu");
            let fields = export_fields(timestamp_iso, card);
            for (column, value) in EXPORT_COLUMNS.iter().zip(fields.iter()) {
                self.buffer.push(' ');
                self.buffer.push_str(column);
                self.buffer.push_str("=\"");
                push_xml_escaped(&mut self.buffer, value);
                self.buffer.push('"');
            }
            self.buffer.push_str("/>");
        }
        self.buffer.push_str("</sample>");
    }

    pub fn finish(mut self) -> String {
        self.buffer.push_str("</samples>");
        self.buffer
    }

    // Test-only: the open document has no closing root until `finish`.
    #[cfg(test)]
    fn clone_body(&self) -> String {
        self.buffer.clone()
    }
}

impl Default for XmlExport {
    fn default() -> Self {
        Self::new()
    }
}

fn export_fields(timestamp_iso: &str, card: &AdapterSnapshot) -> [String; EXPORT_COLUMNS.len()] {
    let power = power_columns(card);
    [
        timestamp_iso.to_string(),
        vendor_name(&card.vendor),
        card.name.clone(),
        card.id.clone(),
        card.pci_bus_id.clone(),
        card.status.clone(),
        fmt_opt_f64(card.power_draw_instant_w),
        fmt_opt_f64(card.power_draw_average_w),
        power.limit_w,
        power.default_w,
        power.min_w,
        power.max_w,
        power.enforced_w,
        power.limit_percent,
        fmt_opt_f64(card.temperature_gpu_c),
        fmt_opt_f64(card.temperature_hotspot_c),
        fmt_opt_f64(card.temperature_memory_c),
        fmt_opt_f64(card.temperature_target_c),
        fmt_opt_f64(card.temperature_max_operating_c),
        fmt_opt_f64(card.temperature_slowdown_c),
        fmt_opt_f64(card.temperature_shutdown_c),
        fmt_opt_f64(card.fan_speed_percent),
        fmt_opt_f64(card.clock_graphics_mhz),
        fmt_opt_f64(card.clock_sm_mhz),
        fmt_opt_f64(card.clock_video_mhz),
        fmt_opt_f64(card.clock_memory_mhz),
        fmt_opt_f64(card.clock_graphics_max_mhz),
        fmt_opt_f64(card.clock_memory_max_mhz),
        fmt_opt_f64(card.util_gpu_percent),
        fmt_opt_f64(card.util_memory_percent),
        fmt_opt_f64(card.util_encoder_percent),
        fmt_opt_f64(card.util_decoder_percent),
        fmt_opt_f64(card.util_jpeg_percent),
        fmt_opt_f64(card.util_ofa_percent),
        fmt_opt_f64(card.vram_used_mib),
        fmt_opt_f64(card.vram_total_mib),
        fmt_opt_f64(card.voltage_mv),
        fmt_opt_string(&card.pstate),
        fmt_opt_i32(card.pcie_gen_current),
        fmt_opt_i32(card.pcie_gen_max),
        fmt_opt_i32(card.pcie_width_current),
        fmt_opt_i32(card.pcie_width_max),
        fmt_opt_i32(card.encoder_sessions),
        fmt_opt_f64(card.encoder_fps),
        card.throttle_reasons.join(";"),
        processes_field(&card.processes),
        fmt_opt_string(&card.error),
    ]
}

struct PowerColumns {
    limit_w: String,
    default_w: String,
    min_w: String,
    max_w: String,
    enforced_w: String,
    limit_percent: String,
}

// Watts cards copy LimitRange into the watt columns and leave percent blank.
// Percent cards write current into power_limit_percent, and watt_ceiling_w into
// power_limit_w only when that ceiling is present.
fn power_columns(card: &AdapterSnapshot) -> PowerColumns {
    let mut columns = PowerColumns {
        limit_w: String::new(),
        default_w: String::new(),
        min_w: String::new(),
        max_w: String::new(),
        enforced_w: String::new(),
        limit_percent: String::new(),
    };
    let Some(limit) = card.limit else {
        return columns;
    };
    match limit.unit {
        LimitUnit::Watts => {
            columns.limit_w = limit.current.to_string();
            columns.default_w = limit.default_value.to_string();
            columns.min_w = limit.min.to_string();
            columns.max_w = limit.max.to_string();
            columns.enforced_w = limit.enforced.to_string();
        }
        LimitUnit::Percent => {
            if let Some(watts) = card.watt_ceiling_w {
                columns.limit_w = fmt_f64(watts);
            }
            columns.limit_percent = limit.current.to_string();
        }
    }
    columns
}

fn processes_field(processes: &[ProcessRow]) -> String {
    processes
        .iter()
        .map(|process| format!("{}:{}", process.pid, process.name))
        .collect::<Vec<_>>()
        .join(";")
}

// Two fractional digits, then drop trailing zeros. A rounded negative zero prints as 0.
fn fmt_f64(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let trimmed = format!("{value:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    if trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed
    }
}

fn fmt_opt_f64(value: Option<f64>) -> String {
    value.map(fmt_f64).unwrap_or_default()
}

fn fmt_opt_i32(value: Option<i32>) -> String {
    value.map(|number| number.to_string()).unwrap_or_default()
}

fn fmt_opt_string(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}

fn vendor_name(vendor: &Vendor) -> String {
    serde_json::to_value(vendor)
        .expect("Vendor serializes")
        .as_str()
        .expect("Vendor serializes as a lowercase string")
        .to_string()
}

fn csv_escape(field: &str) -> String {
    if !field
        .chars()
        .any(|ch| matches!(ch, '"' | ',' | '\n' | '\r'))
    {
        return field.to_string();
    }
    let mut escaped = String::with_capacity(field.len() + 2);
    escaped.push('"');
    for ch in field.chars() {
        if ch == '"' {
            escaped.push('"');
        }
        escaped.push(ch);
    }
    escaped.push('"');
    escaped
}

fn push_xml_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

fn push_json_string(out: &mut String, value: &str) {
    out.push_str(&serde_json::to_string(value).expect("string serializes"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{amd_raphael, gtx1050_error, rtx3090_primary, rtx3090_secondary, ProcessRow};

    #[test]
    fn csv_header_matches_spec_and_blank_sensor_stays_empty() {
        let header = csv_header();
        assert!(header.starts_with("timestamp_iso,vendor,name,uuid,"));
        assert!(header.ends_with("throttle_reasons,processes,error"));
        let mut card = gtx1050_error();
        card.error = Some("device error".into());
        let row = csv_row("2026-09-24T21:29:08-07:00", &card);
        let cols: Vec<_> = row.split(',').collect();
        assert_eq!(cols.len(), EXPORT_COLUMNS.len());
        assert_eq!(cols[0], "2026-09-24T21:29:08-07:00");
        assert_eq!(cols[1], "nvidia");
        assert!(row.contains("device error"));
        let instant = EXPORT_COLUMNS
            .iter()
            .position(|column| *column == "power_draw_instant_w")
            .expect("power_draw_instant_w column");
        assert_eq!(cols[instant], "");
    }

    #[test]
    fn jsonl_escapes_and_xml_is_closed_only_on_finish() {
        let mut card = rtx3090_primary();
        card.processes = vec![ProcessRow {
            pid: 12,
            name: "a&b".into(),
        }];
        card.throttle_reasons = vec!["gpu_idle".into(), "sw_power_cap".into()];
        let line = jsonl_line("2026-09-24T21:29:08-07:00", &card);
        assert!(line.contains("\"throttle_reasons\":\"gpu_idle;sw_power_cap\""));
        assert!(
            line.contains("12:a&b") || line.contains("12:a\\u0026b") || line.contains("12:a&b")
        );
        let mut xml = XmlExport::new();
        xml.push("2026-09-24T21:29:08-07:00", &[card]);
        let open = xml.clone_body();
        assert!(!open.contains("</samples>"));
        assert!(xml.finish().contains("</samples>"));
    }

    #[test]
    fn csv_row_quotes_commas() {
        let mut card = rtx3090_primary();
        card.name = "RTX, 3090".into();
        let row = csv_row("t", &card);
        assert!(row.contains("\"RTX, 3090\""));
    }

    #[test]
    fn test_csv_exact_columns_and_counts() {
        let header = csv_header();
        let header_cols: Vec<_> = header.split(',').collect();
        assert_eq!(header_cols, EXPORT_COLUMNS);

        let fixtures = [
            amd_raphael(),
            gtx1050_error(),
            rtx3090_primary(),
            rtx3090_secondary(),
        ];

        for card in &fixtures {
            let row = csv_row("2026-09-24T21:29:08-07:00", card);
            let mut cols = 1;
            let mut in_quotes = false;
            for ch in row.chars() {
                if ch == '"' {
                    in_quotes = !in_quotes;
                } else if ch == ',' && !in_quotes {
                    cols += 1;
                }
            }
            assert_eq!(cols, EXPORT_COLUMNS.len());
        }
    }

    #[test]
    fn test_csv_row_escaping() {
        let mut card = rtx3090_primary();
        card.name = "RTX, 3090\n\"Edition\"".into();
        let row = csv_row("2026-09-24T21:29:08-07:00", &card);
        let expected = "\"RTX, 3090\n\"\"Edition\"\"\"";
        assert!(row.contains(expected));
    }

    #[test]
    fn test_none_null_fields_rendered_exactly() {
        let mut card = gtx1050_error();
        card.error = Some("device error".into());
        let fields = export_fields("2026-09-24T21:29:08-07:00", &card);

        let power_instant_idx = EXPORT_COLUMNS
            .iter()
            .position(|&c| c == "power_draw_instant_w")
            .unwrap();
        assert_eq!(fields[power_instant_idx], "");

        let error_idx = EXPORT_COLUMNS.iter().position(|&c| c == "error").unwrap();
        assert_eq!(fields[error_idx], "device error");
    }

    #[test]
    fn test_jsonl_line_parses_valid_json() {
        let card = rtx3090_primary();
        let line = jsonl_line("2026-09-24T21:29:08-07:00", &card);

        assert!(!line.ends_with('\n'));

        let parsed: serde_json::Value = serde_json::from_str(&line).expect("parses as valid JSON");

        assert_eq!(parsed["timestamp_iso"], "2026-09-24T21:29:08-07:00");
        assert_eq!(parsed["vendor"], "nvidia");
        assert_eq!(parsed["name"], "NVIDIA GeForce RTX 3090");
        assert_eq!(parsed["power_draw_instant_w"], "8.24");
        assert_eq!(parsed["temperature_gpu_c"], "37");
        assert_eq!(parsed["temperature_memory_c"], "");
    }

    #[test]
    fn test_xml_export_zero_pushes() {
        let xml = XmlExport::new();
        let result = xml.finish();
        assert_eq!(result, "<samples></samples>");
    }

    #[test]
    fn test_xml_export_multiple_cards() {
        let mut xml = XmlExport::new();

        let mut card1 = rtx3090_primary();
        card1.name = "RTX < 3090 & \"Friends\" >".into();

        let card2 = amd_raphael();

        xml.push("2026-09-24T21:29:08-07:00", &[card1, card2]);
        xml.push("2026-09-24T21:29:09-07:00", &[rtx3090_secondary()]);

        let result = xml.finish();

        assert!(result.starts_with("<samples>"));
        assert!(result.ends_with("</samples>"));

        assert!(result.contains("<sample time=\"2026-09-24T21:29:08-07:00\">"));
        assert!(result.contains("<sample time=\"2026-09-24T21:29:09-07:00\">"));
        assert!(result.contains("</sample>"));
        assert!(result.contains("<gpu "));
        assert!(result.contains("/>"));

        assert!(result.contains("name=\"RTX &lt; 3090 &amp; &quot;Friends&quot; >\""));
    }
}
