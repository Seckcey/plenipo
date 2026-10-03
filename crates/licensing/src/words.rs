//! What a Free copy says when it reaches a limit: plain words (ADR-010), naming what Pro adds,
//! and where to enter a key. Never a raw error.

use crate::entitlements::Limit;

/// Where a key is entered, as the screen names it.
pub const WHERE: &str = "Enter a license key in Settings → License.";

/// The first sentence: what Free has.
pub fn free_has(limit: Limit) -> &'static str {
    match limit {
        Limit::Organizations => "Free has one organization.",
        Limit::Departments => "Free has one department.",
        Limit::Projects => "Free has one project.",
        Limit::WorkersAtOnce => "Free runs 3 workers at a time. This one starts when one finishes.",
        Limit::BusinessDepartment => {
            "Business departments, like Sales on HubSpot, are part of Pro."
        }
        Limit::Connections => "Connections are part of Pro.",
        Limit::AddOnTools => "Add-on tools are part of Pro.",
        Limit::Lessons => "Workers that learn from their work are part of Pro.",
        Limit::PhoneAccess => "Using Plenipo from your phone is part of Pro.",
        Limit::CommunityStart => {
            "Starting a conversation, a link, or an invitation in Community is part of Pro."
        }
    }
}

/// The second sentence: what Pro adds.
pub fn pro_adds(limit: Limit) -> &'static str {
    match limit {
        Limit::Organizations => {
            "Plenipo Pro covers 3 organizations, each in its own window, and Partner plans cover \
             10, 25, or any number."
        }
        Limit::Departments => "Plenipo Pro adds as many departments as you want.",
        Limit::Projects => "Plenipo Pro adds as many projects as you want.",
        Limit::WorkersAtOnce => "Plenipo Pro runs 4 at a time in each organization.",
        Limit::BusinessDepartment => {
            "Plenipo Pro adds the business departments, with their roles already set up."
        }
        Limit::Connections => {
            "Plenipo Pro connects your workers to Microsoft 365, Slack, Google, HubSpot, Stripe, \
             and your website."
        }
        Limit::AddOnTools => "Plenipo Pro lets your workers use the add-on tools you set up.",
        Limit::Lessons => "Plenipo Pro keeps the lessons your workers learn, and uses them.",
        Limit::PhoneAccess => {
            "Plenipo Pro lets you see your work, answer approvals, and give objectives from your \
             phone, while your PC stays in charge."
        }
        Limit::CommunityStart => {
            "Plenipo Pro lets you write first to people in Community, link your organization \
             with another, and invite a helper. Answering someone who wrote to you first is free."
        }
    }
}

/// The whole message.
pub fn message(limit: Limit) -> String {
    format!("{} {} {WHERE}", free_has(limit), pro_adds(limit))
}

/// What a Pro or Partner key says when the PC has as many organizations as it covers (ADR-119).
pub fn plan_covers(most: u32) -> String {
    format!(
        "Your plan covers {most} organization{}. Plenipo Partner plans cover 10, 25, or any \
         number, for companies that run Plenipo for clients. Change your plan in your 8 West \
         account, then enter the new key in Settings → License.",
        if most == 1 { "" } else { "s" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Words the screen never uses (docs/design/vocabulary.md): the owner reads plain words.
    const NEVER: [&str; 12] = [
        "entitlement",
        "tier",
        "sku",
        "quota",
        "license tier",
        "upgrade required",
        "forbidden",
        "unauthorized",
        "denied",
        "error",
        "runtime",
        "coordinator",
    ];

    #[test]
    fn every_blocked_path_says_what_free_has_what_pro_adds_and_where_to_go() {
        for limit in Limit::ALL {
            let m = message(limit);
            assert!(m.contains("Pro"), "{m}");
            assert!(m.ends_with(WHERE), "{m}");
            let lower = m.to_lowercase();
            for word in NEVER {
                assert!(!lower.contains(word), "{limit:?} says {word:?}: {m}");
            }
        }
    }

    #[test]
    fn the_messages_are_as_written() {
        // A snapshot: a change here is a change on screen, checked against the vocabulary.
        let all: Vec<String> = Limit::ALL.iter().map(|l| message(*l)).collect();
        assert_eq!(
            all,
            [
                "Free has one organization. Plenipo Pro covers 3 organizations, each in its own \
                 window, and Partner plans cover 10, 25, or any number. Enter a license key in \
                 Settings → License.",
                "Free has one department. Plenipo Pro adds as many departments as you want. \
                 Enter a license key in Settings → License.",
                "Free has one project. Plenipo Pro adds as many projects as you want. Enter a \
                 license key in Settings → License.",
                "Free runs 3 workers at a time. This one starts when one finishes. Plenipo Pro \
                 runs 4 at a time in each organization. Enter a license key in Settings → \
                 License.",
                "Business departments, like Sales on HubSpot, are part of Pro. Plenipo Pro adds \
                 the business departments, with their roles already set up. Enter a license key \
                 in Settings → License.",
                "Connections are part of Pro. Plenipo Pro connects your workers to Microsoft 365, \
                 Slack, Google, HubSpot, Stripe, and your website. Enter a license key in \
                 Settings → License.",
                "Add-on tools are part of Pro. Plenipo Pro lets your workers use the add-on tools \
                 you set up. Enter a license key in Settings → License.",
                "Workers that learn from their work are part of Pro. Plenipo Pro keeps the \
                 lessons your workers learn, and uses them. Enter a license key in Settings → \
                 License.",
                "Using Plenipo from your phone is part of Pro. Plenipo Pro lets you see your \
                 work, answer approvals, and give objectives from your phone, while your PC \
                 stays in charge. Enter a license key in Settings → License.",
                "Starting a conversation, a link, or an invitation in Community is part of Pro. \
                 Plenipo Pro lets you write first to people in Community, link your \
                 organization with another, and invite a helper. Answering someone who wrote to \
                 you first is free. Enter a license key in Settings → License.",
            ]
        );
    }
}
