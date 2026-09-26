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

/// How a program is compared: a bare name in lower case without its Windows extension
/// (`Cargo.EXE` → `cargo`), or a relative path inside the project written as `./path`.
pub fn program_key(program: &str) -> String {
    let p = program.trim().replace('\\', "/").to_lowercase();
    let p = RUN_EXTENSIONS
        .iter()
        .find_map(|ext| p.strip_suffix(ext))
        .map_or(p.clone(), str::to_owned);
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

    /// The program's bare name (`cargo`), for secrets given to named programs.
    pub fn program_name(&self) -> String {
        let key = program_key(&self.program);
        key.rsplit('/').next().unwrap_or(&key).to_owned()
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

    /// Program and arguments in lower case, for the sensitive-action check.
    pub fn words_lower(&self) -> Vec<String> {
        std::iter::once(self.program_name())
            .chain(self.args.iter().map(|a| a.to_lowercase()))
            .collect()
    }
}

/// `text` matches `pattern`, where `*` is any characters and `?` one; ASCII case-insensitive.
fn word_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
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

/// `rule` (e.g. `cargo test *`) matches `cmd`.
pub fn rule_matches(rule: &str, cmd: &CommandLine) -> bool {
    let mut words = rule.split_whitespace();
    let Some(program) = words.next() else {
        return false;
    };
    if !word_match(&program_key(program), &program_key(&cmd.program)) {
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
        .all(|(w, arg)| word_match(w, arg))
}

/// The first rule in `rules` that matches `cmd`.
pub fn first_match<'a>(rules: &'a [String], cmd: &CommandLine) -> Option<&'a str> {
    rules
        .iter()
        .map(String::as_str)
        .find(|r| rule_matches(r, cmd))
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
        assert!(rule_matches("cargo test *", &cmd("Cargo.EXE test")));
        assert!(!rule_matches("cargo test *", &cmd("cargo build")));
        assert!(!rule_matches("cargo test", &cmd("cargo test --release")));
        assert!(rule_matches("npm run *", &cmd("npm.cmd run lint")));
        assert!(rule_matches(
            "git push *",
            &cmd("git push --force origin main")
        ));
        assert!(rule_matches("rm *", &cmd("rm")));
        assert!(rule_matches("Remove-Item *", &cmd("remove-item x")));
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

    #[test]
    fn programs_and_display() {
        assert_eq!(program_key("C:/Tools/Cargo.exe"), "c:/tools/cargo");
        assert_eq!(program_key("scripts\\build.cmd"), "./scripts/build");
        assert_eq!(cmd("npm.cmd test").program_name(), "npm");
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
