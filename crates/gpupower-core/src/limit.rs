use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LimitUnit {
    #[default]
    Watts,
    Percent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitRange {
    pub unit: LimitUnit,
    pub min: i32,
    pub max: i32,
    pub default_value: i32,
    pub current: i32,
    pub enforced: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    PresetPercent(u8),
    Custom { value: i32, unit: LimitUnit },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardLimitInput {
    pub id: String,
    pub name: String,
    pub range: LimitRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardPlan {
    Write {
        id: String,
        name: String,
        unit: LimitUnit,
        value: i32,
    },
    SkippedDifferentUnit {
        id: String,
        name: String,
    },
    RefusedOutOfRange {
        id: String,
        name: String,
        min: i32,
        max: i32,
        unit: LimitUnit,
    },
    RefusedInvalidPreset {
        id: String,
        name: String,
    },
}

pub fn preset_value(range: &LimitRange, percent: u8) -> i32 {
    let raw = match range.unit {
        // +50 before /100 rounds a non-negative result to the nearest integer.
        LimitUnit::Watts => ((range.default_value as i64) * (percent as i64) + 50) / 100,
        LimitUnit::Percent => {
            let span = range.max - range.min;
            i64::from(range.min + (span * percent as i32 + 50) / 100)
        }
    };
    raw.clamp(i64::from(range.min), i64::from(range.max)) as i32
}

pub fn plan_limit(card: &CardLimitInput, target: Target) -> CardPlan {
    match target {
        Target::PresetPercent(percent) => {
            if !matches!(percent, 60 | 70 | 80 | 90 | 100) {
                return CardPlan::RefusedInvalidPreset {
                    id: card.id.clone(),
                    name: card.name.clone(),
                };
            }
            CardPlan::Write {
                id: card.id.clone(),
                name: card.name.clone(),
                unit: card.range.unit,
                value: preset_value(&card.range, percent),
            }
        }
        Target::Custom { value, unit } => {
            if unit != card.range.unit {
                return CardPlan::SkippedDifferentUnit {
                    id: card.id.clone(),
                    name: card.name.clone(),
                };
            }
            if !(card.range.min..=card.range.max).contains(&value) {
                return CardPlan::RefusedOutOfRange {
                    id: card.id.clone(),
                    name: card.name.clone(),
                    min: card.range.min,
                    max: card.range.max,
                    unit: card.range.unit,
                };
            }
            CardPlan::Write {
                id: card.id.clone(),
                name: card.name.clone(),
                unit,
                value,
            }
        }
    }
}

pub fn plan_all(cards: &[CardLimitInput], target: Target) -> Vec<CardPlan> {
    cards.iter().map(|card| plan_limit(card, target)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rtx3090(id: &str) -> CardLimitInput {
        CardLimitInput {
            id: id.into(),
            name: "NVIDIA GeForce RTX 3090".into(),
            range: LimitRange {
                unit: LimitUnit::Watts,
                min: 100,
                max: 350,
                default_value: 350,
                current: 350,
                enforced: 350,
            },
        }
    }

    fn amd_percent() -> CardLimitInput {
        CardLimitInput {
            id: "amd-igpu".into(),
            name: "AMD Radeon Graphics".into(),
            range: LimitRange {
                unit: LimitUnit::Percent,
                min: -30,
                max: 15,
                default_value: 0,
                current: 0,
                enforced: 0,
            },
        }
    }

    #[test]
    fn preset_80_on_3090_is_280_watts() {
        let card = rtx3090("a");
        assert_eq!(preset_value(&card.range, 80), 280);
        assert!(matches!(
            plan_limit(&card, Target::PresetPercent(80)),
            CardPlan::Write {
                value: 280,
                unit: LimitUnit::Watts,
                ..
            }
        ));
    }

    #[test]
    fn preset_below_minimum_clamps_up() {
        let mut card = rtx3090("a");
        card.range.min = 300;
        assert_eq!(preset_value(&card.range, 60), 300);
    }

    #[test]
    fn custom_below_minimum_is_refused() {
        let card = rtx3090("a");
        assert!(matches!(
            plan_limit(
                &card,
                Target::Custom {
                    value: 50,
                    unit: LimitUnit::Watts
                }
            ),
            CardPlan::RefusedOutOfRange {
                min: 100,
                max: 350,
                ..
            }
        ));
    }

    #[test]
    fn invalid_preset_is_refused() {
        let card = rtx3090("a");
        assert!(matches!(
            plan_limit(&card, Target::PresetPercent(50)),
            CardPlan::RefusedInvalidPreset { .. }
        ));
    }

    #[test]
    fn plan_all_custom_watts_skips_percent_card() {
        let plans = plan_all(
            &[rtx3090("a"), rtx3090("b"), amd_percent()],
            Target::Custom {
                value: 250,
                unit: LimitUnit::Watts,
            },
        );
        assert!(matches!(plans[0], CardPlan::Write { value: 250, .. }));
        assert!(matches!(plans[1], CardPlan::Write { value: 250, .. }));
        assert!(
            matches!(&plans[2], CardPlan::SkippedDifferentUnit { name, .. } if name == "AMD Radeon Graphics")
        );
    }

    #[test]
    fn plan_all_preset_uses_each_cards_own_range() {
        let plans = plan_all(&[rtx3090("a"), amd_percent()], Target::PresetPercent(100));
        assert!(matches!(
            plans[0],
            CardPlan::Write {
                value: 350,
                unit: LimitUnit::Watts,
                ..
            }
        ));
        assert!(matches!(
            plans[1],
            CardPlan::Write {
                value: 15,
                unit: LimitUnit::Percent,
                ..
            }
        ));
    }
}
