use serde::Serialize;

use crate::{LimitRange, LimitUnit, SavedLimit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelperCommand {
    ApplySaved,
    ApplyLimit {
        id: String,
        unit: LimitUnit,
        value: i32,
    },
    Reinstall {
        inf: String,
        device_ids: Vec<String>,
    },
    Rollback {
        device_id: String,
    },
    InstallReapplyTask,
    RemoveReapplyTask,
}

pub fn parse_helper_args(args: &[String]) -> Result<HelperCommand, String> {
    let Some(command) = args.first() else {
        return Err("missing helper command".into());
    };
    match command.as_str() {
        "--apply-saved" => bare(args, HelperCommand::ApplySaved),
        "--install-reapply-task" => bare(args, HelperCommand::InstallReapplyTask),
        "--remove-reapply-task" => bare(args, HelperCommand::RemoveReapplyTask),
        "--apply-limit" => parse_apply_limit(&args[1..]),
        "--reinstall-driver" => parse_reinstall(&args[1..]),
        "--rollback-driver" => parse_rollback(&args[1..]),
        other => Err(format!("unknown helper command: {other}")),
    }
}

fn bare(args: &[String], command: HelperCommand) -> Result<HelperCommand, String> {
    if args.len() == 1 {
        Ok(command)
    } else {
        Err(format!("unexpected argument: {}", args[1]))
    }
}

fn parse_apply_limit(args: &[String]) -> Result<HelperCommand, String> {
    let mut id = None;
    let mut unit = None;
    let mut value = None;
    for (flag, raw) in pairs(args)? {
        match flag {
            "--id" => assign(&mut id, raw.to_owned(), "--id")?,
            "--unit" => assign(&mut unit, parse_unit(raw)?, "--unit")?,
            "--value" => assign(&mut value, parse_value(raw)?, "--value")?,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    match (id, unit, value) {
        (Some(id), Some(unit), Some(value)) => Ok(HelperCommand::ApplyLimit { id, unit, value }),
        _ => Err("--apply-limit requires --id, --unit, and --value".into()),
    }
}

fn parse_reinstall(args: &[String]) -> Result<HelperCommand, String> {
    let mut inf = None;
    let mut device_ids = Vec::new();
    for (flag, raw) in pairs(args)? {
        match flag {
            "--inf" => assign(&mut inf, raw.to_owned(), "--inf")?,
            "--device" => device_ids.push(raw.to_owned()),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let Some(inf) = inf else {
        return Err("--reinstall-driver requires --inf".into());
    };
    if device_ids.is_empty() {
        return Err("--reinstall-driver requires --device".into());
    }
    Ok(HelperCommand::Reinstall { inf, device_ids })
}

fn parse_rollback(args: &[String]) -> Result<HelperCommand, String> {
    let mut device_id = None;
    for (flag, raw) in pairs(args)? {
        match flag {
            "--device" => assign(&mut device_id, raw.to_owned(), "--device")?,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let Some(device_id) = device_id else {
        return Err("--rollback-driver requires --device".into());
    };
    Ok(HelperCommand::Rollback { device_id })
}

fn pairs(args: &[String]) -> Result<Vec<(&str, &str)>, String> {
    let mut parsed = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if !flag.starts_with("--") {
            return Err(format!("unexpected argument: {flag}"));
        }
        let Some(value) = args.get(index + 1) else {
            return Err(format!("missing value for {flag}"));
        };
        if value.starts_with("--") {
            return Err(format!("missing value for {flag}"));
        }
        parsed.push((flag, value.as_str()));
        index += 2;
    }
    Ok(parsed)
}

fn assign<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("duplicate {flag}"));
    }
    *slot = Some(value);
    Ok(())
}

fn parse_unit(text: &str) -> Result<LimitUnit, String> {
    match text {
        "watts" => Ok(LimitUnit::Watts),
        "percent" => Ok(LimitUnit::Percent),
        other => Err(format!("unknown unit: {other}")),
    }
}

fn parse_value(text: &str) -> Result<i32, String> {
    text.parse::<i32>()
        .map_err(|_| format!("invalid --value: {text}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReapplyPlan {
    SkipAbsent,
    Write { value: i32, clamped: bool },
}

pub fn plan_reapply(saved: &SavedLimit, live: Option<&LimitRange>) -> ReapplyPlan {
    let Some(live) = live else {
        return ReapplyPlan::SkipAbsent;
    };
    // A present card with a different unit is skipped, same as an absent card.
    if live.unit != saved.unit {
        return ReapplyPlan::SkipAbsent;
    }
    let value = saved.value.clamp(live.min, live.max);
    ReapplyPlan::Write {
        value,
        clamped: value != saved.value,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowUp {
    NotifyRebootRequired,
    ReapplyOnNextSignIn,
}

pub fn follow_up(reboot_required: bool) -> Vec<FollowUp> {
    if reboot_required {
        vec![
            FollowUp::NotifyRebootRequired,
            FollowUp::ReapplyOnNextSignIn,
        ]
    } else {
        Vec::new()
    }
}

pub fn partial_apply_message(results: &[(String, Result<(), String>)]) -> String {
    let failed: Vec<&str> = results
        .iter()
        .filter_map(|(name, result)| result.as_ref().err().map(|_| name.as_str()))
        .collect();
    if failed.is_empty() {
        "Applied.".to_owned()
    } else {
        failed.join(", ")
    }
}

pub fn approval_cancelled_message() -> &'static str {
    "Administrator approval was cancelled. The limit is unchanged."
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HelperReport {
    pub ok: bool,
    pub message: String,
    pub reboot_required: bool,
}

pub fn format_helper_stdout(report: &HelperReport) -> String {
    serde_json::to_string(report).expect("HelperReport serializes to JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LimitRange, LimitUnit, SavedLimit};

    #[test]
    fn reapply_clamps_skips_and_does_not_reboot() {
        let saved = SavedLimit {
            unit: LimitUnit::Watts,
            value: 50,
        };
        let live = LimitRange {
            unit: LimitUnit::Watts,
            min: 100,
            max: 350,
            default_value: 350,
            current: 350,
            enforced: 350,
        };
        assert!(matches!(
            plan_reapply(&saved, Some(&live)),
            ReapplyPlan::Write {
                value: 100,
                clamped: true
            }
        ));
        assert!(matches!(
            plan_reapply(&saved, None),
            ReapplyPlan::SkipAbsent
        ));
        let steps = follow_up(true);
        assert!(steps.contains(&FollowUp::NotifyRebootRequired));
        assert!(!format!("{steps:?}").contains("RebootNow"));
    }

    #[test]
    fn parser_rejects_fan_and_accepts_apply_limit() {
        assert!(parse_helper_args(&["--set-fan".into(), "40".into()]).is_err());
        let cmd = parse_helper_args(&[
            "--apply-limit".into(),
            "--id".into(),
            "GPU-a".into(),
            "--unit".into(),
            "watts".into(),
            "--value".into(),
            "280".into(),
        ])
        .unwrap();
        assert!(matches!(
            cmd,
            HelperCommand::ApplyLimit {
                value: 280,
                unit: LimitUnit::Watts,
                ..
            }
        ));
        assert!(matches!(
            parse_helper_args(&["--install-reapply-task".into()]).unwrap(),
            HelperCommand::InstallReapplyTask
        ));
    }

    #[test]
    fn partial_apply_names_the_failed_card() {
        let text = partial_apply_message(&[
            ("NVIDIA GeForce RTX 3090".into(), Ok(())),
            ("AMD Radeon Graphics".into(), Err("refused".into())),
        ]);
        assert!(text.contains("AMD Radeon Graphics"));
    }

    #[test]
    fn cancelled_approval_has_stable_copy() {
        assert_eq!(
            approval_cancelled_message(),
            "Administrator approval was cancelled. The limit is unchanged."
        );
    }

    #[test]
    fn parse_helper_args_success() {
        assert!(matches!(
            parse_helper_args(&["--apply-saved".into()]).unwrap(),
            HelperCommand::ApplySaved
        ));
        assert!(matches!(
            parse_helper_args(&["--install-reapply-task".into()]).unwrap(),
            HelperCommand::InstallReapplyTask
        ));
        assert!(matches!(
            parse_helper_args(&["--remove-reapply-task".into()]).unwrap(),
            HelperCommand::RemoveReapplyTask
        ));

        let apply_limit = parse_helper_args(&[
            "--apply-limit".into(),
            "--id".into(),
            "foo".into(),
            "--unit".into(),
            "watts".into(),
            "--value".into(),
            "100".into(),
        ])
        .unwrap();
        assert!(matches!(
            apply_limit,
            HelperCommand::ApplyLimit {
                id,
                unit: LimitUnit::Watts,
                value: 100,
            } if id == "foo"
        ));

        let reinstall = parse_helper_args(&[
            "--reinstall-driver".into(),
            "--inf".into(),
            "foo.inf".into(),
            "--device".into(),
            "PCI\\VEN_1234".into(),
        ])
        .unwrap();
        assert!(matches!(
            reinstall,
            HelperCommand::Reinstall {
                inf,
                device_ids,
            } if inf == "foo.inf" && device_ids == vec!["PCI\\VEN_1234"]
        ));

        let rollback = parse_helper_args(&[
            "--rollback-driver".into(),
            "--device".into(),
            "PCI\\VEN_1234".into(),
        ])
        .unwrap();
        assert!(matches!(
            rollback,
            HelperCommand::Rollback {
                device_id,
            } if device_id == "PCI\\VEN_1234"
        ));
    }

    #[test]
    fn parse_helper_args_errors() {
        let err = parse_helper_args(&[]).unwrap_err();
        assert_eq!(err, "missing helper command");

        let err = parse_helper_args(&["--unknown-cmd".into()]).unwrap_err();
        assert_eq!(err, "unknown helper command: --unknown-cmd");

        let err = parse_helper_args(&["--apply-saved".into(), "foo".into()]).unwrap_err();
        assert_eq!(err, "unexpected argument: foo");

        let err = parse_helper_args(&["--apply-limit".into(), "foo".into()]).unwrap_err();
        assert_eq!(err, "unexpected argument: foo");

        let err = parse_helper_args(&["--apply-limit".into(), "--id".into()]).unwrap_err();
        assert_eq!(err, "missing value for --id");

        let err = parse_helper_args(&["--apply-limit".into(), "--id".into(), "--unit".into()])
            .unwrap_err();
        assert_eq!(err, "missing value for --id");

        let err = parse_helper_args(&[
            "--apply-limit".into(),
            "--id".into(),
            "a".into(),
            "--id".into(),
            "b".into(),
        ])
        .unwrap_err();
        assert_eq!(err, "duplicate --id");

        let err = parse_helper_args(&["--apply-limit".into(), "--unknown".into(), "val".into()])
            .unwrap_err();
        assert_eq!(err, "unknown argument: --unknown");

        let err = parse_helper_args(&[
            "--apply-limit".into(),
            "--id".into(),
            "a".into(),
            "--unit".into(),
            "watts".into(),
        ])
        .unwrap_err();
        assert_eq!(err, "--apply-limit requires --id, --unit, and --value");

        let err = parse_helper_args(&[
            "--apply-limit".into(),
            "--id".into(),
            "a".into(),
            "--unit".into(),
            "foo".into(),
            "--value".into(),
            "10".into(),
        ])
        .unwrap_err();
        assert_eq!(err, "unknown unit: foo");

        let err = parse_helper_args(&[
            "--apply-limit".into(),
            "--id".into(),
            "a".into(),
            "--unit".into(),
            "watts".into(),
            "--value".into(),
            "foo".into(),
        ])
        .unwrap_err();
        assert_eq!(err, "invalid --value: foo");

        let err = parse_helper_args(&["--reinstall-driver".into(), "--device".into(), "a".into()])
            .unwrap_err();
        assert_eq!(err, "--reinstall-driver requires --inf");

        let err = parse_helper_args(&["--reinstall-driver".into(), "--inf".into(), "a".into()])
            .unwrap_err();
        assert_eq!(err, "--reinstall-driver requires --device");

        let err = parse_helper_args(&["--rollback-driver".into(), "--inf".into(), "a".into()])
            .unwrap_err();
        assert_eq!(err, "unknown argument: --inf");

        let err = parse_helper_args(&["--rollback-driver".into()]).unwrap_err();
        assert_eq!(err, "--rollback-driver requires --device");
    }

    #[test]
    fn plan_reapply_tests() {
        let saved = SavedLimit {
            unit: LimitUnit::Watts,
            value: 100,
        };

        assert!(matches!(
            plan_reapply(&saved, None),
            ReapplyPlan::SkipAbsent
        ));

        let live_percent = LimitRange {
            unit: LimitUnit::Percent,
            min: 50,
            max: 150,
            default_value: 100,
            current: 100,
            enforced: 100,
        };
        assert!(matches!(
            plan_reapply(&saved, Some(&live_percent)),
            ReapplyPlan::SkipAbsent
        ));

        let live_watts = LimitRange {
            unit: LimitUnit::Watts,
            min: 50,
            max: 150,
            default_value: 100,
            current: 100,
            enforced: 100,
        };
        assert!(matches!(
            plan_reapply(&saved, Some(&live_watts)),
            ReapplyPlan::Write {
                value: 100,
                clamped: false
            }
        ));

        let saved_low = SavedLimit {
            unit: LimitUnit::Watts,
            value: 30,
        };
        assert!(matches!(
            plan_reapply(&saved_low, Some(&live_watts)),
            ReapplyPlan::Write {
                value: 50,
                clamped: true
            }
        ));

        let saved_high = SavedLimit {
            unit: LimitUnit::Watts,
            value: 200,
        };
        assert!(matches!(
            plan_reapply(&saved_high, Some(&live_watts)),
            ReapplyPlan::Write {
                value: 150,
                clamped: true
            }
        ));
    }

    #[test]
    fn follow_up_tests() {
        let steps_false = follow_up(false);
        assert!(steps_false.is_empty());

        let steps_true = follow_up(true);
        assert!(
            steps_true
                == vec![
                    FollowUp::NotifyRebootRequired,
                    FollowUp::ReapplyOnNextSignIn
                ]
        );
    }

    #[test]
    fn partial_apply_message_tests() {
        let msg_ok = partial_apply_message(&[("GPU1".into(), Ok(()))]);
        assert_eq!(msg_ok, "Applied.");

        let msg_mixed =
            partial_apply_message(&[("GPU1".into(), Ok(())), ("GPU2".into(), Err("fail".into()))]);
        assert_eq!(msg_mixed, "GPU2");

        let msg_failed = partial_apply_message(&[
            ("GPU1".into(), Err("fail".into())),
            ("GPU2".into(), Err("fail".into())),
        ]);
        assert_eq!(msg_failed, "GPU1, GPU2");
    }

    #[test]
    fn approval_cancelled_message_tests() {
        assert!(
            approval_cancelled_message()
                == "Administrator approval was cancelled. The limit is unchanged."
        );
    }

    #[test]
    fn format_helper_stdout_tests() {
        let report = HelperReport {
            ok: true,
            message: "Done".to_string(),
            reboot_required: false,
        };
        let json = format_helper_stdout(&report);
        assert_eq!(
            json,
            r#"{"ok":true,"message":"Done","reboot_required":false}"#
        );
    }
}
