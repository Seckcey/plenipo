# Copilot: owner's check on Windows (about 15 minutes)

These steps check GitHub Copilot CLI's sign-in and allowance on your own PC with your own
Copilot account. The results decide whether Copilot can be tried again (see
[the finding](ai-tools-copilot-finding.md) and
[the notes for the decision record](ai-tools-copilot-decision-notes.md)).

- **Plenipo is not involved.** Nothing here installs or changes Plenipo.
- **Cost.** Steps 7 to 9 run three short tasks, which use a few requests from your monthly
  Copilot allowance. Step 5 shows which models are included at no extra cost.
- **Privacy.** Before you paste any output back, remove your email address, GitHub user name,
  account or organization IDs, and any sign-in code. The probe in step 5 hides your user name
  itself, but check anyway. Never paste a token.
- **Not tested on Windows.** These commands were written for PowerShell 7 and checked against
  Copilot CLI 1.0.88 on Linux. They were not run on Windows. If a step fails, send the error.

## 1. Open PowerShell 7 and make a folder for the results

Copilot CLI needs PowerShell 6 or newer on Windows. If `pwsh` is missing, run
`winget install Microsoft.PowerShell` first.

```powershell
pwsh
$check = "$env:TEMP\copilot-check"
New-Item -ItemType Directory -Force $check | Out-Null
Set-Location $check
$PSVersionTable.PSVersion.ToString()
```

## 2. Check that no token or custom-provider setting is present

These variables would make Copilot use a token or a pay-per-use provider instead of your own
sign-in. The command prints **names only**, never values. You want no output, and `False` for
the file.

```powershell
$names = 'GH_TOKEN','GITHUB_TOKEN','COPILOT_GITHUB_TOKEN','GITHUB_COPILOT_GITHUB_TOKEN',
         'GITHUB_PERSONAL_ACCESS_TOKEN','COPILOT_PROVIDERS_CONFIG','COPILOT_API_URL'
foreach ($scope in 'Process','User','Machine') {
  foreach ($key in [Environment]::GetEnvironmentVariables($scope).Keys) {
    if ($names -contains $key -or $key -like 'COPILOT_PROVIDER_*') { "$scope has $key set" }
  }
}
Test-Path "$env:USERPROFILE\.copilot\providers.json"
```

If a name prints, remove it from this window before going on, for example
`Remove-Item Env:GH_TOKEN`. If it was set for your user or the machine, it comes back in new
windows. Remove it in _System Properties → Environment Variables_, then start again from step 1.

## 3. Install Copilot CLI and read its version

Use one of these, not both.

```powershell
winget install GitHub.Copilot          # native copilot.exe
# or: npm install -g @github/copilot   # needs Node.js; installs copilot.exe inside the npm package
```

Close PowerShell, open a new `pwsh`, go back to the folder, and run:

```powershell
Set-Location "$env:TEMP\copilot-check"
copilot --version
(Get-Command copilot).Source
```

Expected: `GitHub Copilot CLI 1.0.88.` or newer. `Source` shows where it is installed: a
`copilot.exe` (winget), or a `copilot.ps1` or `copilot.cmd` shim (npm). The first start unpacks
about 170 MB, so it can take a minute.

## 4. Sign in, and confirm there is no status command

```powershell
copilot login
copilot login status
"exit code: $LASTEXITCODE"
```

`copilot login` opens your browser. Sign in with the GitHub account that has your Copilot plan.
`copilot login status` should be refused with `unexpected argument 'status'`, which confirms
there is no status command.

## 5. Ask Copilot for its sign-in type, your allowance, and its models

This uses the connection GitHub's own Copilot SDK uses (`copilot --headless --stdio`). It needs
Node.js 20 or newer: check with `node --version`, and install it with
`winget install OpenJS.NodeJS.LTS` if it is missing. The script only reads. It hides your user
name, and the only files it leaves are itself and `probe.json`.

```powershell
$exe = (Get-Command copilot).Source
if ($exe -notlike '*.exe') {
  $arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
  $exe = Join-Path (Split-Path $exe) "node_modules\@github\copilot\node_modules\@github\copilot-win32-$arch\copilot.exe"
}
Test-Path $exe
$env:COPILOT_EXE = $exe

@'
import { spawn } from "node:child_process";
const exe = process.env.COPILOT_EXE || "copilot";
const child = spawn(exe, ["--headless", "--stdio", "--no-auto-update", "--log-level", "none"], {
  stdio: ["pipe", "pipe", "inherit"],
});
child.on("error", (e) => { console.log(`Could not start ${exe}: ${e.message}`); process.exit(1); });
let buf = Buffer.alloc(0);
let id = 0;
const waiting = new Map();
child.stdout.on("data", (d) => {
  buf = Buffer.concat([buf, d]);
  for (;;) {
    const head = buf.indexOf("\r\n\r\n");
    if (head < 0) return;
    const len = Number(/Content-Length: (\d+)/i.exec(buf.subarray(0, head).toString())[1]);
    if (buf.length < head + 4 + len) return;
    const msg = JSON.parse(buf.subarray(head + 4, head + 4 + len).toString());
    buf = buf.subarray(head + 4 + len);
    if (msg.id && !msg.method && waiting.has(msg.id)) {
      waiting.get(msg.id)(msg);
      waiting.delete(msg.id);
    }
  }
});
function call(method) {
  const body = JSON.stringify({ jsonrpc: "2.0", id: ++id, method, params: {} });
  child.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
  return new Promise((resolve) => waiting.set(id, resolve));
}
const timer = setTimeout(() => { console.log("Timed out after 60 seconds."); child.kill(); process.exit(1); }, 60000);
const out = { version: (await call("connect")).result?.version };
const auth = await call("auth.getStatus");
const login = auth.result?.login;
out.signIn = auth.result ?? auth.error;
const quota = await call("account.getQuota");
out.allowance = quota.result?.quotaSnapshots ?? quota.error;
const models = await call("models.list");
out.models = models.result?.models?.map((m) => ({
  id: m.id, name: m.name, effort: m.supportedReasoningEfforts, defaultEffort: m.defaultReasoningEffort,
  multiplier: m.billing?.multiplier, price: m.modelPickerPriceCategory, category: m.modelPickerCategory,
  policy: m.policy?.state, metadata: m.metadata,
})) ?? models.error;
clearTimeout(timer);
child.kill();
let text = JSON.stringify(out, null, 1);
if (login) text = text.split(login).join("(hidden)");
console.log(text);
process.exit(0);
'@ | Set-Content -Encoding utf8 copilot-probe.mjs

node .\copilot-probe.mjs | Tee-Object probe.json
```

What to look for in `probe.json`:

- `signIn.isAuthenticated` should be `true`, and `signIn.authType` should be `user`. If it is
  `gh-cli`, `env`, or `token`, Copilot is using another sign-in or a token. Say so.
- `allowance`: one entry per allowance. For each, `overageAllowedWithExhaustedQuota` says
  whether GitHub will charge for extra use once the allowance runs out.
- `models`: each model's ID, name, effort levels, and `multiplier`. A multiplier of 0 means
  the model is included at no extra cost. If one has 0, use it in steps 7 to 9: run
  `$model = '<its id>'`. Otherwise run `$model = $null`.

## 6. Look at your GitHub setting for paid extra use

In the browser, open **github.com → your picture → Settings → Billing and licensing → Budgets and
alerts**, and look for a budget for Copilot (premium requests, or AI credits). If your Copilot
plan comes from an organization, its owner sets this, and you may not see it. I could not
check this page from here, so tell me what it shows: the budget amount, and whether it stops
usage at the limit. Compare it with `overageAllowedWithExhaustedQuota` from step 5.

## 7. One task, prompt on standard input, JSON output, no tools

```powershell
$flags = @('--output-format','json','--no-auto-update','--available-tools=plenipo_no_tools',
           '--disable-builtin-mcps','--no-ask-user')
if ($model) { $flags += @('--model', $model) }
'Remember the word heron. Reply with just OK.' | copilot @flags > turn-new.jsonl 2> turn-new.stderr.txt
"exit code: $LASTEXITCODE"
Get-Content turn-new.jsonl | Select-Object -Last 3
Select-String -Path turn-new.jsonl -Pattern '"type":"assistant.usage"|quotaSnapshots|isByok' | Measure-Object | Select-Object Count
```

Expected: exit code 0, and a last line with `"type":"result"`, a `sessionId`, and
`usage.premiumRequests`. The `Count` line tells whether the real output includes the
per-request allowance report. With the stand-in model it did not.

## 8. Resume that conversation by its ID

```powershell
$sid = ((Select-String -Path turn-new.jsonl -Pattern '"type":"result"').Line | ConvertFrom-Json).sessionId
'What word did I ask you to remember? Reply with just the word.' | copilot @flags --resume $sid > turn-resume.jsonl 2> turn-resume.stderr.txt
"exit code: $LASTEXITCODE"
Select-String -Path turn-resume.jsonl -Pattern '"type":"assistant.message"' | Select-Object -Last 1
```

Expected: exit code 0, and an answer of `heron`.

## 9. A model that does not exist

```powershell
'Reply with OK.' | copilot @flags --model not-a-model > turn-bad-model.jsonl 2> turn-bad-model.stderr.txt
"exit code: $LASTEXITCODE"
Get-Content turn-bad-model.stderr.txt
Get-Content turn-bad-model.jsonl | Select-Object -Last 3
```

This shows how Copilot refuses an unknown model, and whether it lists the real ones.

## 10. The same task signed out

This runs one task with an empty Copilot folder, so Copilot cannot find your sign-in.

```powershell
$env:COPILOT_HOME = "$check\empty-home"
New-Item -ItemType Directory -Force $env:COPILOT_HOME | Out-Null
'Reply with OK.' | copilot @flags > turn-signed-out.jsonl 2> turn-signed-out.stderr.txt
"exit code: $LASTEXITCODE"
Get-Content turn-signed-out.stderr.txt
Remove-Item Env:COPILOT_HOME
```

Expected: exit code 1 and `Error: No authentication information found.` If it succeeds
instead, Copilot found the GitHub CLI's (`gh`) sign-in. Say so.

## 11. Send the results

```powershell
Get-ChildItem $check -File | Select-Object Name, Length
```

Paste back `probe.json` and the `.jsonl` and `.stderr.txt` files from steps 7 to 10, plus what
step 6 showed. **First remove your email address, GitHub user name, account or organization IDs,
and any sign-in code.** When you are done, you can delete the folder:
`Remove-Item -Recurse "$env:TEMP\copilot-check"`.
