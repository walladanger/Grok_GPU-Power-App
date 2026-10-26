use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{GraphRange, LimitUnit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum IconMode {
    #[default]
    Combined,
    Busiest,
    HighestPercent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLimit {
    pub unit: LimitUnit,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowBounds {
    #[serde(default = "default_window_x")]
    pub x: i32,
    #[serde(default = "default_window_y")]
    pub y: i32,
    #[serde(default = "default_window_width")]
    pub width: u32,
    #[serde(default = "default_window_height")]
    pub height: u32,
}

fn default_window_x() -> i32 {
    80
}

fn default_window_y() -> i32 {
    80
}

fn default_window_width() -> u32 {
    960
}

fn default_window_height() -> u32 {
    800
}

impl Default for WindowBounds {
    fn default() -> Self {
        Self {
            x: default_window_x(),
            y: default_window_y(),
            width: default_window_width(),
            height: default_window_height(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PopoutBounds {
    #[serde(default)]
    pub card_id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub bounds: WindowBounds,
}

// `bool`'s Default is false. Start-with-Windows is on unless the file turns it off.
fn default_start_with_windows() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub icon_mode: IconMode,
    #[serde(default)]
    pub graph_range: GraphRange,
    #[serde(default = "default_start_with_windows")]
    pub start_with_windows: bool,
    #[serde(default)]
    pub reapply_at_sign_in: bool,
    #[serde(default)]
    pub detailed_debug: bool,
    #[serde(default)]
    pub last_recording_directory: Option<String>,
    #[serde(default)]
    pub limits: BTreeMap<String, SavedLimit>,
    #[serde(default)]
    pub window: WindowBounds,
    #[serde(default)]
    pub popouts: Vec<PopoutBounds>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadSettings {
    Ready(Settings),
    Corrupt,
}

pub fn default_settings() -> Settings {
    Settings {
        icon_mode: IconMode::Combined,
        graph_range: GraphRange::Min5,
        start_with_windows: default_start_with_windows(),
        reapply_at_sign_in: false,
        detailed_debug: false,
        last_recording_directory: None,
        limits: BTreeMap::new(),
        window: WindowBounds::default(),
        popouts: Vec::new(),
    }
}

pub fn load_settings(text: &str) -> LoadSettings {
    match serde_json::from_str(text) {
        Ok(settings) => LoadSettings::Ready(settings),
        Err(_) => LoadSettings::Corrupt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphRange, LimitUnit};

    #[test]
    fn broken_json_is_corrupt_and_missing_keys_use_defaults() {
        assert!(matches!(load_settings("{"), LoadSettings::Corrupt));
        match load_settings("{}") {
            LoadSettings::Ready(s) => {
                assert!(s.start_with_windows);
                assert!(!s.reapply_at_sign_in);
                assert!(!s.detailed_debug);
                assert!(matches!(s.icon_mode, IconMode::Combined));
                assert!(matches!(s.graph_range, GraphRange::Min5));
            }
            LoadSettings::Corrupt => panic!("empty object is valid"),
        }
    }

    #[test]
    fn saved_limit_missing_unit_or_value_is_corrupt() {
        assert!(matches!(
            load_settings("{\"limits\":{\"GPU-a\":{}}}"),
            LoadSettings::Corrupt
        ));
    }

    #[test]
    fn saved_limit_round_trips_by_uuid() {
        let mut s = default_settings();
        s.limits.insert(
            "GPU-a".into(),
            SavedLimit {
                unit: LimitUnit::Watts,
                value: 280,
            },
        );
        let text = serde_json::to_string(&s).unwrap();
        match load_settings(&text) {
            LoadSettings::Ready(loaded) => assert_eq!(loaded.limits["GPU-a"].value, 280),
            LoadSettings::Corrupt => panic!("round trip"),
        }
    }

    #[test]
    fn test_default_settings_exact_field_values() {
        let s = default_settings();
        assert_eq!(s.icon_mode, IconMode::Combined);
        assert_eq!(s.graph_range, GraphRange::Min5);
        assert!(s.start_with_windows);
        assert!(!s.reapply_at_sign_in);
        assert!(!s.detailed_debug);
        assert_eq!(s.last_recording_directory, None);
        assert!(s.limits.is_empty());

        assert_eq!(s.window.x, 80);
        assert_eq!(s.window.y, 80);
        assert_eq!(s.window.width, 960);
        assert_eq!(s.window.height, 800);

        assert!(s.popouts.is_empty());
    }

    #[test]
    fn test_window_bounds_default_and_missing_keys() {
        let default_bounds = WindowBounds::default();
        assert_eq!(default_bounds.x, 80);
        assert_eq!(default_bounds.y, 80);
        assert_eq!(default_bounds.width, 960);
        assert_eq!(default_bounds.height, 800);

        match load_settings("{\"window\": {}}") {
            LoadSettings::Ready(s) => {
                assert_eq!(s.window.x, 80);
                assert_eq!(s.window.y, 80);
                assert_eq!(s.window.width, 960);
                assert_eq!(s.window.height, 800);
            }
            LoadSettings::Corrupt => panic!("should parse missing window keys"),
        }
    }

    #[test]
    fn test_default_start_with_windows_applied() {
        match load_settings("{}") {
            LoadSettings::Ready(s) => {
                assert!(s.start_with_windows);
            }
            LoadSettings::Corrupt => panic!("should parse empty obj"),
        }
    }

    #[test]
    fn test_icon_mode_serde_round_trip() {
        // Combined
        match load_settings("{\"icon_mode\": \"Combined\"}") {
            LoadSettings::Ready(s) => assert_eq!(s.icon_mode, IconMode::Combined),
            LoadSettings::Corrupt => panic!("Failed to load Combined"),
        }
        assert_eq!(serde_json::to_string(&IconMode::Combined).unwrap(), "\"Combined\"");

        // Busiest
        match load_settings("{\"icon_mode\": \"Busiest\"}") {
            LoadSettings::Ready(s) => assert_eq!(s.icon_mode, IconMode::Busiest),
            LoadSettings::Corrupt => panic!("Failed to load Busiest"),
        }
        assert_eq!(serde_json::to_string(&IconMode::Busiest).unwrap(), "\"Busiest\"");

        // HighestPercent
        match load_settings("{\"icon_mode\": \"HighestPercent\"}") {
            LoadSettings::Ready(s) => assert_eq!(s.icon_mode, IconMode::HighestPercent),
            LoadSettings::Corrupt => panic!("Failed to load HighestPercent"),
        }
        assert_eq!(serde_json::to_string(&IconMode::HighestPercent).unwrap(), "\"HighestPercent\"");
    }

    #[test]
    fn test_popout_bounds_optional_missing_handling() {
        match load_settings("{\"popouts\": [{}]}") {
            LoadSettings::Ready(s) => {
                assert_eq!(s.popouts.len(), 1);
                let popout = &s.popouts[0];
                assert_eq!(popout.card_id, "");
                assert_eq!(popout.kind, "");
                assert_eq!(popout.bounds.x, 80);
                assert_eq!(popout.bounds.y, 80);
                assert_eq!(popout.bounds.width, 960);
                assert_eq!(popout.bounds.height, 800);
            }
            LoadSettings::Corrupt => panic!("should handle missing popout keys"),
        }
    }

    #[test]
    fn test_load_settings_edge_cases() {
        assert!(matches!(load_settings(""), LoadSettings::Corrupt));

        assert!(matches!(load_settings("{}"), LoadSettings::Ready(_)));

        let default_s = default_settings();
        let serialized = serde_json::to_string(&default_s).unwrap();
        match load_settings(&serialized) {
            LoadSettings::Ready(s) => assert_eq!(s, default_s),
            LoadSettings::Corrupt => panic!("Failed round trip of default settings"),
        }

        match load_settings("{\"unknown_extra_key\": 123}") {
            LoadSettings::Ready(s) => {
                // Should ignore extra key and load normally
                assert!(s.start_with_windows);
            }
            LoadSettings::Corrupt => panic!("Should ignore unknown extra keys"),
        }
    }
}
