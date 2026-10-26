use crate::merge::{merge_adapters, WindowsAdapter};
use crate::snapshot::AdapterSnapshot;

pub trait VendorSource {
    fn sample(&mut self) -> Result<Vec<AdapterSnapshot>, String>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    pub cards: Vec<AdapterSnapshot>,
    pub nvidia_error: Option<String>,
    pub amd_error: Option<String>,
}

pub struct Sampler<N, A> {
    pub nvidia: N,
    pub amd: A,
    pub windows: Vec<WindowsAdapter>,
}

impl<N: VendorSource, A: VendorSource> Sampler<N, A> {
    pub fn tick(&mut self) -> Tick {
        let (nvidia, nvidia_error) = take_sample(&mut self.nvidia);
        // An NVIDIA failure must not skip the AMD read.
        let (amd, amd_error) = take_sample(&mut self.amd);
        Tick {
            cards: merge_adapters(&self.windows, &nvidia, &amd),
            nvidia_error,
            amd_error,
        }
    }
}

fn take_sample<S: VendorSource>(source: &mut S) -> (Vec<AdapterSnapshot>, Option<String>) {
    match source.sample() {
        Ok(cards) => (cards, None),
        Err(error) => (Vec::new(), Some(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{amd_raphael, rtx3090_primary, AdapterSnapshot, Vendor};

    struct FakeSource {
        result: Result<Vec<AdapterSnapshot>, String>,
        calls: usize,
    }

    impl FakeSource {
        fn ok(cards: Vec<AdapterSnapshot>) -> Self {
            Self {
                result: Ok(cards),
                calls: 0,
            }
        }
        fn err(msg: &str) -> Self {
            Self {
                result: Err(msg.into()),
                calls: 0,
            }
        }
    }

    impl VendorSource for FakeSource {
        fn sample(&mut self) -> Result<Vec<AdapterSnapshot>, String> {
            self.calls += 1;
            self.result.clone()
        }
    }

    #[test]
    fn zero_adapters() {
        let mut sampler = Sampler {
            nvidia: FakeSource::ok(vec![]),
            amd: FakeSource::ok(vec![]),
            windows: vec![],
        };
        let tick = sampler.tick();
        assert!(tick.cards.is_empty());
        assert!(tick.nvidia_error.is_none());
        assert!(tick.amd_error.is_none());
    }

    #[test]
    fn nvidia_only() {
        let mut sampler = Sampler {
            nvidia: FakeSource::ok(vec![rtx3090_primary()]),
            amd: FakeSource::ok(vec![]),
            windows: vec![],
        };
        let tick = sampler.tick();
        assert_eq!(tick.cards.len(), 1);
        assert_eq!(tick.cards[0].vendor, Vendor::Nvidia);
        assert!(tick.nvidia_error.is_none());
        assert!(tick.amd_error.is_none());
    }

    #[test]
    fn amd_only() {
        let mut sampler = Sampler {
            nvidia: FakeSource::ok(vec![]),
            amd: FakeSource::ok(vec![amd_raphael()]),
            windows: vec![],
        };
        let tick = sampler.tick();
        assert_eq!(tick.cards.len(), 1);
        assert_eq!(tick.cards[0].vendor, Vendor::Amd);
        assert!(tick.nvidia_error.is_none());
        assert!(tick.amd_error.is_none());
    }

    #[test]
    fn both_sources() {
        let mut sampler = Sampler {
            nvidia: FakeSource::ok(vec![rtx3090_primary()]),
            amd: FakeSource::ok(vec![amd_raphael()]),
            windows: vec![],
        };
        let tick = sampler.tick();
        assert_eq!(tick.cards.len(), 2);
        assert_eq!(tick.cards[0].vendor, Vendor::Nvidia);
        assert_eq!(tick.cards[1].vendor, Vendor::Amd);
        assert!(tick.nvidia_error.is_none());
        assert!(tick.amd_error.is_none());
    }

    #[test]
    fn error_reports_correctly() {
        let mut sampler = Sampler {
            nvidia: FakeSource::err("nvidia fail"),
            amd: FakeSource::err("amd fail"),
            windows: vec![],
        };
        let tick = sampler.tick();
        assert!(tick.cards.is_empty());
        assert_eq!(tick.nvidia_error.as_deref(), Some("nvidia fail"));
        assert_eq!(tick.amd_error.as_deref(), Some("amd fail"));
    }

    #[test]
    fn repeated_ticks_count_calls() {
        let mut sampler = Sampler {
            nvidia: FakeSource::ok(vec![]),
            amd: FakeSource::ok(vec![]),
            windows: vec![],
        };
        sampler.tick();
        assert_eq!(sampler.nvidia.calls, 1);
        assert_eq!(sampler.amd.calls, 1);
        sampler.tick();
        assert_eq!(sampler.nvidia.calls, 2);
        assert_eq!(sampler.amd.calls, 2);
    }

    #[test]
    fn nvidia_failure_still_returns_amd_cards() {
        struct Boom;
        impl VendorSource for Boom {
            fn sample(&mut self) -> Result<Vec<AdapterSnapshot>, String> {
                Err("nvidia down".into())
            }
        }
        struct AmdOk;
        impl VendorSource for AmdOk {
            fn sample(&mut self) -> Result<Vec<AdapterSnapshot>, String> {
                Ok(vec![amd_raphael()])
            }
        }
        let mut sampler = Sampler {
            nvidia: Boom,
            amd: AmdOk,
            windows: vec![],
        };
        let tick = sampler.tick();
        assert_eq!(tick.nvidia_error.as_deref(), Some("nvidia down"));
        assert!(tick.cards.iter().any(|c| c.vendor == Vendor::Amd));
    }
}
