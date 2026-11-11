# Build the AI Hub desktop app into dist\AI Hub\ (portable folder: "AI Hub.exe" + ai-hub.exe).
# Steps: export the React UI (embedded into the server), build the server, build the Tauri window, copy both.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$repo = Split-Path -Parent $PSScriptRoot
$out = Join-Path $repo 'dist\AI Hub'

function Invoke-Step([string]$Name, [string]$Dir, [scriptblock]$Action) {
  Write-Host "== $Name"
  Push-Location $Dir
  try { & $Action; if ($LASTEXITCODE -ne 0) { throw "$Name failed (exit $LASTEXITCODE)" } }
  finally { Pop-Location }
}

Invoke-Step 'Giao diện (npm run build)' (Join-Path $repo 'web') { npm run build }
Invoke-Step 'Server (cargo build --release)' $repo { cargo build --release }
Invoke-Step 'Cửa sổ desktop (cargo build --release)' (Join-Path $repo 'desktop') { cargo build --release }

$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }
$server = Join-Path $target 'release\ai-hub.exe'
$window = Join-Path $repo 'desktop\target\release\ai-hub-desktop.exe'
foreach ($file in @($server, $window)) { if (-not (Test-Path $file)) { throw "Missing build output: $file" } }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item $server (Join-Path $out 'ai-hub.exe') -Force
Copy-Item $window (Join-Path $out 'AI Hub.exe') -Force
Write-Host "Xong: $out"
Get-ChildItem $out | ForEach-Object { '{0,-14} {1,8:N1} MB' -f $_.Name, ($_.Length / 1MB) }
