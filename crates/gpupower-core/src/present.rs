use crate::{AdapterSnapshot, IconMode, LimitUnit};

#[derive(Debug, Clone, PartialEq)]
pub struct CombinedPower {
    pub watts: f64,
    pub missing_names: Vec<String>,
}

pub fn combined_power(cards: &[AdapterSnapshot]) -> CombinedPower {
    let mut watts = 0.0;
    let mut missing_names = Vec::new();
    for card in cards {
        match card.power_draw_instant_w {
            Some(draw) => watts += draw,
            None => missing_names.push(card.name.clone()),
        }
    }
    CombinedPower {
        watts,
        missing_names,
    }
}

pub fn icon_number(mode: IconMode, cards: &[AdapterSnapshot]) -> Option<i32> {
    match mode {
        IconMode::Combined => {
            let mut watts = 0.0;
            let mut any = false;
            for card in cards {
                if let Some(draw) = card.power_draw_instant_w {
                    watts += draw;
                    any = true;
                }
            }
            any.then_some(round_non_negative(watts))
        }
        IconMode::Busiest => {
            let mut best: Option<f64> = None;
            for card in cards {
                if let Some(draw) = card.power_draw_instant_w {
                    best = Some(match best {
                        Some(current) => current.max(draw),
                        None => draw,
                    });
                }
            }
            best.map(round_non_negative)
        }
        IconMode::HighestPercent => {
            let mut best: Option<i32> = None;
            for card in cards {
                let Some(percent) = percent_of_limit(card) else {
                    continue;
                };
                let rounded = round_non_negative(percent);
                best = Some(match best {
                    Some(current) => current.max(rounded),
                    None => rounded,
                });
            }
            best
        }
    }
}

pub fn is_stale(age_ms: u64) -> bool {
    age_ms > 3000
}

// (value + 0.5) as i32 is half-up for a non-negative tray reading.
fn round_non_negative(value: f64) -> i32 {
    (value + 0.5) as i32
}

// Highest percent only. Combined and busiest still count the card's watts.
// Watts cards use a positive enforced limit. Percent cards use a positive ceiling.
fn percent_of_limit(card: &AdapterSnapshot) -> Option<f64> {
    let instant = card.power_draw_instant_w?;
    let limit = card.limit.as_ref()?;
    let denominator = match limit.unit {
        LimitUnit::Watts => {
            if limit.enforced <= 0 {
                return None;
            }
            f64::from(limit.enforced)
        }
        LimitUnit::Percent => {
            let ceiling = card.watt_ceiling_w?;
            if ceiling <= 0.0 {
                return None;
            }
            ceiling
        }
    };
    Some(instant / denominator * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        amd_raphael, empty_snapshot, gtx1050_error, rtx3090_primary, rtx3090_secondary, IconMode,
        LimitRange, LimitUnit, Vendor,
    };

    fn percent_card_without_ceiling() -> AdapterSnapshot {
        let mut card = empty_snapshot(
            "amd-percent".into(),
            Vendor::Amd,
            "AMD Radeon Graphics".into(),
        );
        card.can_set_limit = false;
        card.watt_ceiling_w = None;
        card.power_draw_instant_w = Some(40.0);
        card.limit = Some(LimitRange {
            unit: LimitUnit::Percent,
            min: -30,
            max: 15,
            default_value: 0,
            current: 0,
            enforced: 0,
        });
        card
    }

    #[test]
    fn combined_power_sums_cards_that_have_a_reading() {
        let mut dead = gtx1050_error();
        dead.name = "NVIDIA GeForce GTX 1050".into();
        let sum = combined_power(&[rtx3090_primary(), rtx3090_secondary(), dead]);
        assert!((sum.watts - (8.24 + 7.73)).abs() < 0.01);
        assert_eq!(
            sum.missing_names,
            vec!["NVIDIA GeForce GTX 1050".to_string()]
        );
    }

    #[test]
    fn icon_modes_round_to_whole_numbers() {
        let cards = [rtx3090_primary(), rtx3090_secondary()];
        assert_eq!(icon_number(IconMode::Combined, &cards), Some(16));
        assert_eq!(icon_number(IconMode::Busiest, &cards), Some(8));
        assert_eq!(icon_number(IconMode::HighestPercent, &cards), Some(2));
    }

    #[test]
    fn percent_without_ceiling_counts_toward_combined_and_busiest() {
        let cards = [percent_card_without_ceiling(), rtx3090_primary()];
        assert_eq!(icon_number(IconMode::Combined, &cards), Some(48));
        assert_eq!(icon_number(IconMode::Busiest, &cards), Some(40));
        assert_eq!(icon_number(IconMode::HighestPercent, &cards), Some(2));
    }

    #[test]
    fn stale_is_strictly_older_than_three_seconds() {
        assert!(!is_stale(3000));
        assert!(is_stale(3001));
    }

    #[test]
    fn combined_power_no_cards() {
        let sum = combined_power(&[]);
        assert_eq!(
            sum,
            CombinedPower {
                watts: 0.0,
                missing_names: vec![]
            }
        );
    }

    #[test]
    fn combined_power_all_none() {
        let sum = combined_power(&[gtx1050_error(), amd_raphael()]);
        assert_eq!(sum.watts, 0.0);
        assert_eq!(
            sum.missing_names,
            vec![
                "NVIDIA GeForce GTX 1050".to_string(),
                "AMD Radeon Graphics".to_string()
            ]
        );
    }

    #[test]
    fn combined_power_mixed_watts_percent() {
        let sum = combined_power(&[rtx3090_primary(), percent_card_without_ceiling()]);
        assert_eq!(sum.watts, 48.24);
        assert!(sum.missing_names.is_empty());
    }

    #[test]
    fn icon_number_empty_and_no_readings() {
        assert_eq!(icon_number(IconMode::Combined, &[]), None);
        assert_eq!(icon_number(IconMode::Busiest, &[]), None);
        assert_eq!(icon_number(IconMode::HighestPercent, &[]), None);

        let dead_cards = [gtx1050_error(), amd_raphael()];
        assert_eq!(icon_number(IconMode::Combined, &dead_cards), None);
        assert_eq!(icon_number(IconMode::Busiest, &dead_cards), None);
        assert_eq!(icon_number(IconMode::HighestPercent, &dead_cards), None);
    }

    #[test]
    fn icon_number_round_non_negative() {
        let mut card1 = rtx3090_primary();
        card1.power_draw_instant_w = Some(-0.7);
        let mut card2 = rtx3090_primary();
        card2.power_draw_instant_w = Some(2.5);

        assert_eq!(icon_number(IconMode::Busiest, &[card1]), Some(0));
        assert_eq!(icon_number(IconMode::Busiest, &[card2]), Some(3));
    }

    #[test]
    fn percent_of_limit_paths() {
        // Watts card with a valid limit (8.24 / 350.0 * 100.0 = 2.35...) rounds to 2
        let card_watts_valid = rtx3090_primary();
        assert_eq!(
            icon_number(IconMode::HighestPercent, &[card_watts_valid]),
            Some(2)
        );

        // Watts card with enforced <= 0
        let mut card_watts_zero = rtx3090_primary();
        if let Some(limit) = card_watts_zero.limit.as_mut() {
            limit.enforced = 0;
        }
        assert_eq!(
            icon_number(IconMode::HighestPercent, &[card_watts_zero]),
            None
        );

        // Percent card with watt_ceiling_w > 0 (40.0 / 80.0 * 100.0 = 50.0) rounds to 50
        let mut card_percent_valid = percent_card_without_ceiling();
        card_percent_valid.watt_ceiling_w = Some(80.0);
        assert_eq!(
            icon_number(IconMode::HighestPercent, &[card_percent_valid]),
            Some(50)
        );

        // Percent card with watt_ceiling_w <= 0.0
        let mut card_percent_zero = percent_card_without_ceiling();
        card_percent_zero.watt_ceiling_w = Some(0.0);
        assert_eq!(
            icon_number(IconMode::HighestPercent, &[card_percent_zero]),
            None
        );

        // Percent card without watt ceiling (watt_ceiling_w is None)
        let card_percent_none = percent_card_without_ceiling();
        assert_eq!(
            icon_number(IconMode::HighestPercent, &[card_percent_none]),
            None
        );
    }

    #[test]
    fn is_stale_additional_thresholds() {
        assert!(!is_stale(0));
        assert!(!is_stale(2999));
    }
}
