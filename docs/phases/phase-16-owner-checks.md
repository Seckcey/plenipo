# Phase 16 Wave 1 — checks on your Windows PC

**What this is.** Four short checks, run on your own PC with your own sign-ins. They tell me what
your subscriptions really allow, so nothing in Plenipo's model lists is a guess. Nothing here
installs or changes Plenipo.

**Do Parts A, B, and C.** Do Part D only if your paid Ollama plan is active. Then send me the
results (the end of this page says how), and I build from them. **For Google's Gemini CLI, I stop
and wait for your Part C results before writing any of its code** (ADR-014, adding AI tools: its
"step 0" runs on your PC first).

- **Time:** about 30 minutes, most of it waiting.
- **Cost:** each check sends a one-word task ("Reply with the single word OK."). Together they use
  a small amount of your Claude, ChatGPT, and Google plans, like asking each one a quick question.
  A model your plan doesn't allow just says no.
- **Privacy:** the scripts keep only model names, the kind of sign-in, and the first words of each
  answer, and they hide email addresses themselves. Before you send anything, look it over and
  remove any email address, account name, or sign-in code that slipped through. **Never send a
  password or a key.**
- **How sure I am:** each script was tested on Plenipo's build machine (Linux, PowerShell 7.4)
  against stand-ins for Claude Code and Codex, and against the real Gemini CLI 0.61.0 signed out.
  None has run on Windows yet. If a step fails, send me the error; that helps too.

## 0. Open PowerShell 7 and make a folder for the results

Use **PowerShell 7** (`pwsh`), not the older Windows PowerShell. If `pwsh` is not found, run
`winget install Microsoft.PowerShell` first, then open a new window.

```powershell
pwsh
$out = "$env:USERPROFILE\Desktop\plenipo-checks"
New-Item -ItemType Directory -Force $out | Out-Null
Set-Location $out
$PSVersionTable.PSVersion.ToString()
node --version
```

You want `7.4` or newer, and Node.js `v20` or newer (Codex and Gemini run on Node.js; if `node` is
missing, run `winget install OpenJS.NodeJS.LTS` and open a new window). A folder called
**plenipo-checks** is now on your Desktop. Every result lands there.

If you close the window partway, open `pwsh` again and run the `$out = …` and `Set-Location $out`
lines before carrying on.

## Part A — Claude: the exact model behind each name (about 5 minutes)

**Why:** Claude Code's names `fable`, `opus`, `sonnet`, and `haiku` always point to the newest
model of each kind. Plenipo will show the exact version each one points to ("Opus — now Opus 5.5"),
and let a role stay on one exact version. This finds out which exact versions they point to on
your Claude Code, and checks that your subscription runs each exact version by name.

**A0. No keys in the way.** A key setting on your PC makes Claude Code or Codex use a
pay-per-use key instead of your subscription, even when you are signed in. This prints **names
only**, never values. You want no output.

```powershell
foreach ($scope in 'Process', 'User', 'Machine') {
  foreach ($key in [Environment]::GetEnvironmentVariables($scope).Keys) {
    if ($key -match 'ANTHROPIC|CLAUDE_CODE_USE|OPENAI_API_KEY|CODEX_API_KEY') { "$scope has $key set" }
  }
}
```

If a name prints, turn it off **for this window only** (nothing is deleted), for example
`Remove-Item Env:ANTHROPIC_API_KEY`, and do the same in any other window you use for these checks.
Never send its value.

**A1.** Your Claude Code version and sign-in:

```powershell
claude --version | Tee-Object -FilePath claude-version.txt
claude auth status 2>&1 | Tee-Object -FilePath claude-sign-in.txt
```

You want your Claude subscription (Pro or Max), not an API key. If you're not signed in, run
`claude auth login` and finish in your browser.

**A2.** Paste this whole block into the window and press Enter. It runs one tiny task per name,
then one per exact version it found, and saves `claude-models.txt`.

```powershell
# Part A: which exact Claude model each name runs, and whether your subscription runs it.
# Keeps only the model names, the kind of sign-in, and the first words of each answer.
$prompt = "Reply with the single word OK."
function Test-Claude([string]$Name) {
  $lines = $prompt | claude -p --output-format stream-json --verbose --tools "" --strict-mcp-config --model $Name 2>&1
  $init = $null; $result = $null
  foreach ($line in $lines) {
    try { $j = "$line" | ConvertFrom-Json -ErrorAction Stop } catch { continue }
    if ($j.type -eq 'system' -and $j.subtype -eq 'init') { $init = $j }
    if ($j.type -eq 'result') { $result = $j }
  }
  $used = @()
  if ($result -and $result.modelUsage) { $used = @($result.modelUsage.PSObject.Properties.Name) }
  $said = if ($result -and $result.result) { "$($result.result)" } else { (($lines | Select-Object -Last 2) -join ' ') }
  if ($said.Length -gt 120) { $said = $said.Substring(0, 120) }
  [pscustomobject]@{
    Asked   = $Name
    Ran     = if ($init) { $init.model } else { '' }
    Used    = $used -join ', '
    SignIn  = if ($init) { $init.apiKeySource } else { '' }
    Worked  = [bool]($result -and -not $result.is_error)
    Said    = $said
  }
}
$names = 'fable', 'opus', 'sonnet', 'haiku'
$first = foreach ($n in $names) { Test-Claude $n }
$exact = $first | ForEach-Object { $_.Ran; $_.Used -split ', ' } |
  Where-Object { $_ -and ($names -notcontains $_) } | Sort-Object -Unique
$second = foreach ($n in $exact) { Test-Claude $n }
@($first) + @($second) | Format-List | Out-String -Width 400 | Tee-Object -FilePath claude-models.txt
```

What to look for: `SignIn : none` means your Claude subscription (not a key) ran it, and
`Worked : True` means it worked. For the four names, `Ran` is the exact version each points to.

**A3 (optional).** Start `claude` on its own, type `/model`, and take a screenshot of the list. Press
`Esc`, then type `/exit`.

## Part B — Codex: the older OpenAI models your ChatGPT sign-in allows (about 10 minutes)

**Why:** Plenipo lists the models Codex shows in its picker. Codex also knows older models it
hides. This asks Codex itself for its whole list, hidden ones included (OpenAI's documented app
server — the same way Plenipo already reads Codex's models, with no task), then runs a one-word
task on each older one to see which your ChatGPT sign-in really allows. A few older names that may
not be on Codex's list are tried too; for those, "not supported" is a useful answer.

**B1.** Your Codex version and the kind of sign-in (not your email):

```powershell
codex --version | Tee-Object -FilePath codex-version.txt
codex login status 2>&1 | Tee-Object -FilePath codex-sign-in.txt
```

You want `Logged in using ChatGPT`. If it says an API key, stop and tell me.

**B2.** Save the small helper that asks Codex for its list. Paste the whole block, including its
first and last lines:

```powershell
@'
// Asks Codex's app server for every model it knows, hidden ones included (OpenAI's documented
// `model/list` with `includeHidden: true`). No task and no conversation. Keeps only each model's
// name, label, whether it is hidden, and its effort levels, in codex-models.json.
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";

const child = spawn("codex", ["-c", "check_for_update_on_startup=false", "app-server"], {
  shell: process.platform === "win32",
  stdio: ["pipe", "pipe", "ignore"],
});
const send = (m) => child.stdin.write(JSON.stringify(m) + "\n");
const models = [];
let buf = "";
const ask = (id, cursor) =>
  send({ method: "model/list", id, params: { limit: 100, includeHidden: true, ...(cursor ? { cursor } : {}) } });
const done = (why) => {
  writeFileSync("codex-models.json", JSON.stringify({ why, models }, null, 2));
  console.log(`${why}: ${models.length} models, saved in codex-models.json`);
  child.kill();
  process.exit(0);
};
child.stdout.on("data", (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf("\n")) >= 0) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    let m;
    try { m = JSON.parse(line); } catch { continue; }
    if (m.id === 1) { send({ method: "initialized" }); ask(2); }
    else if (typeof m.id === "number" && m.id >= 2) {
      if (m.error) return done(`Codex refused: ${m.error.message}`);
      for (const x of m.result?.data ?? []) {
        models.push({
          model: x.model, label: x.displayName, hidden: x.hidden ?? false, isDefault: x.isDefault ?? false,
          efforts: (x.supportedReasoningEfforts ?? []).map((e) => e.reasoningEffort ?? e),
        });
      }
      if (m.result?.nextCursor && m.id < 20) ask(m.id + 1, m.result.nextCursor);
      else done("Done");
    }
  }
});
child.on("exit", () => done("Codex stopped"));
setTimeout(() => done("No answer in 30 seconds"), 30000);
send({ method: "initialize", id: 1, params: { clientInfo: { name: "plenipo-check", title: "Plenipo check", version: "1.0.0" } } });
'@ | Set-Content -Encoding utf8 codex-models.mjs
```

**B3.** Paste this block. It saves `codex-models.json`, `codex-list.txt`, and `codex-older.txt`.

```powershell
# Part B: the older OpenAI models your ChatGPT sign-in really runs in Codex.
# 1. Codex's own full list, hidden (older) models included.
node codex-models.mjs
$listed = @()
if (Test-Path codex-models.json) { $listed = @((Get-Content codex-models.json -Raw | ConvertFrom-Json).models) }
$listed | Format-Table model, label, hidden, @{ n = 'efforts'; e = { $_.efforts -join ' ' } } -AutoSize |
  Out-String -Width 300 | Tee-Object -FilePath codex-list.txt
# 2. One tiny task on each hidden model, and on a few older names to try (a refusal is an answer too).
$try = @($listed | Where-Object { $_.hidden } | ForEach-Object { $_.model })
$try += 'gpt-5.4', 'gpt-5.3-codex', 'gpt-5.2-codex', 'gpt-5.2', 'gpt-5.1-codex-max', 'gpt-5.1-codex',
        'gpt-5.1', 'gpt-5-codex', 'gpt-5', 'o3', 'o4-mini', 'gpt-4.1'
$try = $try | Where-Object { $_ } | Select-Object -Unique
$prompt = "Reply with the single word OK."
$rows = foreach ($m in $try) {
  $lines = $prompt | codex exec --json --sandbox read-only --skip-git-repo-check `
    -c features.shell_tool=false -c features.view_image=false -c check_for_update_on_startup=false `
    --model $m 2>&1
  $ran = $false; $said = ''
  foreach ($line in $lines) {
    try { $j = "$line" | ConvertFrom-Json -ErrorAction Stop } catch { continue }
    if ($j.type -eq 'turn.completed') { $ran = $true }
    if ($j.type -eq 'item.completed' -and $j.item.type -eq 'agent_message') { $said = "$($j.item.text)" }
    if ($j.type -eq 'turn.failed' -and $j.error.message) { $said = "$($j.error.message)" }
    if ($j.type -eq 'error' -and -not $said) { $said = "$($j.message)" }
  }
  if (-not $said) { $said = (($lines | Select-Object -Last 2) -join ' ') }
  if ($said.Length -gt 160) { $said = $said.Substring(0, 160) }
  [pscustomobject]@{ Model = $m; Ran = $ran; Said = $said }
}
$rows | Format-Table -AutoSize -Wrap | Out-String -Width 300 | Tee-Object -FilePath codex-older.txt
```

What to look for: in `codex-older.txt`, `True` in the `Ran` column means your ChatGPT sign-in runs
that model.

## Part C — Google's Gemini CLI, step 0 (about 15 minutes)

**Why:** before Plenipo adds an AI tool, its real program must pass ADR-014's bar (adding AI tools)
on your PC. The part Gemini CLI might miss is a **sign-in status check**: a way for Plenipo to ask
"is this signed in with your Google subscription, and not a pay-per-use key?" before every task.
Gemini has no command for that. Inside its ACP mode (the same kind of connection Plenipo uses for
Grok and Kimi) it has `/about`, which reports the kind of sign-in and your plan without asking the
model. These steps find out whether that is good enough, plus everything else the bar needs. If it
isn't, Gemini gets a written finding, as GitHub Copilot did, not a workaround.

What I already saw on the build machine, signed out, is on
[the evidence page](evidence/phase-16/README.md).

**C1. No keys or cloud settings in the way.** These variables would make Gemini use a key or a
paid Google Cloud account instead of your own sign-in. This prints **names only**, never values.
You want no output.

```powershell
$names = 'GEMINI_API_KEY', 'GOOGLE_API_KEY', 'GOOGLE_APPLICATION_CREDENTIALS', 'GOOGLE_CLOUD_ACCESS_TOKEN',
  'GOOGLE_CLOUD_PROJECT', 'GOOGLE_CLOUD_PROJECT_ID', 'GOOGLE_CLOUD_QUOTA_PROJECT', 'GOOGLE_CLOUD_LOCATION',
  'GOOGLE_GENAI_USE_VERTEXAI', 'GOOGLE_GENAI_USE_GCA', 'GOOGLE_GEMINI_BASE_URL', 'GOOGLE_VERTEX_BASE_URL',
  'GEMINI_DEFAULT_AUTH_TYPE', 'GEMINI_API_KEY_AUTH_MECHANISM', 'GEMINI_CLI_CUSTOM_HEADERS', 'GEMINI_CLI_HOME'
foreach ($scope in 'Process', 'User', 'Machine') {
  foreach ($key in [Environment]::GetEnvironmentVariables($scope).Keys) {
    if ($names -contains $key) { "$scope has $key set" }
  }
}
```

If a name prints, tell me before going on (never its value).

**C2. Install Gemini CLI and see what Windows gets.**

```powershell
npm install -g @google/gemini-cli
gemini --version | Tee-Object -FilePath gemini-version.txt
$where = (Get-Command gemini).Source
$where | Tee-Object -FilePath gemini-install.txt
Get-ChildItem (Split-Path $where) -Filter 'gemini*' | Select-Object Name, Length |
  Out-String | Add-Content gemini-install.txt
winget search --name "Gemini CLI" 2>&1 | Out-String | Add-Content gemini-install.txt
gemini --help 2>&1 | Out-File gemini-help.txt
```

`gemini-install.txt` shows whether Windows got a real `gemini.exe` or only npm's `gemini.cmd` and
`gemini.ps1` shortcuts that start Node.js (the build machine suggests shortcuts only), and whether
Google publishes a WinGet package.

**C3. Sign in with your Google account.** Run `gemini` on its own. If it asks, trust this folder and
pick a color theme. When it asks how to sign in, choose **Sign in with Google** and finish in your
browser, with the Google account that has your Google AI plan. Back at Gemini's prompt, type
`/about` and take a screenshot. **Cover your email address in the screenshot.** Then type `/quit`.

**C4. The possible status check, signed in.** Save the helper, then run it. It talks to Gemini the
way Plenipo would: start, open a conversation, send `/about`. It hides your email itself.

```powershell
@'
// Talks to Gemini CLI over ACP, as Plenipo would for a sign-in check: initialize, a new
// conversation, and the prompt "/about" (Gemini's own command; it does not ask the model).
// Saves every message in the file named first (default gemini-acp.txt), with email addresses hidden.
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";

const file = process.argv[2] ?? "gemini-acp.txt";
const hide = (s) =>
  s.replace(/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g, "<email hidden>")
   .replace(/(User Email:)[^\\\n"]*/g, "$1 <hidden>");
const log = [];
const note = (s) => { const t = hide(s); log.push(t); console.log(t.length > 300 ? t.slice(0, 300) + " …" : t); };
const child = spawn("gemini", ["--acp"], {
  shell: process.platform === "win32",
  stdio: ["pipe", "pipe", "pipe"],
});
let session = null;
const send = (m) => { const line = JSON.stringify(m); note(">> " + line); child.stdin.write(line + "\n"); };
const finish = (why) => {
  note("-- " + why);
  writeFileSync(file, log.join("\n") + "\n");
  child.kill();
  process.exit(0);
};
let buf = "";
child.stdout.on("data", (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf("\n")) >= 0) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    note("<< " + line);
    let m;
    try { m = JSON.parse(line); } catch { continue; }
    if (m.id === 1) send({ jsonrpc: "2.0", id: 2, method: "session/new", params: { cwd: process.cwd(), mcpServers: [] } });
    else if (m.id === 2 && m.error) finish("no conversation: " + m.error.message);
    else if (m.id === 2) {
      session = m.result.sessionId;
      send({ jsonrpc: "2.0", id: 3, method: "session/prompt", params: { sessionId: session, prompt: [{ type: "text", text: "/about" }] } });
    } else if (m.id === 3) finish("done");
    else if (m.method === "session/request_permission" && m.id !== undefined) {
      send({ jsonrpc: "2.0", id: m.id, result: { outcome: { outcome: "cancelled" } } });
    }
  }
});
child.stderr.on("data", (d) => note("stderr: " + String(d).trimEnd()));
child.on("exit", (c) => finish("Gemini stopped, exit code " + c));
setTimeout(() => finish("no answer in 60 seconds"), 60000);
send({ jsonrpc: "2.0", id: 1, method: "initialize", params: {
  protocolVersion: 1,
  clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false },
} });
'@ | Set-Content -Encoding utf8 gemini-acp.mjs
node gemini-acp.mjs gemini-acp-signed-in.txt
```

**C5. One task, then the same conversation again.** The task's words go in on standard input, and
the answer comes back as JSON lines, in read-only mode (`--approval-mode plan`). `--skip-trust`
trusts this folder for this one run (the build machine showed Gemini refuses a one-off task in a
folder it hasn't been told to trust). Paste this block:

```powershell
# One task with the prompt on standard input, then a second task that resumes it by its ID.
$prompt = "Reply with the single word OK."
$id = [guid]::NewGuid().ToString()
$hide = { param($t) $t -replace '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}', '<email hidden>' }
$first = $prompt | gemini --output-format stream-json --approval-mode plan --skip-trust --session-id $id 2>&1
$code1 = $LASTEXITCODE
& $hide (($first | ForEach-Object { "$_" }) -join "`n") | Out-File gemini-task.txt
"exit code: $code1" | Add-Content gemini-task.txt
$second = "Which single word did you reply with just now?" |
  gemini --output-format stream-json --approval-mode plan --skip-trust --resume $id 2>&1
$code2 = $LASTEXITCODE
& $hide (($second | ForEach-Object { "$_" }) -join "`n") | Out-File gemini-resume.txt
"exit code: $code2" | Add-Content gemini-resume.txt
gemini --list-sessions 2>&1 | ForEach-Object { "$_" } | Out-File gemini-sessions.txt
"First task: exit code $code1. Resumed task: exit code $code2."
```

It saves `gemini-task.txt`, `gemini-resume.txt`, and `gemini-sessions.txt`. The second task should
remember the first one's answer.

**C6. The same checks as if you were signed out, and as if a key were used.** This doesn't sign you
out: it points Gemini at an empty settings folder for this window only. The key is **made up**
(`not-a-real-key`), so nothing is billed; Google just refuses it.

```powershell
$env:GEMINI_CLI_HOME = "$out\gemini-empty-home"
New-Item -ItemType Directory -Force $env:GEMINI_CLI_HOME | Out-Null
node gemini-acp.mjs gemini-acp-signed-out.txt
"Reply with the single word OK." | gemini --output-format stream-json --skip-trust 2>&1 |
  ForEach-Object { "$_" } | Out-File gemini-task-signed-out.txt
"exit code: $LASTEXITCODE" | Add-Content gemini-task-signed-out.txt
$env:GEMINI_API_KEY = "not-a-real-key"
node gemini-acp.mjs gemini-acp-made-up-key.txt
"Reply with the single word OK." | gemini --output-format stream-json --skip-trust 2>&1 |
  ForEach-Object { "$_" } | Out-File gemini-task-made-up-key.txt
"exit code: $LASTEXITCODE" | Add-Content gemini-task-made-up-key.txt
Remove-Item Env:GEMINI_API_KEY
Remove-Item Env:GEMINI_CLI_HOME
```

**C7. Your Google plan.** Tell me which Google AI plan the account has (for example Google AI Pro,
Google AI Ultra, or none). The "Tier" line of `/about` in C3 may already say it.

**Then stop.** I write nothing for Gemini until I have these results.

## Part D — More Ollama cloud models (only if your paid Ollama plan is active)

Skip this part if the paid plan isn't active yet.

**D1.** Open Plenipo → **AI tools** → the **Ollama** card, and tell me what it says after "Signed
in" (the plan's name).

**D2.** The six models Plenipo lists as "(paid plan)" should now answer. Paste this:

```powershell
ollama --version | Tee-Object -FilePath ollama-version.txt
$models = 'kimi-k3:cloud', 'deepseek-v4-pro:cloud', 'deepseek-v4.1-flash:cloud', 'glm-5.3:cloud',
  'glm-5.3-flash:cloud', 'minimax-m3:cloud'
$rows = foreach ($m in $models) {
  $said = ("Reply with the single word OK." | ollama run $m 2>&1 | ForEach-Object { "$_" }) -join ' '
  if ($said.Length -gt 160) { $said = $said.Substring(0, 160) }
  [pscustomobject]@{ Model = $m; Said = $said }
}
$rows | Format-Table -AutoSize -Wrap | Out-String -Width 300 | Tee-Object -FilePath ollama-paid.txt
```

**D3.** Pick any other cloud models you want from
[ollama.com/search?c=cloud](https://ollama.com/search?c=cloud) and tell me their names. For each
one, run these lines with its name in place of `example-model:cloud`:

```powershell
$m = 'example-model:cloud'
ollama show $m 2>&1 | Out-File ("ollama-show-" + ($m -replace '[:/]', '_') + ".txt")
("Reply with the single word OK." | ollama run $m 2>&1) -join ' ' | Tee-Object -Append -FilePath ollama-more.txt
```

## Sending me the results

1. Open the **plenipo-checks** folder on your Desktop.
2. Open each `.txt` and `.json` file and look for an email address, account name, or sign-in code.
   Remove any you find.
3. Attach the files and your screenshots in our chat, or paste each file's text. Tell me your
   Google plan (C7) and, if you did Part D, the Ollama plan's name and the models you want.

When you're done, you can delete the folder. Gemini CLI can stay installed, or be removed with
`npm uninstall -g @google/gemini-cli`.
