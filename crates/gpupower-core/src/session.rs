use crate::history::History;
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
    pub history: History,
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
                history: History::new(),
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
        let mut history = History::new();
        history.push(stamp(now, snapshot));
        cards.push(SessionCard {
            snapshot: snapshot.clone(),
            offline_since: None,
            history,
        });
    }
    Session { cards }
}

fn stamp(now: &str, snapshot: &AdapterSnapshot) -> StampedSample {
    StampedSample {
        at: now.to_string(),
        // An integer `now` is the shared unix second. Any other stamp stays at 0.
        unix: now.parse().unwrap_or(0),
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
    fn reconcile_integer_now_is_unix_and_other_stamps_stay_zero() {
        let card = rtx3090_primary();
        let opened = open_session(&[card.clone()]);
        let at_zero = reconcile(&opened, &[card.clone()], "0");
        assert_eq!(at_zero.cards[0].history[0].at, "0");
        assert_eq!(at_zero.cards[0].history[0].unix, 0);
        let later = reconcile(&at_zero, &[card.clone()], "901");
        assert_eq!(later.cards[0].history.len(), 1);
        assert_eq!(later.cards[0].history[0].unix, 901);
        let labeled = reconcile(&later, &[card], "t1");
        assert_eq!(labeled.cards[0].history.len(), 2);
        assert_eq!(labeled.cards[0].history[1].at, "t1");
        assert_eq!(labeled.cards[0].history[1].unix, 0);
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

    #[test]
    fn open_session_with_zero_one_and_several_cards() {
        let zero = open_session(&[]);
        assert!(zero.cards.is_empty());

        let one_card = rtx3090_primary();
        let one = open_session(std::slice::from_ref(&one_card));
        assert_eq!(one.cards.len(), 1);
        assert_eq!(one.cards[0].snapshot.id, one_card.id);
        assert!(one.cards[0].history.is_empty());
        assert!(one.cards[0].offline_since.is_none());

        let c1 = rtx3090_primary();
        let c2 = rtx3090_secondary();
        let c3 = amd_raphael();
        let several = open_session(&[c1.clone(), c2.clone(), c3.clone()]);
        assert_eq!(several.cards.len(), 3);
        assert_eq!(several.cards[0].snapshot.id, c1.id);
        assert_eq!(several.cards[1].snapshot.id, c2.id);
        assert_eq!(several.cards[2].snapshot.id, c3.id);
    }

    #[test]
    fn reconcile_when_card_is_new_disappears_and_reappears() {
        let card = rtx3090_primary();
        let session0 = open_session(&[]);

        let session1 = reconcile(&session0, std::slice::from_ref(&card), "t1");
        assert_eq!(session1.cards.len(), 1);
        assert_eq!(session1.cards[0].snapshot.id, card.id);
        assert!(session1.cards[0].offline_since.is_none());
        assert_eq!(session1.cards[0].history.len(), 1);
        assert_eq!(session1.cards[0].history[0].at, "t1");

        let session2 = reconcile(&session1, &[], "t2");
        assert_eq!(session2.cards.len(), 1);
        assert_eq!(session2.cards[0].snapshot.id, card.id);
        assert_eq!(session2.cards[0].offline_since.as_deref(), Some("t2"));
        assert_eq!(session2.cards[0].history.len(), 1);

        let session3 = reconcile(&session2, std::slice::from_ref(&card), "t3");
        assert_eq!(session3.cards.len(), 1);
        assert_eq!(session3.cards[0].snapshot.id, card.id);
        assert!(session3.cards[0].offline_since.is_none());
        assert_eq!(session3.cards[0].history.len(), 2);
        assert_eq!(session3.cards[0].history[1].at, "t3");
    }

    #[test]
    fn reconcile_when_ids_repeat() {
        let card = rtx3090_primary();
        let session0 = open_session(std::slice::from_ref(&card));
        let session1 = reconcile(&session0, &[card.clone(), card.clone()], "t1");
        assert_eq!(session1.cards.len(), 1);
        assert_eq!(session1.cards[0].snapshot.id, card.id);
        assert_eq!(session1.cards[0].history.len(), 1);
        assert_eq!(session1.cards[0].history[0].at, "t1");
    }

    #[test]
    fn stamp_output_fields_for_rfc3339_and_integer() {
        let card = rtx3090_primary();
        let session0 = open_session(&[]);

        let session1 = reconcile(
            &session0,
            std::slice::from_ref(&card),
            "2024-01-01T12:00:00Z",
        );
        assert_eq!(session1.cards.len(), 1);
        assert_eq!(session1.cards[0].history[0].at, "2024-01-01T12:00:00Z");
        assert_eq!(session1.cards[0].history[0].unix, 0);

        let session2 = reconcile(&session0, std::slice::from_ref(&card), "1700000000");
        assert_eq!(session2.cards.len(), 1);
        assert_eq!(session2.cards[0].history[0].at, "1700000000");
        assert_eq!(session2.cards[0].history[0].unix, 1700000000);
    }
}
