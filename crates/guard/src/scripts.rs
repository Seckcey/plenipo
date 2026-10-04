//! A PowerShell script read as statements, so the programs it names get the owner's command
//! lists — the never-run list, the always-ask list — and the "outside the project folder" check
//! that a command line gets (P-GUARD-4, ADR-214). The same reading is given to the text a
//! shell is handed on a command line (`bash -c "…"`, `cmd /c …`, `powershell -Command …`).
//!
//! This is a net for naive and accidental calls under Light, not a bound on a determined
//! worker: PowerShell can build a program's name at run time in more ways than any reader can
//! follow. The reader is deliberately simple. A statement it can read names a program literally
//! (`rm`, `Remove-Item`, `& "rm"`); a statement it cannot read — text run as code, an encoded
//! command, a program named by a variable or an expression, an alias made on the spot, a job or
//! a remote command, a direct call into .NET's file or process classes — is reported as such, and
//! the engine asks the owner about it. The role's level and the Safety setting stay the main
//! control; a script file is a program like any other (ADR-213).

use crate::commands::{first_catch, CommandLine};

/// One statement of a script, as far as Plenipo can read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    /// The line it starts on, from 1.
    pub line: usize,
    pub kind: StatementKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementKind {
    /// A program (or cmdlet, or alias) named literally, with its arguments.
    Program(CommandLine),
    /// Plenipo cannot tell which program it runs: why, in plain words, to follow "line N of the
    /// script …".
    Opaque(&'static str),
}

/// Windows PowerShell 5.1's built-in aliases for the cmdlets the owner's lists are likely to
/// name (`rm` and `ri` are `Remove-Item`), so a rule naming either spelling catches both.
/// PowerShell 7 (`pwsh`, the fallback where Windows PowerShell is not installed) drops `curl`
/// and `wget` and keeps the rest; a rule still catches them because the written name is checked
/// too.
const ALIASES: &[(&str, &str)] = &[
    ("rm", "remove-item"),
    ("ri", "remove-item"),
    ("del", "remove-item"),
    ("erase", "remove-item"),
    ("rd", "remove-item"),
    ("rmdir", "remove-item"),
    ("mi", "move-item"),
    ("mv", "move-item"),
    ("move", "move-item"),
    ("cpi", "copy-item"),
    ("cp", "copy-item"),
    ("copy", "copy-item"),
    ("sc", "set-content"),
    ("rni", "rename-item"),
    ("ren", "rename-item"),
    ("curl", "invoke-webrequest"),
    ("wget", "invoke-webrequest"),
    ("iwr", "invoke-webrequest"),
    ("irm", "invoke-restmethod"),
    ("start", "start-process"),
    ("saps", "start-process"),
    ("kill", "stop-process"),
    ("spps", "stop-process"),
    ("iex", "invoke-expression"),
    ("icm", "invoke-command"),
    ("sajb", "start-job"),
    ("nal", "new-alias"),
    ("sal", "set-alias"),
    ("ipal", "import-alias"),
    ("ni", "new-item"),
    ("gci", "get-childitem"),
    ("ls", "get-childitem"),
    ("dir", "get-childitem"),
];

/// Words that start a PowerShell statement without naming a program.
const KEYWORDS: &[&str] = &[
    "if",
    "elseif",
    "else",
    "foreach",
    "for",
    "while",
    "do",
    "until",
    "switch",
    "try",
    "catch",
    "finally",
    "function",
    "filter",
    "workflow",
    "param",
    "return",
    "throw",
    "begin",
    "process",
    "end",
    "in",
    "break",
    "continue",
    "trap",
    "class",
    "enum",
    "using",
    "exit",
    "dynamicparam",
    "data",
    "hidden",
    "static",
];

/// Cmdlets that run, or rename, programs in a way the reader cannot follow.
const OPAQUE_PROGRAMS: &[(&str, &str)] = &[
    ("invoke-expression", "runs text as code (Invoke-Expression)"),
    (
        "invoke-command",
        "runs a command block or a remote command (Invoke-Command)",
    ),
    ("start-job", "starts a job (Start-Job)"),
    ("new-alias", "makes a command alias (New-Alias)"),
    ("set-alias", "changes a command alias (Set-Alias)"),
    ("import-alias", "imports command aliases (Import-Alias)"),
    ("add-type", "compiles and loads code (Add-Type)"),
];

const BY_VARIABLE: &str = "runs a program named by a variable or an expression";
const BY_STRING: &str = "runs a program named by a string that is put together";
const ENCODED: &str = "runs an encoded command";
const BACKTICK: &str = "has a backtick inside a program's name";
const DOTNET: &str = "calls .NET's file, process, or script classes directly";
const COMSPEC: &str = "runs the command interpreter named by $env:ComSpec";

/// The cmdlet an alias stands for (lower case), or `None` for anything else.
pub fn aliased(program: &str) -> Option<&'static str> {
    let p = program.to_lowercase();
    ALIASES.iter().find(|(a, _)| *a == p).map(|(_, c)| *c)
}

/// The command as written and, when its program is an alias, as the cmdlet it stands for: a
/// rule naming either catches it.
pub fn spellings(cmd: &CommandLine) -> Vec<CommandLine> {
    let mut out = vec![cmd.clone()];
    if let Some(cmdlet) = aliased(&cmd.program) {
        out.push(CommandLine {
            program: cmdlet.to_owned(),
            args: cmd.args.clone(),
        });
    }
    out
}

/// The first statement one of `rules` catches (the blocked or the always-ask list): the rule,
/// the program as written, and the line.
pub fn first_caught<'a>(
    rules: &'a [String],
    statements: &[Statement],
) -> Option<(&'a str, String, usize)> {
    statements.iter().find_map(|s| match &s.kind {
        StatementKind::Program(cmd) => spellings(cmd)
            .iter()
            .find_map(|c| first_catch(rules, c))
            .map(|rule| (rule, cmd.program.clone(), s.line)),
        StatementKind::Opaque(_) => None,
    })
}

/// The first statement the reader cannot follow: why, and the line.
pub fn first_opaque(statements: &[Statement]) -> Option<(&'static str, usize)> {
    statements.iter().find_map(|s| match s.kind {
        StatementKind::Opaque(why) => Some((why, s.line)),
        StatementKind::Program(_) => None,
    })
}

/// The programs a statement list names, for the "outside the project folder" check.
pub fn programs(statements: &[Statement]) -> impl Iterator<Item = &CommandLine> {
    statements.iter().filter_map(|s| match &s.kind {
        StatementKind::Program(cmd) => Some(cmd),
        StatementKind::Opaque(_) => None,
    })
}

// ---- PowerShell ---------------------------------------------------------------------------

/// The statements of a PowerShell script.
pub fn statements(script: &str) -> Vec<Statement> {
    let mut out = Vec::new();
    for (line, tokens) in split_powershell(&strip_powershell(script)) {
        read_powershell(line, &tokens, &mut out, 0);
    }
    out
}

/// Comments (`# …`, `<# … #>`) and here-strings (`@" … "@`, `@' … '@`) replaced by spaces,
/// line by line, so the line numbers stay. Quoted strings are left as they are.
fn strip_powershell(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut quote: Option<char> = None;
    // Where the current line began: a `#` or `@"` counts as starting a word when only spaces
    // come before it on its line, or the character before it is one that may touch it.
    let mut line_start = 0usize;
    let starts_word = |i: usize, chars: &[char], line_start: usize| {
        i == line_start
            || chars[i - 1].is_whitespace()
            || chars[line_start..i].iter().all(|x| x.is_whitespace())
    };
    // The line start after skipping `chars[from..to]`, which may hold line breaks.
    let after_skip = |from: usize, to: usize, chars: &[char], line_start: usize| {
        chars[from..to]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(line_start, |p| from + p + 1)
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line_start = i + 1;
        }
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                quote = None;
            } else if c == '`' && q == '"' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 1;
            }
            i += 1;
            continue;
        }
        // A block comment: to `#>`.
        if c == '<' && chars.get(i + 1) == Some(&'#') {
            let mut j = i + 2;
            while j < chars.len() && !(chars[j] == '#' && chars.get(j + 1) == Some(&'>')) {
                j += 1;
            }
            let end = (j + 2).min(chars.len());
            blank(&chars[i..end], &mut out);
            line_start = after_skip(i, end, &chars, line_start);
            i = end;
            continue;
        }
        // A here-string: `@"` or `@'` at the end of a line (after a space, `=`, `(`, or `,`),
        // to a line that starts with `"@` or `'@`.
        if c == '@'
            && matches!(chars.get(i + 1), Some('"') | Some('\''))
            && (starts_word(i, &chars, line_start) || matches!(chars[i - 1], '=' | '(' | ','))
            && chars[i + 2..]
                .iter()
                .take_while(|x| **x != '\n')
                .all(|x| x.is_whitespace())
        {
            let q = chars[i + 1];
            let mut j = i + 2;
            loop {
                while j < chars.len() && chars[j] != '\n' {
                    j += 1;
                }
                if j >= chars.len() {
                    break;
                }
                j += 1;
                if chars.get(j) == Some(&q) && chars.get(j + 1) == Some(&'@') {
                    j += 2;
                    break;
                }
            }
            let end = j.min(chars.len());
            blank(&chars[i..end], &mut out);
            line_start = after_skip(i, end, &chars, line_start);
            i = end;
            continue;
        }
        // A line comment: `#` that starts a word.
        if c == '#' && starts_word(i, &chars, line_start) {
            let mut j = i;
            while j < chars.len() && chars[j] != '\n' {
                j += 1;
            }
            blank(&chars[i..j], &mut out);
            i = j;
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
        }
        out.push(c);
        i += 1;
    }
    out
}

/// `chars` as spaces, keeping its line breaks.
fn blank(chars: &[char], out: &mut String) {
    for c in chars {
        out.push(if *c == '\n' { '\n' } else { ' ' });
    }
}

/// The script split into statements (the line each starts on, and its words), at line ends,
/// `;`, `|`, `||`, `&&`, `&`, and braces and parentheses; never inside a quoted string. A `&`
/// (PowerShell's call operator, cmd's separator) starts the next statement as its first word.
fn split_powershell(text: &str) -> Vec<(usize, Vec<String>)> {
    split_words(text, &[';', '|', '{', '}', '(', ')'], true)
}

/// The shared splitter: `separators` end a statement; `&` ends one and, when `call_operator`,
/// starts the next with the word `&`.
fn split_words(text: &str, separators: &[char], call_operator: bool) -> Vec<(usize, Vec<String>)> {
    let mut out: Vec<(usize, Vec<String>)> = Vec::new();
    let mut tokens: Vec<String> = Vec::new();
    let mut token = String::new();
    let mut line = 1usize;
    let mut start = 1usize;
    let mut quote: Option<char> = None;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut end = |tokens: &mut Vec<String>, token: &mut String, start: usize| {
        if !token.is_empty() {
            tokens.push(std::mem::take(token));
        }
        if !tokens.is_empty() {
            out.push((start, std::mem::take(tokens)));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            token.push(c);
            if c == q {
                quote = None;
            } else if c == '`' && q == '"' && i + 1 < chars.len() {
                token.push(chars[i + 1]);
                i += 1;
            } else if c == '\n' {
                line += 1;
            }
            i += 1;
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                token.push(c);
            }
            '\n' => {
                end(&mut tokens, &mut token, start);
                line += 1;
                start = line;
            }
            '&' if chars.get(i + 1) == Some(&'&') => {
                end(&mut tokens, &mut token, start);
                start = line;
                i += 1;
            }
            '|' if chars.get(i + 1) == Some(&'|') => {
                end(&mut tokens, &mut token, start);
                start = line;
                i += 1;
            }
            '&' => {
                end(&mut tokens, &mut token, start);
                start = line;
                if call_operator {
                    tokens.push("&".into());
                }
            }
            c if separators.contains(&c) => {
                end(&mut tokens, &mut token, start);
                start = line;
            }
            c if c.is_whitespace() => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            }
            c => token.push(c),
        }
        i += 1;
    }
    end(&mut tokens, &mut token, start);
    out
}

/// A word that is one quoted string, as its text; `None` for anything else.
fn literal_string(word: &str) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    let (Some(first), Some(last)) = (chars.first(), chars.last()) else {
        return None;
    };
    if chars.len() < 2 || first != last || !matches!(first, '"' | '\'') {
        return None;
    }
    let inner: String = chars[1..chars.len() - 1].iter().collect();
    if inner.contains(*first) || (*first == '"' && inner.contains(['$', '`'])) {
        return None;
    }
    Some(inner)
}

/// Read one PowerShell statement's words into `out`. `depth` bounds the wrappers followed.
fn read_powershell(line: usize, words: &[String], out: &mut Vec<Statement>, depth: u8) {
    let Some(first) = words.first() else {
        return;
    };
    let lower = first.to_lowercase();
    let push = |out: &mut Vec<Statement>, kind: StatementKind| out.push(Statement { line, kind });
    // The call operator: `& program …` or `. program …`. Alone, what it calls was an
    // expression or a block in parentheses or braces, split off as its own statement.
    if first == "&" || first == "." {
        let Some(named) = words.get(1) else {
            push(out, StatementKind::Opaque(BY_VARIABLE));
            return;
        };
        let program = if named.to_lowercase().contains("$env:comspec") {
            push(out, StatementKind::Opaque(COMSPEC));
            return;
        } else if named.starts_with(['$', '(', '[', '@']) {
            push(out, StatementKind::Opaque(BY_VARIABLE));
            return;
        } else if let Some(text) = literal_string(named) {
            text
        } else if named.starts_with(['"', '\'']) {
            push(out, StatementKind::Opaque(BY_STRING));
            return;
        } else {
            named.clone()
        };
        read_program(line, &program, &words[2..], out, depth);
        return;
    }
    // A variable or a type first: an assignment (`$x = rm …`, `$x=rm …`, `$x =rm …`,
    // `[int]$n = …`, `$x += …`) is read from what comes after the `=`; a .NET class that works
    // on files or processes is reported; anything else (a variable alone, a member call) names
    // no program.
    if first.starts_with(['$', '[']) {
        if is_opaque_dotnet(&lower) {
            push(out, StatementKind::Opaque(DOTNET));
        } else if let Some(rest) = assigned(words) {
            read_powershell(line, &rest, out, depth);
        }
        return;
    }
    if KEYWORDS.contains(&lower.as_str()) {
        // `return rm x` is unusual; `if (…)` already split its condition out.
        return;
    }
    // A parameter, a splat or a string on its own, or the rest of an argument list after a
    // here-string (`"@, 1`) names no program.
    if first.starts_with(['-', '@', '"', '\'', ',']) || lower.contains("$env:comspec") {
        if lower.contains("$env:comspec") {
            push(out, StatementKind::Opaque(COMSPEC));
        }
        return;
    }
    // A member call (`.Delete()`), not the call operator.
    if first.starts_with('.') && !first.starts_with("./") && !first.starts_with(".\\") {
        return;
    }
    if first.chars().all(|c| c.is_ascii_digit()) {
        return;
    }
    read_program(line, first, &words[1..], out, depth);
}

/// The words after the `=` of an assignment, written apart or glued to either side (`$x = a`,
/// `$x=a`, `$x =a`, `$x= a`, and `$x += a` and its kin); `None` when the words are not one.
fn assigned(words: &[String]) -> Option<Vec<String>> {
    let first = words.first()?;
    let not_a_comparison = |s: &str| !s.starts_with("==");
    if let Some(i) = first.find('=').filter(|&i| not_a_comparison(&first[i..])) {
        let mut rest = Vec::new();
        if first.len() > i + 1 {
            rest.push(first[i + 1..].to_owned());
        }
        rest.extend(words[1..].iter().cloned());
        return Some(rest);
    }
    let second = words.get(1)?;
    if second.starts_with('=') && not_a_comparison(second) {
        let mut rest = Vec::new();
        if second.len() > 1 {
            rest.push(second[1..].to_owned());
        }
        rest.extend(words[2..].iter().cloned());
        return Some(rest);
    }
    // `+=`, `-=`, `*=`, `/=`, `%=`, `??=`.
    if second.ends_with('=') && second.len() <= 3 && not_a_comparison(second) {
        return Some(words[2..].to_vec());
    }
    None
}

fn is_opaque_dotnet(lower: &str) -> bool {
    lower.starts_with("[system.io.")
        || lower.starts_with("[io.")
        || lower.starts_with("[scriptblock]::create")
        || lower.starts_with("[system.diagnostics.process]")
        || lower.starts_with("[diagnostics.process]")
        || lower.starts_with("[system.management.automation.")
}

/// A program named literally: the statement, and the statements of a wrapper it hands text to.
fn read_program(line: usize, program: &str, args: &[String], out: &mut Vec<Statement>, depth: u8) {
    let push = |out: &mut Vec<Statement>, kind: StatementKind| out.push(Statement { line, kind });
    if program.contains('`') {
        push(out, StatementKind::Opaque(BACKTICK));
        return;
    }
    if program.to_lowercase().contains("$env:comspec") {
        push(out, StatementKind::Opaque(COMSPEC));
        return;
    }
    let lower = program.to_lowercase();
    let cmdlet = aliased(&lower).unwrap_or(lower.as_str());
    if let Some((_, why)) = OPAQUE_PROGRAMS.iter().find(|(p, _)| *p == cmdlet) {
        push(out, StatementKind::Opaque(why));
        return;
    }
    let cmd = CommandLine {
        program: program.to_owned(),
        args: args.iter().map(|a| unquoted(a)).collect(),
    };
    push(out, StatementKind::Program(cmd.clone()));
    if depth == 0 {
        // `Start-Process rm …` and `Start-Process -FilePath "rm" …` name a program too.
        if cmdlet == "start-process" {
            let named = args
                .iter()
                .position(|a| a.eq_ignore_ascii_case("-filepath"))
                .and_then(|i| args.get(i + 1))
                .or_else(|| args.iter().find(|a| !a.starts_with('-')));
            if let Some(named) = named.map(|n| unquoted(n)) {
                if named.starts_with(['$', '(', '[']) || named.contains('$') {
                    push(out, StatementKind::Opaque(BY_VARIABLE));
                } else if !named.is_empty() {
                    read_program(line, &named, &[], out, depth + 1);
                }
            }
        }
        for s in inner_of(&cmd, depth + 1) {
            out.push(Statement { line, ..s });
        }
    }
}

/// `word` without the quotes around it, if any.
fn unquoted(word: &str) -> String {
    literal_string(word).unwrap_or_else(|| word.to_owned())
}

// ---- Shells on a command line -----------------------------------------------------------

/// The statements of the text a shell is handed on a command line: `sh -c "…"` (and bash, zsh,
/// dash, ksh, fish), `cmd /c …` (and `/k`), and `powershell -Command …` (and pwsh, `-c`); an
/// encoded command is reported as such. Nothing for any other program.
pub fn inner_statements(cmd: &CommandLine) -> Vec<Statement> {
    inner_of(cmd, 1)
}

fn inner_of(cmd: &CommandLine, depth: u8) -> Vec<Statement> {
    let mut out = Vec::new();
    if depth > 1 {
        return out;
    }
    let program = cmd.words_lower().into_iter().next().unwrap_or_default();
    let args = &cmd.args;
    match program.as_str() {
        "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish" => {
            if let Some(i) = args.iter().position(|a| a == "-c") {
                if let Some(text) = args.get(i + 1) {
                    // Groups (`( … )`, `{ …; }`) split like PowerShell's.
                    let separators = [';', '|', '(', ')', '{', '}'];
                    for (line, words) in split_words(&unquoted(text), &separators, false) {
                        read_shell(line, &words, &mut out);
                    }
                }
            }
        }
        "cmd" => {
            if let Some(i) = args
                .iter()
                .position(|a| a.eq_ignore_ascii_case("/c") || a.eq_ignore_ascii_case("/k"))
            {
                let text = args[i + 1..].join(" ");
                for (line, words) in split_words(&unquoted(&text), &['|', '(', ')'], false) {
                    read_shell(line, &words, &mut out);
                }
            }
        }
        "powershell" | "pwsh" => {
            let lower: Vec<String> = args.iter().map(|a| a.to_lowercase()).collect();
            if lower.iter().any(|a| {
                a.len() >= 2 && "-encodedcommand".starts_with(a.as_str()) && a != "-e"
                    || a == "-e"
                    || a == "-ec"
            }) {
                out.push(Statement {
                    line: 1,
                    kind: StatementKind::Opaque(ENCODED),
                });
            } else if let Some(i) = lower
                .iter()
                .position(|a| a.len() >= 2 && "-command".starts_with(a.as_str()))
            {
                let text = args[i + 1..].join(" ");
                let text = unquoted(&text);
                if text.starts_with('$') || text.starts_with('{') {
                    out.push(Statement {
                        line: 1,
                        kind: StatementKind::Opaque(BY_VARIABLE),
                    });
                }
                for (line, words) in split_powershell(&strip_powershell(&text)) {
                    read_powershell(line, &words, &mut out, depth);
                }
            }
        }
        _ => {}
    }
    out
}

/// Words that only change how the program after them runs (`exec rm x`, `nice -n 10 rm x`).
/// `sudo` is not one: the sensitive check catches it; `xargs` is a known gap (ADR-214).
const SHELL_PREFIXES: &[&str] = &[
    "exec", "command", "builtin", "env", "nohup", "time", "nice", "ionice",
];

/// One statement of a POSIX shell or cmd: `VAR=x program args`, `exec program args`, and
/// `program args`. A prefix is a statement of its own as well as the program after it, so a
/// rule naming either catches it (`nohup *` and `rm *` both catch `nohup rm x`), as they would
/// on a command line. `command -v name` and `command -V name` only look a name up: they name
/// `command` and nothing else.
fn read_shell(line: usize, words: &[String], out: &mut Vec<Statement>) {
    let mut words = words;
    loop {
        let Some(first) = words.first() else {
            return;
        };
        let lower = first.to_lowercase();
        if first.contains('=') && !first.starts_with(['$', '"', '\'', '-']) {
            // `VAR=x` before the program.
            words = &words[1..];
        } else if SHELL_PREFIXES.contains(&lower.as_str()) {
            out.push(Statement {
                line,
                kind: StatementKind::Program(CommandLine {
                    program: unquoted(first),
                    args: words[1..].iter().map(|a| unquoted(a)).collect(),
                }),
            });
            words = &words[1..];
            // The prefix's own options (`env -i`, `nice -n 10`, `ionice -c 3`, `command -v`).
            let takes_a_number = matches!(lower.as_str(), "nice" | "ionice");
            let mut looks_up = false;
            while let Some(option) = words.first().filter(|w| {
                w.starts_with('-') || (takes_a_number && w.chars().all(|c| c.is_ascii_digit()))
            }) {
                // `command -v`, `-V`, and the two run together with `-p` (`-pv`).
                looks_up |= lower == "command"
                    && option.starts_with('-')
                    && !option.starts_with("--")
                    && option[1..].contains(['v', 'V']);
                words = &words[1..];
            }
            if looks_up {
                return;
            }
        } else {
            break;
        }
    }
    let Some(first) = words.first() else {
        return;
    };
    if first.starts_with('$') || first.starts_with('`') || first.starts_with("eval") {
        out.push(Statement {
            line,
            kind: StatementKind::Opaque(BY_VARIABLE),
        });
        return;
    }
    let program = unquoted(first);
    if program.starts_with('$') || program.starts_with('%') {
        out.push(Statement {
            line,
            kind: StatementKind::Opaque(BY_VARIABLE),
        });
        return;
    }
    out.push(Statement {
        line,
        kind: StatementKind::Program(CommandLine {
            program,
            args: words[1..].iter().map(|a| unquoted(a)).collect(),
        }),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn programs_of(script: &str) -> Vec<String> {
        statements(script)
            .iter()
            .map(|s| match &s.kind {
                StatementKind::Program(c) => format!("{}:{}", s.line, c.shown()),
                StatementKind::Opaque(why) => format!("{}:?{why}", s.line),
            })
            .collect()
    }

    #[test]
    fn statements_of_a_script() {
        let script = "# tidy up\nGet-ChildItem . | Where-Object { $_.Name -like '*.tmp' } | rm -Force\n\
                      $out = Join-Path $root 'build'\nif (Test-Path $out) { Remove-Item $out -Recurse }\n\
                      Write-Host \"rm -rf is not run; it is text\"\n& 'del' old.log; del new.log\n\
                      <# rm nothing\n here #>\n@\"\nrm in a here-string\n\"@\nforeach ($f in $files) {\n  ri $f\n}";
        assert_eq!(
            programs_of(script),
            [
                "2:Get-ChildItem .",
                "2:Where-Object",
                "2:rm -Force",
                "3:Join-Path $root build",
                "4:Test-Path $out",
                "4:Remove-Item $out -Recurse",
                "5:Write-Host \"rm -rf is not run; it is text\"",
                "6:del old.log",
                "6:del new.log",
                "13:ri $f",
            ]
        );
        // A line comment leaves the rest of its line; a word is not a comment.
        assert_eq!(
            programs_of("Get-Date # now\n$x=\"#1\"; Get-Item '#tag'"),
            ["1:Get-Date", "2:Get-Item #tag"]
        );
    }

    /// A here-string is one string wherever its opening may touch: after `=`, `(`, or `,` as
    /// well as after a space; a comment starts a word at the start of a line or after a space.
    #[test]
    fn here_strings_and_comments_are_set_aside_as_units() {
        assert_eq!(
            programs_of("$x=@\"\nrm in a string\n\"@\nGet-Date"),
            ["4:Get-Date"]
        );
        assert_eq!(
            programs_of("Write-Host (@'\nrm -rf x\n'@)\nGet-Date"),
            ["1:Write-Host", "4:Get-Date"]
        );
        assert_eq!(
            programs_of("Invoke-Thing -Body @\"\nrm\n\"@, 1\nGet-Date"),
            ["1:Invoke-Thing -Body", "4:Get-Date"]
        );
        assert_eq!(
            programs_of(
                "  # a comment at the start of its line\nGet-Date#not a comment\nrm x # tidy"
            ),
            ["2:Get-Date#not a comment", "3:rm x"]
        );
    }

    #[test]
    fn statements_the_reader_cannot_follow_are_said_so() {
        for (script, why) in [
            ("iex $code", "runs text as code (Invoke-Expression)"),
            (
                "Invoke-Expression (Get-Content x)",
                "runs text as code (Invoke-Expression)",
            ),
            ("& $tool --version", BY_VARIABLE),
            ("& (\"r\" + \"m\") x", BY_VARIABLE),
            ("& \"r$m\" x", BY_STRING),
            ("& ('r'+'m') x", BY_VARIABLE),
            ("r`m -rf x", BACKTICK),
            ("[System.IO.File]::Delete('x')", DOTNET),
            ("[IO.Directory]::Delete('x', $true)", DOTNET),
            ("[scriptblock]::Create($s).Invoke()", DOTNET),
            (
                "Set-Alias x Remove-Item; x y",
                "changes a command alias (Set-Alias)",
            ),
            ("nal x rm", "makes a command alias (New-Alias)"),
            (
                "icm { rm x }",
                "runs a command block or a remote command (Invoke-Command)",
            ),
            ("sajb { rm x }", "starts a job (Start-Job)"),
            (
                "Add-Type -TypeDefinition $src",
                "compiles and loads code (Add-Type)",
            ),
            ("& $env:ComSpec /c dir", COMSPEC),
            ("powershell -enc QQBC", ENCODED),
            ("pwsh -EncodedCommand QQBC", ENCODED),
            (
                "Start-Process powershell -ArgumentList '-e QQBC'",
                "runs a program as administrator",
            ),
        ] {
            let found = first_opaque(&statements(script));
            if why == "runs a program as administrator" {
                // Not opaque: the sensitive check catches this one by its words.
                assert_eq!(found, None, "{script}");
                continue;
            }
            assert_eq!(found.map(|(w, _)| w), Some(why), "{script}");
        }
        // Plain scripts are read whole.
        assert_eq!(
            first_opaque(&statements("Get-ChildItem | Measure-Object")),
            None
        );
        assert_eq!(
            first_opaque(&statements("$x = 1; Write-Host $x; cargo build")),
            None
        );
    }

    /// An assignment hides nothing, however its `=` is written; `Start-Process` names its
    /// program quoted, by `-FilePath`, or bare, and says so when the name is a variable.
    #[test]
    fn assignments_and_start_process_name_their_programs() {
        let blocked = vec!["Remove-Item *".to_owned(), "curl *".to_owned()];
        for script in [
            "$null=rm x",
            "$x =rm x",
            "$x= rm x",
            "$x = rm x",
            "[string]$s=rm x",
            "$list += rm x",
            "$out = (rm x)",
        ] {
            assert_eq!(
                first_caught(&blocked, &statements(script)).map(|f| f.1),
                Some("rm".to_owned()),
                "{script}"
            );
        }
        for script in [
            "Start-Process -FilePath \"rm\" -ArgumentList '-rf x'",
            "Start-Process -FilePath 'curl' -ArgumentList https://x",
            "Start-Process 'curl' https://x",
            "saps -FilePath rm",
            "Start-Process -NoNewWindow -FilePath rm",
        ] {
            assert!(
                first_caught(&blocked, &statements(script)).is_some(),
                "{script}"
            );
        }
        assert_eq!(
            first_opaque(&statements("Start-Process -FilePath $tool")).map(|w| w.0),
            Some(BY_VARIABLE)
        );
        assert_eq!(
            first_opaque(&statements("Start-Process \"$env:ProgramFiles\\x.exe\"")).map(|w| w.0),
            Some(BY_VARIABLE)
        );
        // A variable alone, a comparison, or a member call names no program.
        assert!(statements("$x\n$x -eq 1\n$x == 1\n$f.Delete()").is_empty());
    }

    #[test]
    fn a_rule_catches_a_program_in_a_script_by_either_spelling() {
        let blocked = vec!["Remove-Item *".to_owned(), "curl *".to_owned()];
        let found = first_caught(&blocked, &statements("Get-Date\nri .\\x -Recurse"));
        assert_eq!(found, Some(("Remove-Item *", "ri".to_owned(), 2)));
        let found = first_caught(&blocked, &statements("& 'rm' x"));
        assert_eq!(found, Some(("Remove-Item *", "rm".to_owned(), 1)));
        let found = first_caught(&blocked, &statements("Invoke-WebRequest https://x"));
        assert_eq!(found, None, "the rule names the alias only");
        let found = first_caught(&blocked, &statements("curl.exe https://x"));
        assert_eq!(found, Some(("curl *", "curl.exe".to_owned(), 1)));
        // Start-Process names its program too.
        let found = first_caught(&blocked, &statements("Start-Process rm -ArgumentList x"));
        assert_eq!(found, Some(("Remove-Item *", "rm".to_owned(), 1)));
        // A program's name inside a string that is only shown is not a program.
        assert_eq!(
            first_caught(&blocked, &statements("Write-Host 'rm -rf x'")),
            None
        );
    }

    /// A shell's groups and run-this-way prefixes do not hide the program.
    #[test]
    fn shell_groups_and_prefixes_do_not_hide_the_program() {
        let blocked = vec!["rm *".to_owned()];
        for text in [
            "(rm -rf y)",
            "{ rm x; }",
            "cd x && (rm y)",
            "exec rm x",
            "command rm x",
            "builtin echo; rm x",
            "env FOO=1 nohup rm x",
            "nice -n 10 rm x",
            "ionice -c 3 nice -n 5 rm x",
            "time rm x",
        ] {
            let found = first_caught(
                &blocked,
                &inner_statements(&CommandLine::new("bash", &["-c", text])),
            );
            assert_eq!(found.map(|f| f.1), Some("rm".to_owned()), "{text}");
        }
        let found = first_caught(
            &blocked,
            &inner_statements(&CommandLine::new("cmd", &["/c", "(rm x)"])),
        );
        assert_eq!(found.map(|f| f.1), Some("rm".to_owned()));
        // `sudo` stays a program of its own (the sensitive check catches it).
        let found = first_caught(
            &blocked,
            &inner_statements(&CommandLine::new("sh", &["-c", "sudo rm x"])),
        );
        assert_eq!(found, None);
    }

    /// Looking a program up names only the lookup: an owner who blocks `rm` can still ask
    /// whether it is there. `command -p` runs it, so that is still caught.
    #[test]
    fn looking_a_program_up_does_not_name_it() {
        let blocked = vec!["rm *".to_owned()];
        let bash = |text: &str| inner_statements(&CommandLine::new("bash", &["-c", text]));
        for text in [
            "command -v rm",
            "command -V rm",
            "command -pv rm",
            "command -v rm >/dev/null || echo missing",
            "nohup command -v rm",
            "type rm",
            "type -P rm",
            "which rm",
        ] {
            assert_eq!(first_caught(&blocked, &bash(text)), None, "{text}");
        }
        let names: Vec<String> = bash("command -v rm")
            .iter()
            .map(|s| match &s.kind {
                StatementKind::Program(c) => c.shown(),
                StatementKind::Opaque(why) => format!("?{why}"),
            })
            .collect();
        assert_eq!(names, ["command -v rm"], "the lookup is still a statement");
        for text in ["command -p rm x", "command rm x"] {
            assert_eq!(
                first_caught(&blocked, &bash(text)).map(|f| f.1),
                Some("rm".to_owned()),
                "{text}"
            );
        }
        // PowerShell's lookup names Get-Command only.
        let found = first_caught(
            &blocked,
            &statements("Get-Command rm\nGet-Command -Name rm"),
        );
        assert_eq!(found, None);
        let found = first_caught(
            &blocked,
            &inner_statements(&CommandLine::new("pwsh", &["-c", "Get-Command rm"])),
        );
        assert_eq!(found, None);
        // Running what it found is not a lookup: the reader cannot follow it, so it asks.
        assert_eq!(
            first_opaque(&statements("& (Get-Command rm) x")).map(|w| w.0),
            Some(BY_VARIABLE)
        );
    }

    /// A prefix is a statement of its own as well as the program after it, so an owner's rule
    /// on either catches it, as on a command line.
    #[test]
    fn a_rule_on_a_prefix_catches_it_in_shell_text() {
        let bash = |text: &str| inner_statements(&CommandLine::new("bash", &["-c", text]));
        for (rule, text) in [
            ("nohup *", "nohup rm x"),
            ("env *", "env FOO=1 rm x"),
            ("env *", "FOO=1 env -i make"),
            ("time *", "time make test"),
            ("exec *", "exec make"),
            ("nice *", "nice -n 10 make"),
            ("ionice *", "ionice -c 3 nice -n 5 make"),
            ("nice *", "ionice -c 3 nice -n 5 make"),
            ("builtin *", "builtin echo hi"),
            ("command *", "command -v rm"),
        ] {
            let blocked = vec![rule.to_owned()];
            let found = first_caught(&blocked, &bash(text)).map(|f| f.1);
            let named = rule.split(' ').next().unwrap();
            assert_eq!(found.as_deref(), Some(named), "{rule} on {text}");
        }
        // Every word in order: each prefix with what follows it, then the program.
        let names: Vec<String> = bash("env FOO=1 nohup rm x")
            .iter()
            .map(|s| match &s.kind {
                StatementKind::Program(c) => c.shown(),
                StatementKind::Opaque(why) => format!("?{why}"),
            })
            .collect();
        assert_eq!(names, ["env FOO=1 nohup rm x", "nohup rm x", "rm x"]);
        // A rule on the program still catches it behind any prefix.
        let rm = ["rm *".to_owned()];
        let found = first_caught(&rm, &bash("env FOO=1 nohup rm x"));
        assert_eq!(found.map(|f| f.1), Some("rm".to_owned()));
        // No prefix rule: nothing new is caught.
        assert_eq!(
            first_caught(&["make *".to_owned()], &bash("nohup ls")),
            None
        );
    }

    #[test]
    fn a_shell_on_a_command_line_hands_over_its_text() {
        let inner =
            |program: &str, args: &[&str]| inner_statements(&CommandLine::new(program, args));
        let blocked = vec!["rm *".to_owned(), "curl *".to_owned()];
        let found = first_caught(&blocked, &inner("bash", &["-c", "cd x && rm -rf y"]));
        assert_eq!(found, Some(("rm *", "rm".to_owned(), 1)));
        let found = first_caught(
            &blocked,
            &inner("cmd", &["/c", "dir", "&", "curl", "https://x"]),
        );
        assert_eq!(found, Some(("curl *", "curl".to_owned(), 1)));
        let found = first_caught(&blocked, &inner("cmd", &["/C", "dir & curl https://x"]));
        assert_eq!(found, Some(("curl *", "curl".to_owned(), 1)));
        let found = first_caught(&blocked, &inner("powershell", &["-Command", "rm x"]));
        assert_eq!(found, Some(("rm *", "rm".to_owned(), 1)));
        let found = first_caught(&blocked, &inner("pwsh", &["-c", "Get-Date; rm x"]));
        assert_eq!(found, Some(("rm *", "rm".to_owned(), 1)));
        assert_eq!(first_caught(&blocked, &inner("sh", &["-c", "ls"])), None);
        assert_eq!(
            first_opaque(&inner("sh", &["-c", "$CMD x"])).map(|w| w.0),
            Some(BY_VARIABLE)
        );
        assert_eq!(
            first_opaque(&inner("pwsh", &["-e", "QQBC"])).map(|w| w.0),
            Some(ENCODED)
        );
        assert_eq!(
            first_opaque(&inner("powershell", &["-EncodedCommand", "QQBC"])).map(|w| w.0),
            Some(ENCODED)
        );
        assert!(inner("cargo", &["test", "--workspace"]).is_empty());
        assert!(inner("python", &["-c", "import os; os.system('rm x')"]).is_empty());
    }
}
