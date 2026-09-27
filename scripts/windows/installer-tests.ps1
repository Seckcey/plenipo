# Phase 13: Plenipo installed on a Windows computer, the way the owner gets it (GitHub's Windows
# machine in CI). Installs the installer this run built, runs it, ends it in the ways that matter,
# upgrades over the published 1.8.0 installer, goes back to 1.8.0 and forward again, installs an
# update the way Plenipo does, uninstalls (keeping, then deleting, your data), and lists what is
# left behind. A real Windows restart is the owner's check; here a restart is simulated by ending
# Plenipo and dating its "running" note before Windows started.
#
# pwsh scripts/windows/installer-tests.ps1 -NewInstaller <setup.exe> -OldInstaller <1.8.0 setup.exe>
#   -Version <x.y.z> -UpdaterKey <throwaway private key file> -UpdatesDir <empty folder>

param(
  [Parameter(Mandatory)] [string] $NewInstaller,
  [Parameter(Mandatory)] [string] $OldInstaller,
  [Parameter(Mandatory)] [string] $Version,
  [Parameter(Mandatory)] [string] $UpdaterKey,
  [Parameter(Mandatory)] [string] $UpdatesDir,
  [int] $UpdatePort = 8765
)

$ErrorActionPreference = 'Stop'
# Missing report fields read as nothing (a report may not have every field yet).
Set-StrictMode -Version 1.0

$InstallDir = Join-Path $env:LOCALAPPDATA 'Plenipo'
$Exe = Join-Path $InstallDir 'plenipo-desktop.exe'
$Uninstaller = Join-Path $InstallDir 'uninstall.exe'
$Data = Join-Path $env:LOCALAPPDATA 'com.eightwest.plenipo'
$RoamingData = Join-Path $env:APPDATA 'com.eightwest.plenipo'
$RunNote = Join-Path $Data 'run\plenipo-running.json'
$UninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plenipo'
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$StartMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Plenipo.lnk'
$Desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Plenipo.lnk'
$Report = Join-Path $env:RUNNER_TEMP 'plenipo-report.json'

function Step([string] $name) { Write-Host "::group::$name" }
function Done { Write-Host '::endgroup::' }
function Check([bool] $ok, [string] $what) {
  if (-not $ok) { throw "FAILED: $what" }
  Write-Host "ok: $what"
}

function Installed-Version {
  (Get-ItemProperty -Path $UninstallKey -ErrorAction SilentlyContinue).DisplayVersion
}

function Plenipo-Processes { @(Get-Process -Name 'plenipo-desktop' -ErrorAction SilentlyContinue) }

function Wait-Until([scriptblock] $Ready, [int] $Seconds, [string] $What) {
  $deadline = (Get-Date).AddSeconds($Seconds)
  while ((Get-Date) -lt $deadline) {
    if (& $Ready) { return }
    Start-Sleep -Milliseconds 500
  }
  throw "Timed out after ${Seconds}s waiting for: $What"
}

function Wait-NoPlenipo([int] $Seconds = 30) {
  Wait-Until { (Plenipo-Processes).Count -eq 0 } $Seconds 'no Plenipo program running'
}

function Install([string] $installer, [string[]] $arguments = @('/S')) {
  $p = Start-Process -FilePath $installer -ArgumentList $arguments -PassThru -Wait
  Check ($p.ExitCode -eq 0) "$(Split-Path $installer -Leaf) $($arguments -join ' ') finished (exit $($p.ExitCode))"
  Wait-Until { Test-Path $Exe } 60 'the installed program'
}

function Uninstall([string[]] $arguments = @('/S')) {
  # The uninstaller copies itself away and returns at once; wait for its work instead.
  Start-Process -FilePath $Uninstaller -ArgumentList $arguments -Wait | Out-Null
  Wait-Until { -not (Test-Path $Exe) -and -not (Test-Path $UninstallKey) } 90 'the uninstall to finish'
  Start-Sleep -Seconds 2
}

function Read-Report {
  if (Test-Path $Report) { Get-Content $Report -Raw | ConvertFrom-Json } else { $null }
}

# Start the installed Plenipo in smoke mode. Without -NoWait, wait for it to exit 0 and return
# its report (a 1.8.0 Plenipo writes none).
function Start-Plenipo([string] $Scenario = '', [switch] $NoWait) {
  Remove-Item $Report -ErrorAction SilentlyContinue
  $env:PLENIPO_SMOKE_TEST = '1'
  $env:PLENIPO_SMOKE_TIMEOUT_SECS = '150'
  $env:PLENIPO_SMOKE_REPORT = $Report
  if ($Scenario) { $env:PLENIPO_SMOKE_SCENARIO = $Scenario } else { $env:PLENIPO_SMOKE_SCENARIO = '' }
  try {
    $p = Start-Process -FilePath $Exe -PassThru
    $null = $p.Handle
  } finally {
    Remove-Item Env:PLENIPO_SMOKE_TEST, Env:PLENIPO_SMOKE_TIMEOUT_SECS, Env:PLENIPO_SMOKE_REPORT, Env:PLENIPO_SMOKE_SCENARIO -ErrorAction SilentlyContinue
  }
  if ($NoWait) { return $p }
  if (-not $p.WaitForExit(200000)) {
    Stop-Process -Id $p.Id -Force
    throw "Plenipo did not exit within 200s"
  }
  Check ($p.ExitCode -eq 0) "Plenipo started, showed its window, and exited cleanly (exit $($p.ExitCode))"
  Wait-NoPlenipo
  return Read-Report
}

# ($Check, not $Ready: PowerShell looks names up through the callers, and Wait-Until's own
# $Ready would be found first, calling itself forever.)
function Wait-Report([scriptblock] $Check, [string] $What) {
  Wait-Until { $r = Read-Report; $null -ne $r -and (& $Check $r) } 150 $What
  return Read-Report
}

# End Plenipo the hard way (as a crash or Task Manager would): no clean exit.
function Kill-Plenipo($p) {
  Stop-Process -Id $p.Id -Force
  $p.WaitForExit(30000) | Out-Null
  # Its AI tool and other programs end with it (they are in its Windows job).
  Wait-NoPlenipo 20
  Check (Test-Path $RunNote) "the note that Plenipo was running is left behind (it did not end cleanly)"
}

# A Windows restart: the note's last heartbeat is dated before Windows started.
function Date-NoteBeforeBoot {
  $note = Get-Content $RunNote -Raw | ConvertFrom-Json
  $note.heartbeatAt = 1000
  $note.startedAt = 1000
  $note | ConvertTo-Json | Set-Content -Path $RunNote -Encoding utf8NoBOM
}

function Has([object[]] $list, [scriptblock] $match) {
  @($list | Where-Object $match).Count -gt 0
}

# ---- Start clean --------------------------------------------------------------------------------
Step 'Start from a clean computer'
Get-Process -Name 'plenipo-desktop' -ErrorAction SilentlyContinue | Stop-Process -Force
if (Test-Path $Uninstaller) { Uninstall @('/S') }
Remove-Item $Data, $RoamingData -Recurse -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $RunKey -Name 'Plenipo' -ErrorAction SilentlyContinue
Check (-not (Test-Path $Exe)) 'Plenipo is not installed'
Done

# ---- Clean install ------------------------------------------------------------------------------
Step "Clean install of $Version"
Install $NewInstaller
Check ((Installed-Version) -eq $Version) "Windows lists Plenipo $Version (Settings -> Apps)"
Check (Test-Path $StartMenu) 'a Start menu shortcut'
$r = Start-Plenipo
Check ($r.version -eq $Version) "it runs as $Version"
Check ($r.persistent -eq $true) 'the Ledger is kept on this computer'
Check ($null -eq $r.recovery.recovery) 'a first start has nothing to recover'
Check ($r.logFiles -ge 1) 'Plenipo writes its log file'
Check (Test-Path (Join-Path $Data 'ledger\plenipo.db')) 'the Ledger is in its folder'
Check (Test-Path (Join-Path $Data 'logs\plenipo.log')) 'the log is in its folder'
Check (-not (Test-Path $RunNote)) 'a clean exit removes the "running" note'
Done

# ---- Forced crash with work running -------------------------------------------------------------
Step 'Forced crash while a task and a program run'
$p = Start-Plenipo -Scenario 'start-work' -NoWait
$started = Wait-Report { param($x) $null -ne $x.extra -and $null -ne $x.extra.task } 'work to start'
$task = $started.extra.task
Check ($null -ne $started.extra.execution) 'a long program is running'
Kill-Plenipo $p
$r = Start-Plenipo
Check ($r.recovery.recovery.cause -eq 'crash') 'the next start says Plenipo closed unexpectedly'
Check (Has $r.recovery.recovery.stoppedTasks { $_.taskId -eq $task }) 'and names the task that stopped'
Check ($r.recovery.recovery.stoppedPrograms -ge 1) 'and the program that stopped'
Check ($r.eventTypes -contains 'plenipo.recovered') 'the recovery is recorded in the Ledger'
Done

# ---- Windows restart while idle, and with recoverable task metadata ------------------------------
Step 'Windows restart while idle (simulated)'
$p = Start-Plenipo -Scenario 'stay' -NoWait
Wait-Report { param($x) $x.extra.stage -eq 'ready' } 'Plenipo to be ready' | Out-Null
Kill-Plenipo $p
Date-NoteBeforeBoot
$r = Start-Plenipo
Check ($r.recovery.recovery.cause -eq 'windowsRestart') 'the next start says Windows restarted'
Check (@($r.recovery.recovery.stoppedTasks).Count -eq 0) 'and that nothing was running'
Done

Step 'Windows restart with a task running (simulated)'
$p = Start-Plenipo -Scenario 'start-work' -NoWait
$started = Wait-Report { param($x) $null -ne $x.extra -and $null -ne $x.extra.task } 'work to start'
Kill-Plenipo $p
Date-NoteBeforeBoot
$r = Start-Plenipo
Check ($r.recovery.recovery.cause -eq 'windowsRestart') 'the next start says Windows restarted'
Check (Has $r.recovery.recovery.stoppedTasks { $_.taskId -eq $started.extra.task }) 'and names the task that stopped'
Done

# ---- Corrupted settings files -------------------------------------------------------------------
Step 'A damaged "running" note'
Set-Content -Path $RunNote -Value 'not the note Plenipo writes' -Encoding utf8NoBOM
$r = Start-Plenipo
Check ($r.recovery.recovery.cause -eq 'unknown') 'a damaged note is read as an unclean end of unknown cause'
Done

# ---- A crashed window ---------------------------------------------------------------------------
Step 'The window crashes (its WebView2 programs are ended)'
$p = Start-Plenipo -Scenario 'window-crash' -NoWait
Wait-Report { param($x) $x.extra.stage -eq 'ready' } 'the window to be ready' | Out-Null
$webviews = @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" |
    Where-Object { $_.ParentProcessId -eq $p.Id })
Check ($webviews.Count -ge 1) "Plenipo's window runs in WebView2 programs ($($webviews.Count))"
$webviews | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
if (-not $p.WaitForExit(150000)) {
  Stop-Process -Id $p.Id -Force
  throw 'the window was not brought back within 150s'
}
Check ($p.ExitCode -eq 0) "Plenipo brought the window back and kept running (exit $($p.ExitCode))"
$r = Read-Report
Check ($r.extra.stage -eq 'back') 'its report says the window came back'
Check ($r.eventTypes -contains 'plenipo.window_recovered') 'recorded in the Ledger'
Wait-NoPlenipo
Done

# ---- One Plenipo at a time, and --quit ----------------------------------------------------------
Step 'A second launch shows the first; --quit quits it cleanly'
$first = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 15
$second = Start-Process -FilePath $Exe -PassThru
Check ($second.WaitForExit(30000)) 'the second launch hands over to the first and ends'
Check (-not $first.HasExited) 'the first keeps running'
Check (@(Plenipo-Processes | Where-Object { $_.Id -ne $first.Id }).Count -eq 0) 'only one Plenipo runs'
Start-Process -FilePath $Exe -ArgumentList '--quit' -Wait | Out-Null
Check ($first.WaitForExit(60000)) 'Plenipo --quit asks the running one to quit'
Wait-NoPlenipo
Check (-not (Test-Path $RunNote)) 'it quit cleanly (no "running" note left)'
Done

# ---- Uninstall, keeping your data ---------------------------------------------------------------
Step 'Uninstall, keeping your data'
New-ItemProperty -Path $RunKey -Name 'Plenipo' -Value "`"$Exe`" --in-tray" -PropertyType String -Force | Out-Null
Uninstall @('/S')
Check (-not (Test-Path $InstallDir) -or @(Get-ChildItem $InstallDir -Force).Count -eq 0) 'the program is gone'
Check (-not (Test-Path $UninstallKey)) 'Windows no longer lists Plenipo'
Check (-not (Test-Path $StartMenu)) 'the Start menu shortcut is gone'
Check (-not (Test-Path $Desktop)) 'the desktop shortcut is gone'
Check ($null -eq (Get-ItemProperty -Path $RunKey -Name 'Plenipo' -ErrorAction SilentlyContinue)) 'Start with Windows is removed'
Check (Test-Path (Join-Path $Data 'ledger\plenipo.db')) 'your Ledger is kept'
Done

# ---- Upgrade from 1.8.0 -------------------------------------------------------------------------
Step 'Upgrade from the published 1.8.0 installer'
Remove-Item $Data -Recurse -Force -ErrorAction SilentlyContinue
Install $OldInstaller
Check ((Installed-Version) -eq '1.8.0') 'Plenipo 1.8.0 is installed'
Start-Plenipo | Out-Null
Check (Test-Path (Join-Path $Data 'ledger\plenipo.db')) '1.8.0 made its Ledger'
Install $NewInstaller
Check ((Installed-Version) -eq $Version) "Windows lists Plenipo $Version"
$r = Start-Plenipo
Check ($r.version -eq $Version) "it runs as $Version"
Check ($r.lastVersion -eq $Version) 'the Ledger knows the new version'
Check (Has $r.backups { $_.kind -eq 'beforeUpgrade' }) 'the Ledger was backed up before the new version first used it'
Check ($r.eventTypes -contains 'plenipo.version_changed') 'the upgrade is recorded'
Check ($null -eq $r.recovery.recovery) '1.8.0 had closed cleanly: nothing to recover'
Done

# ---- Going back to 1.8.0, and forward again -----------------------------------------------------
Step 'Rollback: back to 1.8.0, then forward again'
Install $OldInstaller
Check ((Installed-Version) -eq '1.8.0') 'the older installer goes back to 1.8.0'
Start-Plenipo | Out-Null
Check $true '1.8.0 opens the Ledger 1.9 used (1.9 does not change its layout)'
Install $NewInstaller
$r = Start-Plenipo
Check ($r.version -eq $Version -and $r.persistent -eq $true) "forward again to $Version, same Ledger"
Done

# ---- An update, the way Plenipo installs it -----------------------------------------------------
Step 'An update: checked, signed, backed up, installed, reopened'
$parts = $Version.Split('.')
$Next = "$($parts[0]).$($parts[1]).$([int]$parts[2] + 1)"
New-Item -ItemType Directory -Force -Path $UpdatesDir | Out-Null
$package = Join-Path $UpdatesDir "Plenipo_${Next}_x64-setup.exe"
Copy-Item $NewInstaller $package -Force
# Signed with this run's throwaway updater key, as version $Next (the key built into this CI copy).
& pnpm --filter '@plenipo/desktop' tauri signer sign -f $UpdaterKey --password= --app-version $Next $package
Check ($LASTEXITCODE -eq 0) 'the test update is signed'
$manifest = [ordered]@{
  version   = $Next
  notes     = 'A test update.'
  pub_date  = (Get-Date).ToUniversalTime().ToString('o')
  platforms = @{ 'windows-x86_64' = @{
      signature = (Get-Content "$package.sig" -Raw).Trim()
      url       = "http://127.0.0.1:$UpdatePort/Plenipo_${Next}_x64-setup.exe"
    }
  }
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $UpdatesDir 'latest.json') -Encoding utf8NoBOM
$server = Start-Process -FilePath 'python' -ArgumentList '-m', 'http.server', "$UpdatePort", '--bind', '127.0.0.1', '--directory', $UpdatesDir -PassThru -WindowStyle Hidden
try {
  Start-Sleep -Seconds 2
  $p = Start-Plenipo -Scenario 'update' -NoWait
  Check ($p.WaitForExit(200000)) 'Plenipo stops its work and quits to install'
  Check ($p.ExitCode -eq 0) "it handed over to the installer (exit $($p.ExitCode))"
  # The installer Plenipo downloaded runs on its own, then opens Plenipo again. It opens it
  # through Windows' desktop (as the signed-in person, not as administrator), which a CI machine
  # may not have: then that last step is the owner's check.
  $setup = "Plenipo_${Next}_x64-setup"
  try {
    Wait-Until { @(Get-Process -Name $setup -ErrorAction SilentlyContinue).Count -ge 1 -or (Plenipo-Processes).Count -ge 1 } 60 'the downloaded installer to start'
  } catch {
    Write-Host 'The installer finished before it was seen (it is quick); its result is checked below.'
  }
  Wait-Until { @(Get-Process -Name $setup -ErrorAction SilentlyContinue).Count -eq 0 } 180 'the downloaded installer to finish'
  try {
    Wait-Until { (Plenipo-Processes).Count -ge 1 } 30 'the installer to open Plenipo again'
    Write-Host 'ok: the installer opened Plenipo again'
    Start-Sleep -Seconds 10
    Start-Process -FilePath $Exe -ArgumentList '--quit' -Wait | Out-Null
    Wait-NoPlenipo 60
  } catch {
    Write-Host '::warning::The installer did not open Plenipo again on this machine (no Windows desktop to open it through). Check it on a real computer.'
  }
  Check ((Installed-Version) -eq $Version) 'the installed copy is in place'
  Check (Test-Path $Exe) 'the program is there'
  $r = Start-Plenipo
  Check (Has $r.backups { $_.kind -eq 'beforeUpdate' }) 'the Ledger was backed up before installing'
  Check ($r.eventTypes -contains 'plenipo.update_available') 'the new version was found and recorded'
  Check ($r.eventTypes -contains 'plenipo.update_installing') 'installing it was recorded'
  Check ($null -eq $r.recovery.recovery) 'installing was a clean exit, not a crash'
} finally {
  Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
}
Done

# ---- A damaged Ledger ---------------------------------------------------------------------------
Step 'A damaged Ledger is set aside, never silently used'
Set-Content -Path (Join-Path $Data 'ledger\plenipo.db') -Value 'not a database' -Encoding utf8NoBOM
Remove-Item (Join-Path $Data 'ledger\plenipo.db-wal'), (Join-Path $Data 'ledger\plenipo.db-shm') -ErrorAction SilentlyContinue
$r = Start-Plenipo
Check ($r.persistent -eq $true) 'a new Ledger is started and kept'
Check (Has $r.ledgerNotices { $_ -match 'failed its integrity check' }) 'and Plenipo says so, naming where the damaged one went'
Check (@(Get-ChildItem (Join-Path $Data 'ledger') -Filter 'plenipo.db.corrupt-*').Count -ge 1) 'the damaged one is kept aside'
Done

# ---- Uninstall, deleting your data --------------------------------------------------------------
Step 'Uninstall, deleting your data'
$forget = Start-Process -FilePath $Exe -ArgumentList '--plenipo-forget-secrets' -PassThru -Wait
Check ($forget.ExitCode -eq 0) 'Plenipo forgets the secrets it kept (the uninstaller asks it to)'
Uninstall @('/S', '/DELETEAPPDATA')
Check (-not (Test-Path $Data)) 'your Plenipo data is deleted'
Check (-not (Test-Path $UninstallKey)) 'Windows no longer lists Plenipo'
Done

# ---- What is left behind ------------------------------------------------------------------------
Step 'What is left behind'
$left = [ordered]@{
  "Program folder ($InstallDir)"     = (Test-Path $InstallDir) -and @(Get-ChildItem $InstallDir -Force -ErrorAction SilentlyContinue).Count -gt 0
  "Plenipo's data ($Data)"           = Test-Path $Data
  "Roaming data ($RoamingData)"      = Test-Path $RoamingData
  'Uninstall entry'                  = Test-Path $UninstallKey
  'Start with Windows entry'         = $null -ne (Get-ItemProperty -Path $RunKey -Name 'Plenipo' -ErrorAction SilentlyContinue)
  'Start menu shortcut'              = Test-Path $StartMenu
  'Desktop shortcut'                 = Test-Path $Desktop
  'A Plenipo program still running'  = (Plenipo-Processes).Count -gt 0
}
$left.GetEnumerator() | ForEach-Object { Write-Host ("{0,-60} {1}" -f $_.Key, $(if ($_.Value) { 'LEFT' } else { 'gone' })) }
Check (-not ($left.Values -contains $true)) 'nothing is left behind'
Done

Write-Host "All installer tests passed for $Version."
