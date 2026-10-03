//! Rewards for taking part (ADR-169, as amended by ADR-172): your points, the leaderboard, badges,
//! "**Thanked by 12 people**", and **Getting started**.
//!
//! 8 West works out points and badges from its own records; this PC never sends points (contract
//! §12). The free month of Pro for invitations (ADR-169 §6) is not part of this release, so
//! nothing here shows it. Members under 18 see their own points only, and are never on the
//! leaderboard: 8 West leaves them out.

use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use crate::client::{self, Transport};
use crate::profile::ProfileDraft;
use crate::service::{words, Community, Refused};
use crate::wire::{self, Badge, PointsReason};

/// A word the contract writes in `snake_case`, as the screen uses it; `None` for one this copy
/// doesn't know.
fn word<V: Serialize>(value: &V) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .filter(|w| w != "unknown")
}

fn badges(badges: &[Badge]) -> Vec<String> {
    badges.iter().filter_map(word).collect()
}

/// One change to your points.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PointsChangeView {
    #[ts(type = "number")]
    pub points: i64,
    /// What for, as the contract names it (`contact_accepted`, `thanks`, …).
    pub reason: String,
    #[ts(type = "number")]
    pub at: i64,
}

/// Your points, places, badges, and thanks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PointsView {
    #[ts(type = "number")]
    pub total: i64,
    #[ts(type = "number")]
    pub week: i64,
    /// Your place this week and all time; `null` when you are left out (under 18, appearing
    /// offline, or not in good standing).
    pub place_week: Option<u32>,
    pub place_all: Option<u32>,
    pub badges: Vec<String>,
    /// "Thanked by 12 people": different people, not presses.
    pub thanked_by: u32,
    /// The last 20 changes, newest first.
    pub recent: Vec<PointsChangeView>,
}

/// One place on the leaderboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LeaderView {
    pub place: u32,
    pub member_id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub has_picture: bool,
    pub picture_version: Option<String>,
    pub badges: Vec<String>,
    #[ts(type = "number")]
    pub points: i64,
}

/// The leaderboard: **This week** or **All time**, the top 50, and your own place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LeaderboardView {
    pub all_time: bool,
    /// When this week started (Unix seconds); `null` for all time.
    #[ts(type = "number | null")]
    pub since: Option<i64>,
    pub top: Vec<LeaderView>,
    /// Your place, `null` when you are left out.
    pub my_place: Option<u32>,
    #[ts(type = "number")]
    pub my_points: i64,
}

/// **Getting started** (ADR-169 §5, ADR-172 §4): the three steps part 24C shows. Linking with an
/// organization and inviting a helper come with parts 24D and 24E.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GettingStarted {
    /// **Fill in your profile**: you saved a profile with something of your own in it.
    pub profile: bool,
    /// **Find someone**.
    pub found_someone: bool,
    /// **Send a message**.
    pub sent_a_message: bool,
    /// You closed it.
    pub closed: bool,
}

/// Whether a profile has something of your own in it.
fn filled_in(draft: &ProfileDraft) -> bool {
    let shown = draft.shown;
    (shown.company && !draft.company.trim().is_empty())
        || (shown.business
            && (!draft.business_kinds.is_empty() || !draft.business_line.trim().is_empty()))
        || (shown.region && !draft.region.trim().is_empty())
}

impl<T: Transport> Community<T> {
    /// Your points, places, badges, and thanks. New changes and badges since this PC last looked
    /// are recorded in the Ledger, each once (ADR-169 §8).
    pub async fn points(&self) -> Result<PointsView, Refused> {
        let answer = self
            .as_member(&client::points(), 200)
            .await
            .map_err(|f| Refused(words(&f)))?;
        let points: wire::Points = client::read(answer, 200).map_err(|f| Refused(words(&f)))?;
        self.record_new(&points);
        let mut recent: Vec<PointsChangeView> = points
            .recent
            .iter()
            .filter(|c| c.reason != PointsReason::Unknown)
            .filter_map(|c| {
                Some(PointsChangeView {
                    points: c.points,
                    reason: word(&c.reason)?,
                    at: c.at,
                })
            })
            .collect();
        recent.sort_by_key(|c| std::cmp::Reverse(c.at));
        Ok(PointsView {
            total: points.total,
            week: points.week,
            place_week: points.place_week,
            place_all: points.place_all,
            badges: badges(&points.badges),
            thanked_by: points.thanked_by,
            recent,
        })
    }

    /// Record what is new since this PC last looked: each change to your points, and each badge
    /// earned or lost.
    fn record_new(&self, points: &wire::Points) {
        let seen = self.settings();
        let mut newest = seen.points_seen_at;
        for change in &points.recent {
            if change.at > seen.points_seen_at {
                newest = newest.max(change.at);
                if let Some(reason) = word(&change.reason) {
                    self.record(
                        "community.points_earned",
                        json!({ "points": change.points, "reason": reason }),
                    );
                }
            }
        }
        let now = badges(&points.badges);
        for badge in now.iter().filter(|b| !seen.badges_seen.contains(b)) {
            self.record("community.badge_earned", json!({ "badge": badge }));
        }
        for badge in seen.badges_seen.iter().filter(|b| !now.contains(b)) {
            self.record("community.badge_lost", json!({ "badge": badge }));
        }
        if newest != seen.points_seen_at || now != seen.badges_seen {
            self.change_settings(|s| {
                s.points_seen_at = newest;
                s.badges_seen = now;
            });
        }
    }

    /// The leaderboard: this week (from Monday, Pacific time) or all time.
    pub async fn leaderboard(&self, all_time: bool) -> Result<LeaderboardView, Refused> {
        let answer = self
            .as_member(&client::leaderboard(all_time), 200)
            .await
            .map_err(|f| Refused(words(&f)))?;
        let board: wire::Leaderboard = client::read(answer, 200).map_err(|f| Refused(words(&f)))?;
        Ok(LeaderboardView {
            all_time: board.period == wire::LeaderboardPeriod::All,
            since: board.since,
            top: board
                .top
                .into_iter()
                .take(50)
                .map(|row| LeaderView {
                    place: row.place,
                    member_id: row.member_id,
                    name: row.name,
                    display_name: row.display_name,
                    has_picture: row.has_picture,
                    picture_version: row.picture_version,
                    badges: badges(&row.badges),
                    points: row.points,
                })
                .collect(),
            my_place: board.me.as_ref().and_then(|m| m.place),
            my_points: board.me.map_or(0, |m| m.points),
        })
    }

    /// **Getting started**, as this PC knows it. Nothing is sent.
    pub fn getting_started(&self) -> GettingStarted {
        let s = self.settings();
        GettingStarted {
            profile: s.profile.as_ref().is_some_and(filled_in),
            found_someone: s.found_someone,
            sent_a_message: s.sent_a_message,
            closed: s.getting_started_closed,
        }
    }

    /// Close **Getting started**. Nothing is sent.
    pub fn close_getting_started(&self) {
        self.change_settings(|s| s.getting_started_closed = true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_is_filled_in_with_something_of_your_own() {
        let mut d = ProfileDraft {
            shown: crate::profile::ProfileShown::default(),
            ..ProfileDraft::default()
        };
        assert!(!filled_in(&d));
        d.display_name = "Frank".into();
        assert!(!filled_in(&d), "the account's name is not your own words");
        d.company = "8 West Ventures, LLC".into();
        assert!(filled_in(&d));
        d.shown.company = false;
        assert!(!filled_in(&d), "an unticked part doesn't count");
        d.region = "US-CA".into();
        assert!(filled_in(&d));
    }

    #[test]
    fn words_this_copy_does_not_know_are_left_out() {
        assert_eq!(word(&Badge::GoodNeighbor).as_deref(), Some("good_neighbor"));
        assert_eq!(word(&Badge::Unknown), None);
        assert_eq!(
            word(&PointsReason::Link7Days).as_deref(),
            Some("link_7_days")
        );
    }
}
