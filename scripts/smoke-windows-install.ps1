# ABOUTME: Installs a built Rumble .msi or NSIS setup.exe silently, finds the installed sidecar and
# ABOUTME: app, and checks the installed sidecar starts and answers /health (#116).
param([Parameter(Mandatory)][string]$Installer)
$ErrorActionPreference = 'Stop'

$SmokePort = if ($env:RUMBLE_SMOKE_PORT) { [int]$env:RUMBLE_SMOKE_PORT } else { 18765 }
$HealthWaitSeconds = if ($env:RUMBLE_SMOKE_HEALTH_WAIT_SECONDS) { [int]$env:RUMBLE_SMOKE_HEALTH_WAIT_SECONDS } else { 90 }
# msiexec's success codes: done, and done but a restart is pending.
$MsiSuccess = @(0, 3010)
# Where per-machine (MSI) and per-user (NSIS) installs land; searched, not assumed.
$InstallRoots = @($env:ProgramFiles, $env:LOCALAPPDATA)

$before = @(Get-ChildItem -Path $InstallRoots -Filter rumble-sidecar.exe -Recurse -Depth 3 -ErrorAction SilentlyContinue | ForEach-Object FullName)

if ($Installer -like '*.msi') {
  $log = Join-Path $env:RUNNER_TEMP 'rumble-msi.log'
  $install = Start-Process msiexec.exe -ArgumentList '/i', "`"$Installer`"", '/qn', '/l*v', "`"$log`"" -Wait -PassThru
  if ($MsiSuccess -notcontains $install.ExitCode) { Get-Content $log -Tail 40; throw "msiexec exited with $($install.ExitCode)" }
} else {
  $install = Start-Process $Installer -ArgumentList '/S' -Wait -PassThru
  if ($install.ExitCode -ne 0) { throw "$Installer exited with $($install.ExitCode)" }
}

$sidecar = Get-ChildItem -Path $InstallRoots -Filter rumble-sidecar.exe -Recurse -Depth 3 -ErrorAction SilentlyContinue |
  Where-Object { $before -notcontains $_.FullName } | Select-Object -First 1
if (-not $sidecar) { throw "no rumble-sidecar.exe appeared under $($InstallRoots -join ', ') after installing $Installer" }
$appDir = $sidecar.DirectoryName
$app = Get-ChildItem -Path $appDir -Filter *.exe | Where-Object Name -ne 'rumble-sidecar.exe' | Where-Object Name -notlike 'uninstall*'
if (-not $app) { throw "no app executable beside $($sidecar.FullName)" }
Write-Output "installed to $appDir : $(($app | ForEach-Object Name) -join ', '), rumble-sidecar.exe"

$dataDir = Join-Path $env:RUNNER_TEMP ("rumble-smoke-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $dataDir | Out-Null
$secret = [guid]::NewGuid().ToString()
$env:RUMBLE_SIDECAR_SECRET = $secret
$env:RUMBLE_DB_KEY = -join ((1..32) | ForEach-Object { '{0:x2}' -f (Get-Random -Maximum 256) })
$out = Join-Path $dataDir 'sidecar.out'
$err = Join-Path $dataDir 'sidecar.err'
$process = Start-Process $sidecar.FullName -ArgumentList '--port', $SmokePort, '--data-dir', "`"$dataDir`"" `
  -RedirectStandardOutput $out -RedirectStandardError $err -PassThru
# Reading the handle now keeps ExitCode available once the process has gone.
$null = $process.Handle
$lastError = 'no answer yet'
try {
  for ($i = 0; $i -lt $HealthWaitSeconds; $i++) {
    try {
      $health = Invoke-RestMethod -Uri "http://127.0.0.1:$SmokePort/health" -Headers @{ 'X-Rumble-Secret' = $secret } -TimeoutSec 2
      Write-Output "installed sidecar answered /health: $($health | ConvertTo-Json -Compress)"
      return
    } catch { $lastError = $_.Exception.Message }
    if ($process.HasExited) {
      Get-Content $out, $err -ErrorAction SilentlyContinue
      throw "installed sidecar exited with code $($process.ExitCode) before /health answered"
    }
    Start-Sleep -Seconds 1
  }
  Get-Content $out, $err -ErrorAction SilentlyContinue
  throw "installed sidecar did not answer /health within ${HealthWaitSeconds}s (last error: $lastError)"
} finally {
  # The PyInstaller bootloader and its Python child: stop the whole tree.
  if (-not $process.HasExited) { & taskkill.exe /PID $process.Id /T /F | Out-Null }
  # taskkill fails if the sidecar exited on its own meanwhile; that must not fail the step.
  $global:LASTEXITCODE = 0
}
