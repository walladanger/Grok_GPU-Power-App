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
    #[serde(default)]
    pub unit: LimitUnit,
    #[serde(default)]
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
}
