# Phase 16 Wave 1 — checks on your Windows PC

**What this is.** Short checks, run on your own PC with your own sign-ins. They tell me what your
subscriptions really allow, so nothing in Plenipo's model lists is a guess. Nothing here installs
or changes Plenipo.

**Where things stand (2026-09-29):**

- **Part A (Claude): done.** Thank you.
- **Part B (Codex): done** on Codex 0.159.0. Thank you.
- **Part C (Gemini CLI): done — it can't be used.** Google stopped serving Gemini CLI to personal
  Google plans; see [the finding](ai-tools-gemini-finding.md).
- **Part E (Antigravity CLI, Google's replacement): E1 to E8 done** — it answered a task and
  continued the conversation on your Google sign-in. **One more step, E9, please**, plus your
  `/credits` screenshot and your Google plan's name (E4, E8). I stop for Antigravity until then.
- **Part D (Ollama):** your paid plan starts 2026-09-30, so this waits for a small follow-up.

- **Time:** about 30 minutes, most of it waiting.
- **Cost:** each check sends a one-word task ("Reply with the single word OK."). Together they use
  a small amount of your Claude, ChatGPT, and Google plans, like asking each one a quick question.
  A model your plan doesn't allow just says no.
- **Privacy:** the scripts keep only model names, the kind of sign-in, and the first words of each
  answer, and they hide email addresses themselves. Before you send anything, look it over and
  remove any email address, account name, or sign-in code that slipped through. **Never send a
  password or a key.**
- **How sure I am:** each script was tested on Plenipo's build machine (Linux, PowerShell 7.4)
  against stand-ins for Claude Code and Codex, and against the real Gemini CLI 0.61.0 and
  Antigravity CLI 1.2.13, signed out and with a made-up key. Parts A to C then ran on your PC. If a
  step fails, send me the error; that helps too.

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

**B0. Update Codex first.** Its list of models depends on its version.

```powershell
npm install -g @openai/codex@latest
codex --version
```

You want `0.157.1` or newer. If it still says `0.145.0`, Codex was installed another way: send me
what `(Get-Command codex).Source` prints. Then close PowerShell 7, open it again, and run the three
lines of step 0 before B1.

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

## Part C — Google's Gemini CLI: done, it can't be used

On your PC, **Sign in with Google** was refused: "This client is no longer supported for Gemini
Code Assist for individuals." Since 2026-06-18, Google serves Gemini CLI only to business licenses,
pay-per-use keys, and Google Cloud accounts, which Plenipo's rules refuse. The written finding is
[here](ai-tools-gemini-finding.md). You can remove it: `npm uninstall -g @google/gemini-cli`.
Google's replacement, Antigravity CLI, is Part E.

## Part E — Google's Antigravity CLI, step 0 (about 15 minutes)

**Why:** Gemini CLI can't sign in with a personal Google plan any more (Part C), so at your
direction Plenipo checks Google's replacement, **Antigravity CLI** (the command is `agy`), against
the same bar. On the build machine it already did the hard parts: it reads the task from standard
input, prints its progress as JSON lines, and continues a conversation by its ID
([evidence](evidence/phase-16/README.md)). What only your PC can show:

1. **Is it signed in with your Google plan, not a key?** `agy models` is the likely check, the way
   Plenipo already checks Grok. Signed out, it refuses.
2. **Does it ever spend paid credits** when your plan's allowance runs out? Google's settings have
   a switch for that, `useG1Credits`. Plenipo must never let a task spend money.
3. **Does it update itself in the middle of work?** Plenipo updates AI tools only between tasks.

If it can't pass, it gets a written finding like Gemini CLI. **I stop again for Antigravity until
you send these results.**

Use a PowerShell 7 window in the checks folder (the three lines of step 0). You already installed
Antigravity and signed in, so there is nothing to install here unless `agy` is not found (then run
`irm https://antigravity.google/cli/install.ps1 | iex`, Google's own installer, and open a new
window).

**E1. No keys or cloud settings in the way.** Names only, never values. You want no output.

```powershell
foreach ($scope in 'Process', 'User', 'Machine') {
  foreach ($key in [Environment]::GetEnvironmentVariables($scope).Keys) {
    if ($key -match '^(GEMINI_|GOOGLE_|AGY_|ANTIGRAVITY_)') { "$scope has $key set" }
  }
}
```

If a name prints, tell me (never its value), and turn it off for this window with
`Remove-Item Env:NAME`, using the name that printed.

**E2. The program, its version, and its help.**

```powershell
agy --version 2>&1 | Tee-Object -FilePath agy-version.txt
(Get-Command agy).Source | Tee-Object -FilePath agy-install.txt
Get-Item (Get-Command agy).Source | Select-Object Name, Length | Out-String | Add-Content agy-install.txt
agy --help 2>&1 | Out-File agy-help.txt
```

**E3. The likely sign-in check.** Signed in with your Google plan, it should list models.

```powershell
agy models 2>&1 | Tee-Object -FilePath agy-models.txt
"exit code: $LASTEXITCODE" | Add-Content agy-models.txt
```

**E4. Its settings, and your plan's credits.** This shows Antigravity's own settings file. It holds
choices, not your password; still, look it over before you send it.

```powershell
$settings = "$env:USERPROFILE\.gemini\antigravity-cli\settings.json"
if (Test-Path $settings) { Get-Content $settings | Out-File agy-settings.txt } else { "no settings file" | Out-File agy-settings.txt }
Get-Content agy-settings.txt
```

Then start `agy` on its own, type `/credits`, and take a screenshot (**cover your email** if it
shows). Type `/config` too, and screenshot the part that shows **useG1Credits** if you find it.
Leave it as it is. Type `/exit` to leave.

**E5. One task, then the same conversation again.** The task's words go in on standard input
(never on the command line), read-only (`--mode plan --sandbox`). It saves `agy-task.txt` and
`agy-resume.txt`, with your home folder's name and any email address hidden. Paste this block:

```powershell
# One task with the words on standard input (never on the command line), read-only, then a
# second task that continues the same conversation by its ID. Saves agy-task.txt and
# agy-resume.txt, with your home folder's name and any email address hidden.
function Ask-Agy([string]$Text, [string]$Conversation) {
  $msg = @{ event = 'user'; message = @{ role = 'user'; content = $Text } } | ConvertTo-Json -Compress -Depth 5
  $flags = @('-p=', '--input-format', 'stream-json', '--output-format', 'stream-json', '--mode', 'plan', '--sandbox')
  if ($Conversation) { $flags += @('--conversation', $Conversation) }
  $lines = $msg | agy @flags 2>&1 | ForEach-Object { "$_" }
  $code = $LASTEXITCODE
  $id = ''; $said = ''; $status = ''
  foreach ($line in $lines) {
    try { $j = $line | ConvertFrom-Json -ErrorAction Stop } catch { continue }
    if ($j.event -eq 'init' -and $j.conversation_id) { $id = $j.conversation_id }
    if ($j.event -eq 'result') { $id = $j.result.conversation_id; $status = $j.result.status; $said = "$($j.result.response)$($j.result.error)" }
  }
  $home1 = [regex]::Escape($env:USERPROFILE); $home2 = [regex]::Escape($env:USERPROFILE.Replace('\', '\\'))
  $clean = ($lines -join "`n") -replace $home2, '<home>' -replace $home1, '<home>' -replace '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}', '<email hidden>'
  [pscustomobject]@{ Id = $id; Status = $status; Said = $said; ExitCode = $code; Raw = $clean }
}
$first = Ask-Agy "Reply with the single word OK."
$first.Raw + "`nexit code: $($first.ExitCode)" | Out-File agy-task.txt
$second = Ask-Agy "Which single word did you reply with just now?" $first.Id
$second.Raw + "`nexit code: $($second.ExitCode)" | Out-File agy-resume.txt
"First task:   status $($first.Status), conversation $($first.Id), said: $($first.Said)"
"Second task:  status $($second.Status), conversation $($second.Id), said: $($second.Said)"
```

You want a status that is not `ERROR` and "OK" for the first task, the **same conversation** ID on
both lines, and the second task remembering "OK".

**E6. As if you were signed out, and as if a key were used.** This doesn't sign you out: for a
moment, it points Antigravity at an empty home folder, then puts yours back. The key is **made up**
(`not-a-real-key`), so nothing is billed; Google just refuses it.

```powershell
$realHome = $env:USERPROFILE
$empty = Join-Path $out 'agy-empty-home'
$agyDir = Join-Path $empty '.gemini' 'antigravity-cli'
New-Item -ItemType Directory -Force $agyDir | Out-Null
try {
  $env:USERPROFILE = $empty
  agy models 2>&1 | Out-File agy-models-empty-home.txt
  "exit code: $LASTEXITCODE" | Add-Content agy-models-empty-home.txt
  '{"modelProvider": "gemini"}' | Set-Content (Join-Path $agyDir 'settings.json')
  $env:GEMINI_API_KEY = 'not-a-real-key'
  agy models 2>&1 | Out-File agy-models-made-up-key.txt
  "exit code: $LASTEXITCODE" | Add-Content agy-models-made-up-key.txt
} finally {
  Remove-Item Env:GEMINI_API_KEY -ErrorAction SilentlyContinue
  $env:USERPROFILE = $realHome
}
Get-Content agy-models-empty-home.txt, agy-models-made-up-key.txt
```

If the empty-home check still lists models, that's a useful answer too: it means your sign-in is
kept in Windows Credential Manager, not in the home folder.

**E7. Did it update itself?** Run this last, and compare with E2:

```powershell
agy --version 2>&1 | Tee-Object -FilePath agy-version-after.txt
```

**E8. Your plan.** Tell me which Google AI plan your Antigravity sign-in uses (for example Google AI
Pro), and whether `/credits` showed any paid credits.

**E9. Antigravity the way Plenipo would run it** (added after your first Part E results). Plenipo
would give Antigravity its own settings folder with **paid credits off** (`"useG1Credits": false`)
and strict permissions, turn its self-updates off (`AGY_CLI_DISABLE_AUTO_UPDATE=true`), and run it
read-only. Your first results showed your sign-in is kept in Windows Credential Manager, so it
still works with a different settings folder. This step checks that, asks Antigravity to write a
file in read-only mode (it should not manage to), and repeats the made-up-key check from E6 (that
part didn't run: the older Windows PowerShell doesn't accept one of its commands). Run the step 0
lines first so `$out` is set, then paste this block. It works in either PowerShell.

```powershell
# E9: Antigravity run the way Plenipo would: its own settings folder (paid credits off, strict
# permissions), no self-update, read-only mode. Your Google sign-in stays in Windows Credential
# Manager, so it still works. Works in PowerShell 7 and in the older Windows PowerShell.
$realHome = $env:USERPROFILE
$plenipoHome = Join-Path $out 'agy-plenipo-home'
$settingsDir = Join-Path (Join-Path $plenipoHome '.gemini') 'antigravity-cli'
$work = Join-Path $out 'agy-work'
New-Item -ItemType Directory -Force $settingsDir | Out-Null
New-Item -ItemType Directory -Force $work | Out-Null
'{"useG1Credits": false, "toolPermission": "strict"}' | Set-Content -Encoding ascii (Join-Path $settingsDir 'settings.json')
$hide = { param($t) ($t -replace [regex]::Escape($realHome.Replace('\', '\\')), '<home>') -replace [regex]::Escape($realHome), '<home>' -replace '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}', '<email hidden>' }
$save = { param($lines, $name) & $hide (($lines | ForEach-Object { "$_" }) -join "`n") | Set-Content -Encoding utf8 (Join-Path $out $name) }
Push-Location $work
try {
  $env:USERPROFILE = $plenipoHome
  $env:AGY_CLI_DISABLE_AUTO_UPDATE = 'true'
  # 1. The sign-in check, with Plenipo's own settings folder.
  $models = agy models 2>&1
  & $save (@($models) + "exit code: $LASTEXITCODE") 'agy-e9-models.txt'
  # 2. Ask it to write a file, in read-only mode. It should refuse or not manage it.
  $ask = '{"event":"user","message":{"role":"user","content":"Create a file named proof.txt in the current folder containing the word hello, then reply DONE."}}'
  $write = $ask | agy -p= --input-format stream-json --output-format stream-json --mode plan --sandbox 2>&1
  $code = $LASTEXITCODE
  & $save (@($write) + "exit code: $code" + "proof.txt written: $(Test-Path (Join-Path $work 'proof.txt'))") 'agy-e9-write.txt'
  # 3. A made-up key in the environment, with no key setting in the settings file: it should
  #    still use your Google sign-in (the key alone must not switch it to pay-per-use).
  $env:GEMINI_API_KEY = 'not-a-real-key'
  $keyModels = agy models 2>&1
  & $save (@($keyModels) + "exit code: $LASTEXITCODE") 'agy-e9-made-up-key.txt'
} finally {
  Remove-Item Env:GEMINI_API_KEY -ErrorAction SilentlyContinue
  Remove-Item Env:AGY_CLI_DISABLE_AUTO_UPDATE -ErrorAction SilentlyContinue
  $env:USERPROFILE = $realHome
  Pop-Location
}
Get-Content (Join-Path $out 'agy-e9-write.txt') | Select-Object -Last 2
"Models listed with Plenipo's settings folder: $(@($models | Where-Object { "$_" -match '\t' }).Count)"
"Models listed with a made-up key in the environment: $(@($keyModels | Where-Object { "$_" -match '\t' }).Count)"
```

It saves `agy-e9-models.txt`, `agy-e9-write.txt`, and `agy-e9-made-up-key.txt`. You want models
listed in both counts, and `proof.txt written: False`. If the counts are 0, send me the files:
that's an answer too.

**Then stop.** I write nothing for Antigravity until I have these results.

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
   Google plan (E8) and, if you did Part D, the Ollama plan's name and the models you want.

When you're done, you can delete the folder. Gemini CLI can stay installed, or be removed with
`npm uninstall -g @google/gemini-cli`.
