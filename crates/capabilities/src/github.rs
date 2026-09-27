//! GitHub (Phase 8, ADR-016): the project's repository on GitHub, which is the only one Plenipo's
//! GitHub tools act on, and the pull request link `gh` prints.

/// `owner/name` of a GitHub repository address (`https://github.com/owner/name`, with or without
/// `.git`, or `git@github.com:owner/name.git`); `None` for anything else.
pub fn repo_of(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("https://www.github.com/"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("git@github.com:"))?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let ok = |s: &str| {
        (1..=100).contains(&s.len())
            && !s.starts_with(['.', '-'])
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (ok(owner) && ok(name)).then(|| format!("{owner}/{name}"))
}

/// The first pull request link in `text` (`https://github.com/<owner>/<name>/pull/<n>`), and its
/// number.
pub fn pull_request_link(text: &str) -> Option<(String, u64)> {
    text.split_whitespace().find_map(|word| {
        let word = word.trim_matches(|c: char| matches!(c, '(' | ')' | '<' | '>' | ',' | '.'));
        let rest = word.strip_prefix("https://github.com/")?;
        let mut parts = rest.split('/');
        let (_, _, pull, number) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
        if pull != "pull" || parts.next().is_some() {
            return None;
        }
        let n = number.parse().ok()?;
        Some((word.to_owned(), n))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_addresses_are_recognized_and_nothing_else() {
        for url in [
            "https://github.com/Seckcey/plenipo",
            "https://github.com/Seckcey/plenipo.git",
            "https://github.com/Seckcey/plenipo/",
            "git@github.com:Seckcey/plenipo.git",
            "ssh://git@github.com/Seckcey/plenipo",
        ] {
            assert_eq!(repo_of(url).as_deref(), Some("Seckcey/plenipo"), "{url}");
        }
        for url in [
            "https://gitlab.com/a/b",
            "https://github.com/a",
            "https://github.com/a/b/tree/main",
            "https://github.com/../b",
            "https://github.com/a/-b",
            "file:///repo",
            "",
        ] {
            assert_eq!(repo_of(url), None, "{url}");
        }
    }

    #[test]
    fn a_pull_request_link_is_found_in_gh_output() {
        assert_eq!(
            pull_request_link("Creating draft pull request\nhttps://github.com/a/b/pull/42\n"),
            Some(("https://github.com/a/b/pull/42".into(), 42))
        );
        assert_eq!(
            pull_request_link("see https://github.com/a/b/issues/3"),
            None
        );
        assert_eq!(pull_request_link("no link"), None);
    }
}
