//! Spreading a plan's use over its window (Phase 25, item 4.6; ADR-258, spread use across the
//! week and the month).
//!
//! AI companies don't say how many tokens a plan holds. Claude Code, Codex, and Copilot report
//! how much of each window is used and when it starts again (ADR-060); the rest report nothing.
//! So Plenipo paces by **percent of each window over time**: by now, a fair share of the window
//! is how far through it the clock is, day hours (8 AM to 8 PM, Pacific time) counting fully and
//! night hours by the owner's night weight. A window used well past its fair share is **ahead
//! of pace**; well short of it, **behind**.
//!
//! An AI tool that reports nothing is paced against a weekly budget of tokens the owner sets,
//! from Plenipo's own token counts, and is marked "estimated".

use plenipo_runtime::agent::{PlanReport, PlanWindow};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Day hours, Pacific time: from 8 AM to 8 PM.
pub const DAY_HOURS: std::ops::Range<u32> = 8..20;
/// How far from its fair share (in points) a window is ahead or behind.
pub const PACE_MARGIN: u8 = 10;
/// How far ahead (in points) also moves work to a smaller model.
pub const FAR_AHEAD: u8 = 25;
/// How much a night hour counts, in percent of a day hour, to start with.
pub const NIGHT_WEIGHT: u8 = 50;

const HOUR_MS: u64 = 3_600_000;
const WEEK_MINUTES: u64 = 7 * 24 * 60;
/// The longest window paced: a month.
const MAX_WINDOW_MS: u64 = 31 * 24 * HOUR_MS;

/// How a window's use compares with a fair share of it so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Pace {
    Ahead,
    OnPace,
    Behind,
    /// The AI tool didn't say how long the window is, or when it starts again.
    Unknown,
}

/// One plan window and its pace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowPace {
    /// How long the window is, in minutes (300: five hours; 10080: a week).
    #[ts(type = "number | null")]
    pub minutes: Option<u64>,
    /// The models it counts, when only some ("Opus").
    pub models: Option<String>,
    pub used_percent: u8,
    /// How much would be used by now if use were spread fairly over the window.
    pub fair_percent: Option<u8>,
    pub pace: Pace,
    #[ts(type = "number | null")]
    pub resets_at: Option<u64>,
    /// Worked out from Plenipo's own token counts against the owner's weekly budget.
    pub estimated: bool,
}

/// How far through the window from `start` to `end` the clock is at `now`, in percent, day hours
/// counting fully and night hours by `night_weight` percent. `hour_of` gives the hour of day
/// (Pacific time) of a moment.
pub fn fair_share(
    start: u64,
    end: u64,
    now: u64,
    night_weight: u8,
    hour_of: &dyn Fn(u64) -> u32,
) -> Option<u8> {
    if end <= start || end - start > MAX_WINDOW_MS {
        return None;
    }
    let now = now.clamp(start, end);
    let weight = |at: u64| {
        if DAY_HOURS.contains(&hour_of(at)) {
            100
        } else {
            u64::from(night_weight.min(100))
        }
    };
    let (mut total, mut done) = (0u64, 0u64);
    let mut at = start;
    while at < end {
        let next = (at - at % HOUR_MS + HOUR_MS).min(end);
        let w = weight(at);
        total += w * (next - at);
        if now > at {
            done += w * (now.min(next) - at);
        }
        at = next;
    }
    if total == 0 {
        // Night only, weighted at nothing: the clock alone.
        return u8::try_from((now - start) * 100 / (end - start)).ok();
    }
    u8::try_from((done * 100 + total / 2) / total).ok()
}

/// A window's pace at `now`; `None` when it reports no share used, or has started again.
pub fn window_pace(
    w: &PlanWindow,
    now: u64,
    night_weight: u8,
    hour_of: &dyn Fn(u64) -> u32,
    estimated: bool,
) -> Option<WindowPace> {
    let used = w.used_percent?;
    if w.resets_at.is_some_and(|r| r <= now) {
        return None;
    }
    let fair = match (w.minutes, w.resets_at) {
        (Some(minutes), Some(end)) if minutes > 0 => fair_share(
            end.saturating_sub(minutes.saturating_mul(60_000)),
            end,
            now,
            night_weight,
            hour_of,
        ),
        _ => None,
    };
    let pace = match fair {
        Some(f) if used >= f.saturating_add(PACE_MARGIN) => Pace::Ahead,
        Some(f) if used.saturating_add(PACE_MARGIN) <= f => Pace::Behind,
        Some(_) => Pace::OnPace,
        None => Pace::Unknown,
    };
    Some(WindowPace {
        minutes: w.minutes,
        models: w.models.clone(),
        used_percent: used,
        fair_percent: fair,
        pace,
        resets_at: w.resets_at,
        estimated,
    })
}

/// Every window of a plan report with its pace. A plan the AI tool says is limited is full.
pub fn plan_paces(
    plan: &PlanReport,
    now: u64,
    night_weight: u8,
    hour_of: &dyn Fn(u64) -> u32,
) -> Vec<WindowPace> {
    let mut out: Vec<WindowPace> = plan
        .windows
        .iter()
        .filter_map(|w| window_pace(w, now, night_weight, hour_of, false))
        .collect();
    if plan.limited && !out.iter().any(|w| w.used_percent >= 100) {
        out.push(WindowPace {
            minutes: None,
            models: None,
            used_percent: 100,
            fair_percent: None,
            pace: Pace::Unknown,
            resets_at: None,
            estimated: false,
        });
    }
    out
}

/// The window of an AI tool that reports nothing: this week's tokens (from Monday at midnight,
/// Pacific time) against the owner's weekly `budget`.
pub fn estimated_window(tokens: u64, budget: u64, week_start: u64) -> PlanWindow {
    // No budget at all is a budget used up.
    let used = tokens
        .saturating_mul(100)
        .checked_div(budget)
        .map_or(100, |share| u8::try_from(share.min(100)).unwrap_or(100));
    PlanWindow {
        minutes: Some(WEEK_MINUTES),
        used_percent: Some(used),
        resets_at: Some(week_start + WEEK_MINUTES * 60_000),
        models: None,
    }
}

/// How far work on a window steps down (Phase 25, items 4.5 and 4.6): 0 not at all, 1 one
/// effort level lower, 2 also a smaller model. Past the owner's `line` (and halfway from it to
/// the limit) work steps down unless the window is behind pace; well ahead of pace it steps down
/// before the line.
pub fn level(w: &WindowPace, line: u8) -> u8 {
    let behind = w.pace == Pace::Behind;
    let ahead_by = w
        .fair_percent
        .map_or(0, |f| w.used_percent.saturating_sub(f));
    let halfway = line.saturating_add(100u8.saturating_sub(line) / 2);
    if (w.used_percent >= halfway && !behind) || ahead_by >= FAR_AHEAD {
        2
    } else if (w.used_percent >= line && !behind) || ahead_by >= PACE_MARGIN {
        1
    } else {
        0
    }
}

/// A window's name in a sentence: "week", "5-hour window", "week (Opus)", "plan".
pub fn window_name(w: &WindowPace) -> String {
    let name = match w.minutes {
        Some(300) => "5-hour window".to_owned(),
        Some(m) if (10_000..=10_100).contains(&m) => "week".to_owned(),
        Some(m) if m >= 40_000 => "month".to_owned(),
        Some(m) if m % 60 == 0 && m > 0 => format!("{}-hour window", m / 60),
        _ => "plan".to_owned(),
    };
    match &w.models {
        Some(models) => format!("{name} ({models})"),
        None => name,
    }
}

/// Why work on `tool` steps down, in a sentence's first part: "Claude Code's week is 45% used,
/// ahead of pace (27% by now)" or "Claude Code's plan is 92% used (your line is 80%)".
pub fn why(tool: &str, w: &WindowPace, line: u8) -> String {
    let about = if w.estimated { "about " } else { "" };
    let estimated = if w.estimated { " (estimated)" } else { "" };
    match (w.pace, w.fair_percent) {
        (Pace::Ahead, Some(fair)) => format!(
            "{tool}'s {} is {about}{}% used{estimated}, ahead of pace ({fair}% by now)",
            window_name(w),
            w.used_percent
        ),
        _ => format!(
            "{tool}'s {} is {about}{}% used{estimated} (your line is {line}%)",
            window_name(w),
            w.used_percent
        ),
    }
}

/// How much room a set of windows leaves (100 minus the fullest), for sending low-priority work
/// to the plan with the most room; `None` when nothing is known.
pub fn room(windows: &[WindowPace]) -> Option<u8> {
    windows
        .iter()
        .map(|w| w.used_percent)
        .max()
        .map(|used| 100u8.saturating_sub(used))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clock where every hour is a day hour, and one where a day is 8 AM to 8 PM.
    fn always_day(_: u64) -> u32 {
        12
    }

    fn clock(at: u64) -> u32 {
        u32::try_from(at / HOUR_MS % 24).unwrap_or(0)
    }

    fn window(minutes: u64, used: u8, resets_at: u64) -> PlanWindow {
        PlanWindow {
            minutes: Some(minutes),
            used_percent: Some(used),
            resets_at: Some(resets_at),
            models: None,
        }
    }

    #[test]
    fn a_fair_share_is_how_far_through_the_window_the_clock_is() {
        let day = 24 * HOUR_MS;
        // Day hours only: halfway through, half.
        assert_eq!(
            fair_share(0, 10 * HOUR_MS, 5 * HOUR_MS, 50, &always_day),
            Some(50)
        );
        // A whole day, night at half weight: 12 day hours (8 to 20) weigh 1200, 12 night hours
        // 600. At 8 AM, the 8 night hours before weigh 400 of 1800.
        assert_eq!(fair_share(0, day, 8 * HOUR_MS, 50, &clock), Some(22));
        // Night at full weight: the clock alone (a third of the day).
        assert_eq!(fair_share(0, day, 8 * HOUR_MS, 100, &clock), Some(33));
        // Night not counted: nothing is due until 8 AM, all of it by 8 PM.
        assert_eq!(fair_share(0, day, 8 * HOUR_MS, 0, &clock), Some(0));
        assert_eq!(fair_share(0, day, 20 * HOUR_MS, 0, &clock), Some(100));
        // Before the start and after the end, the ends.
        assert_eq!(fair_share(HOUR_MS, day, 0, 50, &clock), Some(0));
        assert_eq!(fair_share(0, day, 2 * day, 50, &clock), Some(100));
        // No window, or longer than a month: not paced.
        assert_eq!(fair_share(5, 5, 5, 50, &clock), None);
        assert_eq!(fair_share(0, 40 * day, 5, 50, &clock), None);
    }

    #[test]
    fn a_window_is_ahead_on_pace_or_behind() {
        let week = 7 * 24 * HOUR_MS;
        let reset = 2 * week;
        // Two days of a week gone, day hours only: 29% is fair.
        let now = reset - week + 2 * 24 * HOUR_MS;
        let pace = |used| {
            window_pace(
                &window(WEEK_MINUTES, used, reset),
                now,
                50,
                &always_day,
                false,
            )
            .unwrap()
            .pace
        };
        assert_eq!(pace(45), Pace::Ahead);
        assert_eq!(pace(30), Pace::OnPace);
        assert_eq!(pace(10), Pace::Behind);
        // Without a length, not paced; a window that started again is left out.
        let unknown = PlanWindow {
            minutes: None,
            ..window(WEEK_MINUTES, 50, reset)
        };
        assert_eq!(
            window_pace(&unknown, now, 50, &always_day, false)
                .unwrap()
                .pace,
            Pace::Unknown
        );
        assert!(window_pace(&window(300, 50, now), now, 50, &always_day, false).is_none());
    }

    #[test]
    fn work_steps_down_by_its_pace_and_the_owners_line() {
        let w = |used, fair: Option<u8>| WindowPace {
            minutes: Some(WEEK_MINUTES),
            models: None,
            used_percent: used,
            fair_percent: fair,
            pace: match fair {
                Some(f) if used >= f + PACE_MARGIN => Pace::Ahead,
                Some(f) if used + PACE_MARGIN <= f => Pace::Behind,
                Some(_) => Pace::OnPace,
                None => Pace::Unknown,
            },
            resets_at: None,
            estimated: false,
        };
        // Not paced: the line alone (4.5).
        assert_eq!(level(&w(70, None), 80), 0);
        assert_eq!(level(&w(85, None), 80), 1);
        assert_eq!(level(&w(92, None), 80), 2);
        // Ahead of pace: before the line.
        assert_eq!(level(&w(40, Some(28)), 80), 1);
        assert_eq!(level(&w(60, Some(30)), 80), 2);
        // Behind pace: the best model, even past the line.
        assert_eq!(level(&w(85, Some(98)), 80), 0);
        // On pace past the line: the line holds.
        assert_eq!(level(&w(85, Some(80)), 80), 1);
    }

    #[test]
    fn the_reason_says_which_window_and_why() {
        let w = WindowPace {
            minutes: Some(WEEK_MINUTES),
            models: Some("Opus".into()),
            used_percent: 45,
            fair_percent: Some(27),
            pace: Pace::Ahead,
            resets_at: None,
            estimated: false,
        };
        assert_eq!(
            why("Claude Code", &w, 80),
            "Claude Code's week (Opus) is 45% used, ahead of pace (27% by now)"
        );
        let plain = WindowPace {
            minutes: None,
            models: None,
            used_percent: 92,
            fair_percent: None,
            pace: Pace::Unknown,
            ..w.clone()
        };
        assert_eq!(
            why("Claude Code", &plain, 80),
            "Claude Code's plan is 92% used (your line is 80%)"
        );
        let estimated = WindowPace {
            models: None,
            estimated: true,
            ..w
        };
        assert_eq!(
            why("Kimi Code", &estimated, 80),
            "Kimi Code's week is about 45% used (estimated), ahead of pace (27% by now)"
        );
    }

    #[test]
    fn an_estimated_week_counts_tokens_against_the_budget() {
        let w = estimated_window(250_000, 1_000_000, 1_000);
        assert_eq!(w.used_percent, Some(25));
        assert_eq!(w.minutes, Some(WEEK_MINUTES));
        assert_eq!(w.resets_at, Some(1_000 + 7 * 24 * HOUR_MS));
        assert_eq!(
            estimated_window(5_000_000, 1_000_000, 0).used_percent,
            Some(100)
        );
    }
}
