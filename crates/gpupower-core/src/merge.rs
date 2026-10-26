use crate::{empty_snapshot, AdapterSnapshot, Vendor};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAdapter {
    pub name: String,
    pub pnp_id: String,
    pub vendor_id: Option<u16>,
    pub status: String,
    pub driver_version: Option<String>,
    pub pci_bus_id: Option<String>,
}

// A vendor row matches one Windows adapter only when both PCI bus ids are
// non-empty and byte-for-byte equal. Matched Windows rows are not emitted again.
pub fn merge_adapters(
    windows: &[WindowsAdapter],
    nvidia: &[AdapterSnapshot],
    amd: &[AdapterSnapshot],
) -> Vec<AdapterSnapshot> {
    let mut nvidia_used = vec![false; nvidia.len()];
    let mut amd_used = vec![false; amd.len()];
    let mut merged = Vec::with_capacity(windows.len() + nvidia.len() + amd.len());

    for adapter in windows {
        if let Some(index) = matching_vendor(adapter, nvidia, &nvidia_used) {
            nvidia_used[index] = true;
            merged.push(with_windows_status(nvidia[index].clone(), adapter));
        } else if let Some(index) = matching_vendor(adapter, amd, &amd_used) {
            amd_used[index] = true;
            merged.push(with_windows_status(amd[index].clone(), adapter));
        } else {
            merged.push(snapshot_from_windows(adapter));
        }
    }

    push_unused(&mut merged, nvidia, &nvidia_used);
    push_unused(&mut merged, amd, &amd_used);
    merged
}

fn matching_vendor(
    adapter: &WindowsAdapter,
    cards: &[AdapterSnapshot],
    used: &[bool],
) -> Option<usize> {
    let bus = adapter
        .pci_bus_id
        .as_deref()
        .filter(|bus| !bus.is_empty())?;
    cards.iter().enumerate().position(|(index, card)| {
        !used[index] && !card.pci_bus_id.is_empty() && card.pci_bus_id == bus
    })
}

// Vendor status stays unless the vendor left it blank.
fn with_windows_status(mut card: AdapterSnapshot, adapter: &WindowsAdapter) -> AdapterSnapshot {
    if card.status.is_empty() {
        card.status = adapter.status.clone();
    }
    card
}

fn snapshot_from_windows(adapter: &WindowsAdapter) -> AdapterSnapshot {
    let mut card = empty_snapshot(
        adapter.pnp_id.clone(),
        vendor_from_id(adapter.vendor_id),
        adapter.name.clone(),
    );
    card.status = adapter.status.clone();
    card.driver_version = adapter.driver_version.clone();
    if let Some(pci_bus_id) = &adapter.pci_bus_id {
        card.pci_bus_id = pci_bus_id.clone();
    }
    card
}

// Windows vendor ids 0x10DE and 0x1002 are NVIDIA and AMD.
fn vendor_from_id(vendor_id: Option<u16>) -> Vendor {
    match vendor_id {
        Some(0x10DE) => Vendor::Nvidia,
        Some(0x1002) => Vendor::Amd,
        _ => Vendor::Other,
    }
}

fn push_unused(merged: &mut Vec<AdapterSnapshot>, cards: &[AdapterSnapshot], used: &[bool]) {
    for (card, seen) in cards.iter().zip(used.iter()) {
        if !seen {
            merged.push(card.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{amd_raphael, gtx1050_error, rtx3090_primary, rtx3090_secondary, Vendor};

    #[test]
    fn merge_keeps_error_adapter_without_a_vendor_sample() {
        let windows = vec![WindowsAdapter {
            name: "NVIDIA GeForce GTX 1050".into(),
            pnp_id: gtx1050_error().id,
            vendor_id: Some(0x10DE),
            status: "Error".into(),
            driver_version: Some("32.0.15.6094".into()),
            pci_bus_id: None,
        }];
        let merged = merge_adapters(&windows, &[], &[]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].status, "Error");
        assert!(merged[0].power_draw_instant_w.is_none());
        assert!(merged[0].vendor == Vendor::Nvidia);
    }

    #[test]
    fn merge_matches_pci_and_does_not_duplicate() {
        let card = rtx3090_primary();
        let windows = vec![WindowsAdapter {
            name: card.name.clone(),
            pnp_id: "PCI\\VEN_10DE&DEV_2204".into(),
            vendor_id: Some(0x10DE),
            status: "OK".into(),
            driver_version: card.driver_version.clone(),
            pci_bus_id: Some(card.pci_bus_id.clone()),
        }];
        let merged = merge_adapters(&windows, &[card.clone()], &[]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].power_draw_instant_w, card.power_draw_instant_w);
    }

    #[test]
    fn merge_with_zero_inputs_returns_empty() {
        let merged = merge_adapters(&[], &[], &[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_creates_snapshot_from_windows_when_unmatched() {
        let windows = vec![
            WindowsAdapter {
                name: "Unmatched NVIDIA".into(),
                pnp_id: "PCI\\VEN_10DE&DEV_9999".into(),
                vendor_id: Some(0x10DE),
                status: "OK".into(),
                driver_version: None,
                pci_bus_id: Some("bus1".into()),
            },
            WindowsAdapter {
                name: "Unmatched AMD".into(),
                pnp_id: "PCI\\VEN_1002&DEV_9999".into(),
                vendor_id: Some(0x1002),
                status: "Error".into(),
                driver_version: None,
                pci_bus_id: None,
            },
            WindowsAdapter {
                name: "Other Vendor".into(),
                pnp_id: "PCI\\VEN_1234".into(),
                vendor_id: Some(0x1234),
                status: "OK".into(),
                driver_version: None,
                pci_bus_id: Some("bus3".into()),
            },
            WindowsAdapter {
                name: "No Vendor".into(),
                pnp_id: "PCI\\VEN_NONE".into(),
                vendor_id: None,
                status: "Unknown".into(),
                driver_version: None,
                pci_bus_id: None,
            },
        ];
        let merged = merge_adapters(&windows, &[], &[]);
        assert_eq!(merged.len(), 4);

        assert!(matches!(merged[0].vendor, Vendor::Nvidia));
        assert_eq!(merged[0].id, "PCI\\VEN_10DE&DEV_9999");
        assert_eq!(merged[0].name, "Unmatched NVIDIA");
        assert_eq!(merged[0].status, "OK");
        assert_eq!(merged[0].pci_bus_id, "bus1");

        assert!(matches!(merged[1].vendor, Vendor::Amd));
        assert_eq!(merged[1].id, "PCI\\VEN_1002&DEV_9999");
        assert_eq!(merged[1].name, "Unmatched AMD");
        assert_eq!(merged[1].status, "Error");
        assert!(merged[1].pci_bus_id.is_empty());

        assert!(matches!(merged[2].vendor, Vendor::Other));
        assert_eq!(merged[2].id, "PCI\\VEN_1234");
        assert_eq!(merged[2].name, "Other Vendor");
        assert_eq!(merged[2].status, "OK");
        assert_eq!(merged[2].pci_bus_id, "bus3");

        assert!(matches!(merged[3].vendor, Vendor::Other));
        assert_eq!(merged[3].id, "PCI\\VEN_NONE");
        assert_eq!(merged[3].name, "No Vendor");
        assert_eq!(merged[3].status, "Unknown");
        assert!(merged[3].pci_bus_id.is_empty());
    }

    #[test]
    fn merge_copies_windows_status_only_if_card_status_is_empty() {
        let mut card1 = rtx3090_primary();
        card1.status = String::new(); // Empty string gets replaced
        let mut card2 = rtx3090_secondary();
        card2.status = "CardStatus".into(); // Non-empty string remains

        let windows = vec![
            WindowsAdapter {
                name: card1.name.clone(),
                pnp_id: "PNP1".into(),
                vendor_id: Some(0x10DE),
                status: "WinStatus1".into(),
                driver_version: None,
                pci_bus_id: Some(card1.pci_bus_id.clone()),
            },
            WindowsAdapter {
                name: card2.name.clone(),
                pnp_id: "PNP2".into(),
                vendor_id: Some(0x10DE),
                status: "WinStatus2".into(),
                driver_version: None,
                pci_bus_id: Some(card2.pci_bus_id.clone()),
            },
        ];

        let merged = merge_adapters(&windows, &[card1, card2], &[]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].status, "WinStatus1"); // Copied
        assert_eq!(merged[1].status, "CardStatus"); // Not copied
    }

    #[test]
    fn merge_appends_unmatched_vendor_cards_in_order() {
        let nvidia = vec![rtx3090_primary(), rtx3090_secondary()];
        let amd = vec![amd_raphael()];

        let merged = merge_adapters(&[], &nvidia, &amd);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].id, nvidia[0].id);
        assert_eq!(merged[1].id, nvidia[1].id);
        assert_eq!(merged[2].id, amd[0].id);
    }

    #[test]
    fn merge_matches_same_vendor_by_pci_bus_id_not_order() {
        let card1 = rtx3090_primary();
        let card2 = rtx3090_secondary();

        let windows = vec![WindowsAdapter {
            name: card2.name.clone(),
            pnp_id: "PNP".into(),
            vendor_id: Some(0x10DE),
            status: "OK".into(),
            driver_version: None,
            pci_bus_id: Some(card2.pci_bus_id.clone()),
        }];

        let merged = merge_adapters(&windows, &[card1.clone(), card2.clone()], &[]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, card2.id);
        assert_eq!(merged[1].id, card1.id);
    }

    #[test]
    fn merge_prevents_duplicate_matching() {
        let card = rtx3090_primary();

        let windows = vec![
            WindowsAdapter {
                name: "Match 1".into(),
                pnp_id: "PNP1".into(),
                vendor_id: Some(0x10DE),
                status: "OK".into(),
                driver_version: None,
                pci_bus_id: Some(card.pci_bus_id.clone()),
            },
            WindowsAdapter {
                name: "Match 2".into(),
                pnp_id: "PNP2".into(),
                vendor_id: Some(0x10DE),
                status: "OK".into(),
                driver_version: None,
                pci_bus_id: Some(card.pci_bus_id.clone()),
            },
        ];

        let merged = merge_adapters(&windows, &[card.clone()], &[]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, card.id);
        assert_eq!(merged[1].id, "PNP2"); // Unmatched fallback
    }
}
