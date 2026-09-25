use crate::snapshot::AdapterSnapshot;

#[derive(Clone, Debug, PartialEq)]
pub struct StampedSample {
    pub at: String,
    pub unix: i64,
    pub snapshot: AdapterSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionCard {
    pub snapshot: AdapterSnapshot,
    pub offline_since: Option<String>,
    pub history: Vec<StampedSample>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub cards: Vec<SessionCard>,
}

pub fn open_session(seen: &[AdapterSnapshot]) -> Session {
    Session {
        cards: seen
            .iter()
            .map(|snapshot| SessionCard {
                snapshot: snapshot.clone(),
                offline_since: None,
                history: Vec::new(),
            })
            .collect(),
    }
}

pub fn reconcile(session: &Session, seen: &[AdapterSnapshot], now: &str) -> Session {
    let mut cards = Vec::with_capacity(session.cards.len());
    for card in &session.cards {
        if let Some(snapshot) = seen.iter().find(|item| item.id == card.snapshot.id) {
            let mut history = card.history.clone();
            history.push(stamp(now, snapshot));
            cards.push(SessionCard {
                snapshot: snapshot.clone(),
                offline_since: None,
                history,
            });
        } else {
            let mut kept = card.clone();
            if kept.offline_since.is_none() {
                kept.offline_since = Some(now.to_string());
            }
            cards.push(kept);
        }
    }
    for snapshot in seen {
        if cards.iter().any(|card| card.snapshot.id == snapshot.id) {
            continue;
        }
        cards.push(SessionCard {
            snapshot: snapshot.clone(),
            offline_since: None,
            history: vec![stamp(now, snapshot)],
        });
    }
    Session { cards }
}

fn stamp(now: &str, snapshot: &AdapterSnapshot) -> StampedSample {
    StampedSample {
        at: now.to_string(),
        // `now` is a display stamp. This function is not given a unix instant.
        unix: 0,
        snapshot: snapshot.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{amd_raphael, gtx1050_error, rtx3090_primary, rtx3090_secondary};
    use crate::Vendor;

    #[test]
    fn reconcile_keeps_uuid_history_when_twin_drops() {
        let a = rtx3090_primary();
        let b = rtx3090_secondary();
        let mut session = open_session(&[a.clone(), b.clone()]);
        session.cards[0].history.push(StampedSample {
            at: "t0".into(),
            unix: 0,
            snapshot: a.clone(),
        });
        session.cards[1].history.push(StampedSample {
            at: "t0".into(),
            unix: 0,
            snapshot: b.clone(),
        });
        let still_seen = [b.clone()];
        let next = reconcile(&session, &still_seen, "t1");
        let kept = next.cards.iter().find(|c| c.snapshot.id == b.id).unwrap();
        let gone = next.cards.iter().find(|c| c.snapshot.id == a.id).unwrap();
        assert!(kept.offline_since.is_none());
        assert_eq!(kept.history.len(), 2);
        assert_eq!(kept.history[0].snapshot.id, b.id);
        assert_eq!(kept.history[1].at, "t1");
        assert_eq!(kept.history[1].snapshot.id, b.id);
        assert_eq!(gone.offline_since.as_deref(), Some("t1"));
        assert_eq!(gone.history.len(), 1);
        assert_eq!(gone.history[0].snapshot.id, a.id);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn next_launch_omits_card_that_is_still_absent() {
        let session = open_session(&[rtx3090_secondary(), amd_raphael(), gtx1050_error()]);
        let ids: Vec<_> = session
            .cards
            .iter()
            .map(|c| c.snapshot.id.as_str())
            .collect();
        assert!(!ids.contains(&rtx3090_primary().id.as_str()));
        assert!(ids.iter().any(|id| id.contains("VEN_10DE&DEV_1C81")));
    }

    #[test]
    fn fixtures_do_not_invent_1050_power() {
        let card = gtx1050_error();
        assert_eq!(card.status, "Error");
        assert!(card.power_draw_instant_w.is_none());
        assert!(card.vendor == Vendor::Nvidia);
    }
}
