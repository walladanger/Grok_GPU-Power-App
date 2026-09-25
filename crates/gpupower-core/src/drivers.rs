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
}
