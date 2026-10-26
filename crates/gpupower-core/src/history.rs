use std::collections::VecDeque;
use std::ops::Index;

use serde::{Deserialize, Serialize};

use crate::session::StampedSample;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GraphRange {
    Min1,
    #[default]
    Min5,
    Min15,
}

#[derive(Clone, Debug, PartialEq)]
pub struct History {
    samples: VecDeque<StampedSample>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
        }
    }

    pub fn push(&mut self, sample: StampedSample) {
        let newest = sample.unix;
        self.samples.push_back(sample);
        let cutoff = newest - 15 * 60;
        while self
            .samples
            .front()
            .is_some_and(|oldest| oldest.unix < cutoff)
        {
            self.samples.pop_front();
        }
        // Callers borrow one slice. Compacting stops the ring from wrapping.
        self.samples.make_contiguous();
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn window(&self, range: GraphRange) -> Vec<&StampedSample> {
        let Some(newest) = self.samples.back().map(|sample| sample.unix) else {
            return Vec::new();
        };
        let span = match range {
            GraphRange::Min1 => 60,
            GraphRange::Min5 => 5 * 60,
            GraphRange::Min15 => 15 * 60,
        };
        self.samples
            .iter()
            .filter(|sample| newest - sample.unix <= span)
            .collect()
    }

    pub fn as_slice(&self) -> &[StampedSample] {
        let (front, back) = self.samples.as_slices();
        assert!(
            back.is_empty(),
            "history must stay contiguous so callers can share one slice"
        );
        front
    }
}

impl Index<usize> for History {
    type Output = StampedSample;

    fn index(&self, index: usize) -> &Self::Output {
        &self.samples[index]
    }
}

pub fn snap_index(samples: &[StampedSample], target_unix: i64) -> Option<usize> {
    let mut best_index = None;
    let mut best_distance = u64::MAX;
    for (index, sample) in samples.iter().enumerate() {
        let distance = sample.unix.abs_diff(target_unix);
        if distance < best_distance {
            best_distance = distance;
            best_index = Some(index);
        }
    }
    best_index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::rtx3090_primary;
    use crate::StampedSample;

    fn sample_at(unix: i64) -> StampedSample {
        StampedSample {
            at: unix.to_string(),
            unix,
            snapshot: rtx3090_primary(),
        }
    }

    #[test]
    fn snap_picks_nearest_second_and_shares_that_timestamp() {
        let mut a = History::new();
        let mut b = History::new();
        for sec in [10_i64, 11, 12] {
            a.push(sample_at(sec));
            b.push(sample_at(sec));
        }
        let idx = snap_index(a.as_slice(), 11).unwrap();
        let shared = a.as_slice()[idx].unix;
        assert_eq!(snap_index(b.as_slice(), shared), Some(idx));
        assert_eq!(shared, 11);
    }

    #[test]
    fn equidistant_snap_picks_the_earlier_sample() {
        let samples = vec![sample_at(10), sample_at(12)];
        assert_eq!(snap_index(&samples, 11), Some(0));
    }

    #[test]
    fn ring_drops_samples_older_than_15_minutes() {
        let mut h = History::new();
        h.push(sample_at(0));
        h.push(sample_at(15 * 60 + 1));
        assert_eq!(h.len(), 1);
        assert_eq!(h.as_slice()[0].unix, 901);
    }

    #[test]
    fn five_minute_window_hides_older_samples() {
        let mut h = History::new();
        h.push(sample_at(0));
        h.push(sample_at(600));
        assert_eq!(h.window(GraphRange::Min5).len(), 1);
    }

    #[test]
    fn test_history_new_is_empty() {
        let h = History::new();
        assert_eq!(h.len(), 0);
        assert!(h.is_empty());
        assert!(h.as_slice().is_empty());
    }

    #[test]
    fn test_push_beyond_retention_limit_drops_oldest() {
        let mut h = History::new();
        h.push(sample_at(0));
        h.push(sample_at(15 * 60 - 1));
        h.push(sample_at(15 * 60));
        h.push(sample_at(15 * 60 + 1));
        assert_eq!(h.len(), 3);
        assert_eq!(h.as_slice()[0].unix, 899);
        assert_eq!(h.as_slice()[2].unix, 901);
    }

    #[test]
    fn test_window_range_inclusivity() {
        let mut h = History::new();
        for unix in [99, 100, 699, 700, 939, 940, 1000] {
            h.push(sample_at(unix));
        }

        let min1 = h.window(GraphRange::Min1);
        assert_eq!(min1.len(), 2);
        assert_eq!(min1[0].unix, 940);
        assert_eq!(min1[1].unix, 1000);

        let min5 = h.window(GraphRange::Min5);
        assert_eq!(min5.len(), 4);
        assert_eq!(min5[0].unix, 700);
        assert_eq!(min5[1].unix, 939);
        assert_eq!(min5[2].unix, 940);
        assert_eq!(min5[3].unix, 1000);

        let min15 = h.window(GraphRange::Min15);
        assert_eq!(min15.len(), 6);
        assert_eq!(min15[0].unix, 100);
        assert_eq!(min15[5].unix, 1000);
    }

    #[test]
    fn test_snap_index_cases() {
        let empty: &[StampedSample] = &[];
        assert_eq!(snap_index(empty, 5), None);

        let samples = [sample_at(10), sample_at(20)];
        assert_eq!(snap_index(&samples, 5), Some(0));
        assert_eq!(snap_index(&samples, 25), Some(1));
        assert_eq!(snap_index(&samples, 20), Some(1));
        assert_eq!(snap_index(&samples, 15), Some(0));
    }
}
