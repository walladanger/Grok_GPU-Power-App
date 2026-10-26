#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverPackage {
    pub class_name: String,
    pub version: String,
    pub date: String,
    pub inf_name: String,
    pub bound_device_id: String,
    pub is_previous_for_selected: bool,
}

pub fn rollback_candidate<'a>(
    selected_device_id: &str,
    packages: &'a [DriverPackage],
) -> Option<&'a DriverPackage> {
    packages.iter().find(|package| {
        package.class_name.eq_ignore_ascii_case("Display")
            && package.is_previous_for_selected
            && package.bound_device_id == selected_device_id
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gtx1050_error;

    #[test]
    fn rollback_rejects_other_device_and_nandisplay() {
        let selected = rtx3090_primary_pnp();
        let packages = vec![
            DriverPackage {
                class_name: "Display".into(),
                version: "32.0.15.6094".into(),
                date: "2024-08-13".into(),
                inf_name: "nv_dispig.inf".into(),
                bound_device_id: gtx1050_error().id,
                is_previous_for_selected: false,
            },
            DriverPackage {
                class_name: "System".into(),
                version: "1.0.0.12".into(),
                date: "2025-09-26".into(),
                inf_name: "amd3dvcache.inf".into(),
                bound_device_id: selected.clone(),
                is_previous_for_selected: true,
            },
            DriverPackage {
                class_name: "Display".into(),
                version: "32.0.15.7000".into(),
                date: "2025-01-02".into(),
                inf_name: "nvddsi.inf".into(),
                bound_device_id: selected.clone(),
                is_previous_for_selected: true,
            },
        ];
        let chosen = rollback_candidate(&selected, &packages).unwrap();
        assert_eq!(chosen.version, "32.0.15.7000");
        assert_eq!(chosen.class_name, "Display");
    }

    fn rtx3090_primary_pnp() -> String {
        "PCI\\VEN_10DE&DEV_2204&SUBSYS_38801028&REV_A1\\4&D0BDF66&0&0009".into()
    }

    #[test]
    fn rollback_accepts_display_case_insensitive_rejects_others() {
        let selected = rtx3090_primary_pnp();

        let reject_packages = vec![
            DriverPackage {
                class_name: "Net".into(),
                version: "1.0".into(),
                date: "2024".into(),
                inf_name: "net.inf".into(),
                bound_device_id: selected.clone(),
                is_previous_for_selected: true,
            },
            DriverPackage {
                class_name: "System".into(),
                version: "1.0".into(),
                date: "2024".into(),
                inf_name: "system.inf".into(),
                bound_device_id: selected.clone(),
                is_previous_for_selected: true,
            },
        ];

        assert!(rollback_candidate(&selected, &reject_packages).is_none());

        let mut accept_packages = vec![DriverPackage {
            class_name: "display".into(),
            version: "2.0".into(),
            date: "2024".into(),
            inf_name: "display1.inf".into(),
            bound_device_id: selected.clone(),
            is_previous_for_selected: true,
        }];

        let chosen = rollback_candidate(&selected, &accept_packages).unwrap();
        assert_eq!(chosen.class_name, "display");
        assert_eq!(chosen.inf_name, "display1.inf");

        accept_packages[0].class_name = "DISPLAY".into();
        let chosen2 = rollback_candidate(&selected, &accept_packages).unwrap();
        assert_eq!(chosen2.class_name, "DISPLAY");

        accept_packages[0].class_name = "Display".into();
        let chosen3 = rollback_candidate(&selected, &accept_packages).unwrap();
        assert_eq!(chosen3.class_name, "Display");
    }

    #[test]
    fn rollback_rejects_false_is_previous_for_selected() {
        let selected = rtx3090_primary_pnp();
        let packages = vec![DriverPackage {
            class_name: "Display".into(),
            version: "1.0".into(),
            date: "2024".into(),
            inf_name: "display1.inf".into(),
            bound_device_id: selected.clone(),
            is_previous_for_selected: false,
        }];
        assert!(rollback_candidate(&selected, &packages).is_none());
    }

    #[test]
    fn rollback_rejects_mismatched_bound_device_id() {
        let selected = rtx3090_primary_pnp();
        let different_id = crate::fixtures::rtx3090_secondary().id;

        let packages = vec![
            DriverPackage {
                class_name: "Display".into(),
                version: "1.0".into(),
                date: "2024".into(),
                inf_name: "display1.inf".into(),
                bound_device_id: different_id,
                is_previous_for_selected: true,
            },
            DriverPackage {
                class_name: "Display".into(),
                version: "1.0".into(),
                date: "2024".into(),
                inf_name: "display2.inf".into(),
                bound_device_id: selected.to_lowercase(),
                is_previous_for_selected: true,
            },
        ];

        assert!(rollback_candidate(&selected, &packages).is_none());
    }

    #[test]
    fn rollback_returns_none_for_empty_list() {
        let selected = rtx3090_primary_pnp();
        let packages: Vec<DriverPackage> = vec![];
        assert!(rollback_candidate(&selected, &packages).is_none());
    }

    #[test]
    fn rollback_returns_first_qualifying_package() {
        let selected = rtx3090_primary_pnp();
        let packages = vec![
            DriverPackage {
                class_name: "Display".into(),
                version: "1.0".into(),
                date: "2024".into(),
                inf_name: "display1.inf".into(),
                bound_device_id: selected.clone(),
                is_previous_for_selected: true,
            },
            DriverPackage {
                class_name: "Display".into(),
                version: "2.0".into(),
                date: "2024".into(),
                inf_name: "display2.inf".into(),
                bound_device_id: selected.clone(),
                is_previous_for_selected: true,
            },
        ];

        let chosen = rollback_candidate(&selected, &packages).unwrap();
        assert_eq!(chosen.inf_name, "display1.inf");
        assert_eq!(chosen.version, "1.0");
        assert!(std::ptr::eq(chosen, &packages[0]));
    }
}
