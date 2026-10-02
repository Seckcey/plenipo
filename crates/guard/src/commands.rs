//! Command lines and the owner's command rules. A worker names a program and a list of
//! arguments — never a shell string — so a rule can be matched word by word.

/// A program to run and its arguments, as the worker asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine {
    pub program: String,
    pub args: Vec<String>,
}

/// File extensions Windows runs without being named.
const RUN_EXTENSIONS: &[&str] = &[".exe", ".cmd", ".bat", ".com"];

/// Whether this system's program and file names ignore upper and lower case. Windows does, and
/// runs `cargo.exe` when told `cargo`. On a Mac or a Linux PC, `Deploy` and `deploy` can be two
/// different files, so a rule that allows names a program exactly there (ADR-150).
pub const NAMES_IGNORE_CASE: bool = cfg!(windows);

/// How a rule is compared with a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compare {
    /// As this system tells programs apart: for rules that allow, which must never cover a
    /// program the owner did not name.
    Exact,
    /// Ignoring upper and lower case and Windows' run extensions on every system: for rules
    /// that block or ask, which must catch a name however it is written.
    Loose,
}

impl Compare {
    fn ignores_case(self) -> bool {
        self == Self::Loose || NAMES_IGNORE_CASE
    }
}

/// How a program is compared: a bare name, or a relative path inside the project written as
/// `./path`. On Windows in lower case and without its run extension (`Cargo.EXE` → `cargo`);
/// on a Mac or a Linux PC exactly as written.
pub fn program_key(program: &str) -> String {
    key(program, Compare::Exact)
}

fn key(program: &str, compare: Compare) -> String {
    let p = program.trim().replace('\\', "/");
    let p = if compare.ignores_case() {
        let lower = p.to_lowercase();
        RUN_EXTENSIONS
            .iter()
            .find_map(|ext| lower.strip_suffix(ext))
            .map_or(lower.clone(), str::to_owned)
    } else {
        p
    };
    let b = p.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    if p.contains('/') && !p.starts_with("./") && !p.starts_with('/') && !drive {
        format!("./{p}")
    } else {
        p
    }
}

impl CommandLine {
    pub fn new(program: impl Into<String>, args: &[&str]) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
        }
    }

    /// The program's bare name (`cargo`), for secrets given to named programs: compared the
    /// way this system tells programs apart (`program_key`).
    pub fn program_name(&self) -> String {
        bare(&program_key(&self.program))
    }

    /// The command line as a person would type it (arguments with spaces are quoted).
    pub fn shown(&self) -> String {
        std::iter::once(&self.program)
            .chain(self.args.iter())
            .map(|w| {
                if w.is_empty() || w.contains(char::is_whitespace) || w.contains('"') {
                    format!("\"{}\"", w.replace('"', "\\\""))
                } else {
                    w.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Program and arguments in lower case, for the sensitive-action check, which catches a
    /// name however it is written: the program without Windows' run extension on every system.
    pub fn words_lower(&self) -> Vec<String> {
        std::iter::once(bare(&key(&self.program, Compare::Loose)))
            .chain(self.args.iter().map(|a| a.to_lowercase()))
            .collect()
    }
}

/// The last part of a program key (`./tools/run` → `run`).
fn bare(key: &str) -> String {
    key.rsplit('/').next().unwrap_or(key).to_owned()
}

/// `text` matches `pattern`, where `*` is any characters and `?` one; ignoring upper and lower
/// case when `ignore_case`.
fn word_match(pattern: &str, text: &str, ignore_case: bool) -> bool {
    let fold = |s: &str| {
        if ignore_case {
            s.to_lowercase()
        } else {
            s.to_owned()
        }
    };
    let p: Vec<char> = fold(pattern).chars().collect();
    let t: Vec<char> = fold(text).chars().collect();
    fn go(p: &[char], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('*') => (0..=t.len()).any(|i| go(&p[1..], &t[i..])),
            Some('?') => !t.is_empty() && go(&p[1..], &t[1..]),
            Some(c) => t.first() == Some(c) && go(&p[1..], &t[1..]),
        }
    }
    go(&p, &t)
}

/// `rule` (e.g. `cargo test *`) allows `cmd`: the program and each word compared the way this
/// system tells them apart. For approved commands, and programs given secrets (ADR-048).
pub fn rule_matches(rule: &str, cmd: &CommandLine) -> bool {
    matches(rule, cmd, Compare::Exact)
}

/// `rule` catches `cmd` for blocking or asking: upper and lower case and Windows' run
/// extensions are ignored on every system, so `rm *` catches `RM` and `rm.exe` anywhere.
pub fn rule_catches(rule: &str, cmd: &CommandLine) -> bool {
    matches(rule, cmd, Compare::Loose)
}

fn matches(rule: &str, cmd: &CommandLine, compare: Compare) -> bool {
    let ignore_case = compare.ignores_case();
    let mut words = rule.split_whitespace();
    let Some(program) = words.next() else {
        return false;
    };
    if !word_match(
        &key(program, compare),
        &key(&cmd.program, compare),
        ignore_case,
    ) {
        return false;
    }
    let words: Vec<&str> = words.collect();
    let (fixed, rest_any) = match words.split_last() {
        Some((&"*", fixed)) => (fixed, true),
        _ => (words.as_slice(), false),
    };
    if cmd.args.len() < fixed.len() || (!rest_any && cmd.args.len() != fixed.len()) {
        return false;
    }
    fixed
        .iter()
        .zip(&cmd.args)
        .all(|(w, arg)| word_match(w, arg, ignore_case))
}

/// The first rule in `rules` that allows `cmd` (`rule_matches`).
pub fn first_match<'a>(rules: &'a [String], cmd: &CommandLine) -> Option<&'a str> {
    rules
        .iter()
        .map(String::as_str)
        .find(|r| rule_matches(r, cmd))
}

/// The first rule in `rules` that catches `cmd` (`rule_catches`): for the blocked and
/// always-ask lists.
pub fn first_catch<'a>(rules: &'a [String], cmd: &CommandLine) -> Option<&'a str> {
    rules
        .iter()
        .map(String::as_str)
        .find(|r| rule_catches(r, cmd))
}

/// A command rule as the owner writes it: a program name (or `./path` inside the project)
/// followed by words, one line, at most 200 characters.
pub fn valid_rule(rule: &str) -> Result<String, String> {
    let words: Vec<&str> = rule.split_whitespace().collect();
    let Some(program) = words.first() else {
        return Err("a command rule cannot be empty".into());
    };
    let joined = words.join(" ");
    if joined.chars().count() > 200 || joined.chars().any(char::is_control) {
        return Err(format!("{joined:?} is not a usable command rule"));
    }
    let path_like = program.contains(['/', '\\']);
    let relative = program.starts_with("./") || program.starts_with(".\\");
    if path_like && !relative {
        return Err(format!(
            "{program:?}: name a program (like cargo), or a program inside the project as \
             ./path — not a full path"
        ));
    }
    if program.starts_with('-') {
        return Err(format!("{program:?} is not a program name"));
    }
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(line: &str) -> CommandLine {
        let mut w = line.split_whitespace();
        let program = w.next().unwrap().to_owned();
        CommandLine {
            program,
            args: w.map(str::to_owned).collect(),
        }
    }

    #[test]
    fn rules_match_word_by_word() {
        assert!(rule_matches("cargo test *", &cmd("cargo test")));
        assert!(rule_matches(
            "cargo test *",
            &cmd("cargo test --workspace --locked")
        ));
        assert!(!rule_matches("cargo test *", &cmd("cargo build")));
        assert!(!rule_matches("cargo test", &cmd("cargo test --release")));
        assert!(rule_matches(
            "git push *",
            &cmd("git push --force origin main")
        ));
        assert!(rule_matches("rm *", &cmd("rm")));
        assert!(rule_matches(
            "python -m pytest *",
            &cmd("python -m pytest -q")
        ));
        assert!(!rule_matches(
            "python -m pytest *",
            &cmd("python -m pip install x")
        ));
        assert!(rule_matches(
            "npm * publish *",
            &cmd("npm --silent publish")
        ));
        assert!(rule_matches("./gradlew test *", &cmd("./gradlew test")));
        assert!(!rule_matches(
            "./gradlew test *",
            &cmd("gradlew/../gradlew test")
        ));
        assert!(rule_matches("docker*", &cmd("docker-compose")));
        assert!(!rule_matches("", &cmd("x")));
        let rules = vec!["cargo build *".to_owned(), "cargo test *".to_owned()];
        assert_eq!(
            first_match(&rules, &cmd("cargo test -q")),
            Some("cargo test *")
        );
        assert_eq!(first_match(&rules, &cmd("cargo run")), None);
    }

    /// ADR-150: a rule that allows names a program the way this system tells programs apart;
    /// a rule that blocks or asks catches it however it is written, on every system.
    #[test]
    fn allowing_is_exact_where_the_system_is_and_blocking_never_is() {
        let windows = cfg!(windows);
        assert_eq!(
            rule_matches("cargo test *", &cmd("Cargo.EXE test")),
            windows
        );
        assert_eq!(rule_matches("npm run *", &cmd("npm.cmd run lint")), windows);
        assert_eq!(rule_matches("./deploy *", &cmd("./Deploy now")), windows);
        assert_eq!(
            rule_matches("./deploy *", &cmd("./deploy.cmd now")),
            windows
        );
        assert_eq!(
            rule_matches("git commit -m *", &cmd("git commit -M x")),
            windows
        );
        assert!(rule_matches("./deploy *", &cmd("./deploy now")));
        let remove = vec!["Remove-Item *".to_owned()];
        assert_eq!(
            first_match(&remove, &cmd("remove-item x")).is_some(),
            windows
        );
        assert_eq!(
            first_catch(&remove, &cmd("remove-item x")),
            Some("Remove-Item *")
        );
        assert!(rule_catches("rm *", &cmd("RM -rf x")));
        assert!(rule_catches("rm *", &cmd("rm.exe -rf x")));
        assert!(rule_catches("./deploy *", &cmd("./Deploy now")));
        assert!(rule_catches("cargo test *", &cmd("Cargo.EXE test")));
        assert!(!rule_catches("rm *", &cmd("rmdir x")));
    }

    #[test]
    fn programs_and_display() {
        if cfg!(windows) {
            assert_eq!(program_key("C:/Tools/Cargo.exe"), "c:/tools/cargo");
            assert_eq!(program_key("scripts\\build.cmd"), "./scripts/build");
            assert_eq!(cmd("npm.cmd test").program_name(), "npm");
        } else {
            assert_eq!(program_key("/opt/Tools/Cargo"), "/opt/Tools/Cargo");
            assert_eq!(program_key("scripts/build.sh"), "./scripts/build.sh");
            assert_eq!(cmd("npm.cmd test").program_name(), "npm.cmd");
        }
        assert_eq!(cmd("NPM.cmd test").words_lower()[0], "npm");
        let c = CommandLine::new("git", &["commit", "-m", "fix the bug"]);
        assert_eq!(c.shown(), "git commit -m \"fix the bug\"");
        assert_eq!(c.words_lower(), ["git", "commit", "-m", "fix the bug"]);
    }

    #[test]
    fn rules_are_validated() {
        assert_eq!(valid_rule("  cargo   test  * ").unwrap(), "cargo test *");
        assert!(valid_rule("./gradlew build *").is_ok());
        assert!(valid_rule("").is_err());
        assert!(valid_rule("/usr/bin/rm *").is_err());
        assert!(valid_rule("C:\\x\\y.exe").is_err());
        assert!(valid_rule("--flag").is_err());
    }
}
