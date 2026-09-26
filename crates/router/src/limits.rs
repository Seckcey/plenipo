//! Usage/capacity state, derived from the Ledger (ADR-011): an AI tool whose latest turn hit a
//! usage limit gets no new work until the reset time it reported, or for an hour when it did
//! not report one — unless a later turn on it completed, or the owner asked to try it again.
//! Nothing is kept in memory, so the state is the same after a restart.

use std::collections::{BTreeMap, HashMap};

use plenipo_ledger::TurnOutcomeRecord;

use crate::dto::UsageLimit;

/// How long an AI tool rests after a usage limit without a reported reset time.
pub const HOLD_MS: u64 = 60 * 60 * 1000;
/// How far back turn results are read. Longer than any limit an AI tool reports (weekly).
pub const LOOKBACK_MS: u64 = 8 * 24 * 60 * 60 * 1000;

/// Active usage limits by runtime ID. `outcomes` are newest first; `cleared` holds when the
/// owner last asked to try each runtime again.
pub fn active(
    outcomes: &[TurnOutcomeRecord],
    cleared: &BTreeMap<String, u64>,
    now: u64,
) -> HashMap<String, UsageLimit> {
    let mut latest: HashMap<&str, &TurnOutcomeRecord> = HashMap::new();
    for o in outcomes {
        latest.entry(o.runtime.as_str()).or_insert(o);
    }
    latest
        .into_iter()
        .filter(|(runtime, o)| {
            o.outcome == "usageLimited" && cleared.get(*runtime).is_none_or(|c| o.at > *c)
        })
        .filter_map(|(runtime, o)| {
            let detail = o
                .error
                .clone()
                .or_else(|| o.summary.clone())
                .unwrap_or_default();
            let resets_at = reset_time(&detail, o.at);
            let until = resets_at.unwrap_or(o.at + HOLD_MS);
            (now < until).then(|| {
                (
                    runtime.to_owned(),
                    UsageLimit {
                        model: o.model.clone(),
                        since: o.at,
                        resets_at,
                        until,
                        detail: first_line(&detail),
                    },
                )
            })
        })
        .collect()
}

/// A reset time reported as `…|<Unix seconds or ms>` (the form Claude Code uses), when it is
/// plausible: after the limit and within 30 days of it.
pub fn reset_time(text: &str, at: u64) -> Option<u64> {
    text.split('|').skip(1).find_map(|part| {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        let n: u64 = digits.parse().ok()?;
        let ms = if n < 100_000_000_000 {
            n.checked_mul(1000)?
        } else {
            n
        };
        (ms > at && ms <= at + 30 * 24 * 60 * 60 * 1000).then_some(ms)
    })
}

fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    let mut out: String = line.chars().take(200).collect();
    if line.chars().count() > 200 {
        out.push('…');
    }
    out
}

/// "about 3 hours", "25 minutes", "a minute".
pub fn duration_words(ms: u64) -> String {
    let minutes = ms.div_ceil(60_000);
    match minutes {
        0 | 1 => "a minute".into(),
        2..=59 => format!("{minutes} minutes"),
        60..=89 => "about an hour".into(),
        _ if minutes < 48 * 60 => format!("about {} hours", (minutes + 30) / 60),
        _ => format!("about {} days", (minutes + 12 * 60) / (24 * 60)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(runtime: &str, outcome: &str, at: u64, error: Option<&str>) -> TurnOutcomeRecord {
        TurnOutcomeRecord {
            runtime: runtime.into(),
            model: Some("m".into()),
            outcome: outcome.into(),
            error: error.map(str::to_owned),
            summary: Some("reported a usage limit".into()),
            at,
        }
    }

    const T: u64 = 1_760_000_000_000;

    #[test]
    fn reset_times_are_read_when_plausible() {
        assert_eq!(
            reset_time("Claude AI usage limit reached|1760003600", T),
            Some(T + 3_600_000)
        );
        assert_eq!(reset_time("limit|1760003600000", T), Some(T + 3_600_000));
        // Before the limit, far in the future, or not a number: ignored.
        assert_eq!(reset_time("limit|1759990000", T), None);
        assert_eq!(reset_time("limit|1999999999", T), None);
        assert_eq!(reset_time("You've hit your limit · resets 3pm", T), None);
        assert_eq!(reset_time("a|b|1760003600", T), Some(T + 3_600_000));
    }

    #[test]
    fn the_latest_outcome_per_tool_decides() {
        let cleared = BTreeMap::new();
        // Newest first: a limit after a success is active until the reported reset.
        let outcomes = [
            outcome(
                "a",
                "usageLimited",
                T,
                Some("usage limit reached|1760007200"),
            ),
            outcome("a", "completed", T - 10, None),
            outcome("b", "completed", T, None),
            outcome("b", "usageLimited", T - 10, None),
            outcome("c", "usageLimited", T - HOLD_MS - 1, None),
            outcome("d", "usageLimited", T - 1000, None),
        ];
        let limits = active(&outcomes, &cleared, T + 1);
        let a = &limits["a"];
        assert_eq!(
            (a.since, a.resets_at, a.until),
            (T, Some(T + 7_200_000), T + 7_200_000)
        );
        assert_eq!(a.detail, "usage limit reached|1760007200");
        // b completed after its limit; c's hour has passed; d rests an hour without a reset.
        assert!(!limits.contains_key("b") && !limits.contains_key("c"));
        assert_eq!(limits["d"].until, T - 1000 + HOLD_MS);
        assert_eq!(limits["d"].detail, "reported a usage limit");
        // After the reset time, a is free again.
        assert!(!active(&outcomes, &cleared, T + 7_200_000).contains_key("a"));
        // The owner's "try again" clears limits recorded before it.
        let cleared = BTreeMap::from([("a".to_owned(), T + 5), ("d".to_owned(), T - 2000)]);
        let limits = active(&outcomes, &cleared, T + 10);
        assert!(!limits.contains_key("a"));
        assert!(limits.contains_key("d"));
    }

    #[test]
    fn durations_read_plainly() {
        assert_eq!(duration_words(30_000), "a minute");
        assert_eq!(duration_words(25 * 60_000), "25 minutes");
        assert_eq!(duration_words(70 * 60_000), "about an hour");
        assert_eq!(duration_words(170 * 60_000), "about 3 hours");
        assert_eq!(duration_words(5 * 24 * 3_600_000), "about 5 days");
    }
}
