//! Side chats (Phase 25, item 3.5; ADR-201): the owner asks a full-time agent a question while
//! it works or waits for its team. The side chat is a new conversation on the agent's AI tool
//! and model, told who it is, what it is doing now, and its recent conversation (from Plenipo's
//! own record of it), with the owner's question. Answer only: no tools and no hand-offs. The
//! agent's real work is never touched.

use plenipo_runtime::agent::AgentSessionDetail;

/// How many of the agent's latest turns the briefing repeats.
const TURNS: usize = 4;
const OBJECTIVE_CHARS: usize = 300;
const ANSWER_CHARS: usize = 800;

fn cut(text: &str, max: usize) -> String {
    let t = text.trim();
    if t.chars().count() > max {
        let kept: String = t.chars().take(max).collect();
        format!("{kept}…")
    } else {
        t.to_owned()
    }
}

/// The side chat's first message: who it is, what it is doing, its recent conversation, and the
/// owner's question.
pub fn brief(
    title: &str,
    role: &str,
    conversation: Option<&AgentSessionDetail>,
    question: &str,
) -> String {
    let mut out = format!(
        "You are {title} ({role}). This is a side chat: your owner is asking you a question while \
         you work. Answer it from what you know. In this side chat you can't use tools or hand \
         work to your team, and nothing said here changes your work.\n"
    );
    let turns = conversation.map(|c| c.turns.as_slice()).unwrap_or_default();
    match turns.iter().rev().find(|t| t.running || t.waiting) {
        Some(t) if t.waiting => out.push_str(&format!(
            "\nStatus now: waiting for your team's replies on: {}\n",
            cut(t.objective.lines().next().unwrap_or(""), OBJECTIVE_CHARS)
        )),
        Some(t) => out.push_str(&format!(
            "\nStatus now: working on: {}\n",
            cut(t.objective.lines().next().unwrap_or(""), OBJECTIVE_CHARS)
        )),
        None => out.push_str("\nStatus now: not working on anything.\n"),
    }
    let recent: Vec<_> = turns.iter().rev().take(TURNS).rev().collect();
    if !recent.is_empty() {
        out.push_str("\nYour recent conversation (oldest first):\n");
        for t in recent {
            out.push_str(&format!(
                "- Objective: {}\n",
                cut(&t.objective, OBJECTIVE_CHARS)
            ));
            match t.result.as_ref().and_then(|r| r.text.as_deref()) {
                Some(answer) if !answer.trim().is_empty() => {
                    out.push_str(&format!("  Your answer: {}\n", cut(answer, ANSWER_CHARS)));
                }
                _ if t.running || t.waiting => out.push_str("  Not finished yet.\n"),
                _ => {}
            }
        }
    }
    out.push_str(&format!("\nYour owner's question:\n{}\n", question.trim()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_brief_says_who_it_is_what_it_is_doing_and_the_question() {
        let text = brief(
            "Website Supervisor",
            "Supervisor",
            None,
            "  How far along is it? ",
        );
        assert!(text.starts_with("You are Website Supervisor (Supervisor). This is a side chat"));
        assert!(text.contains("you can't use tools or hand work to your team"));
        assert!(text.contains("Status now: not working on anything."));
        assert!(text.ends_with("Your owner's question:\nHow far along is it?\n"));
        assert!(!text.contains("recent conversation"));
    }

    #[test]
    fn long_words_are_cut() {
        assert_eq!(cut("abcdef", 3), "abc…");
        assert_eq!(cut(" abc ", 3), "abc");
    }
}
