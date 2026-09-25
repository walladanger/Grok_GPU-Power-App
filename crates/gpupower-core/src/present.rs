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
    let cards = cards
        .iter()
        .filter(|card| !percent_without_watt_ceiling(card));
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

// Percent-unit cards with no watt ceiling are left out of every icon mode.
fn percent_without_watt_ceiling(card: &AdapterSnapshot) -> bool {
    matches!(
        card.limit.as_ref().map(|limit| limit.unit),
        Some(LimitUnit::Percent)
    ) && card.watt_ceiling_w.is_none()
}

// (value + 0.5) as i32 is half-up for a non-negative tray reading.
fn round_non_negative(value: f64) -> i32 {
    (value + 0.5) as i32
}

// Watts cards use enforced watts. Percent cards use their watt ceiling.
// A non-positive cap is not a usable percent base.
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
    use crate::{gtx1050_error, rtx3090_primary, rtx3090_secondary, IconMode};

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
    fn stale_is_strictly_older_than_three_seconds() {
        assert!(!is_stale(3000));
        assert!(is_stale(3001));
    }
}
