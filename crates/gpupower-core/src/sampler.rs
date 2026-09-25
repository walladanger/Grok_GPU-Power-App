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
    use crate::{amd_raphael, AdapterSnapshot, Vendor};

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
