# Build the MutiS desktop app:
#   dist\MutiS\                      portable folder ("MutiS.exe" window + ai-hub.exe server)
#   dist\MutiS_<version>_x64-setup.exe   installer (NSIS, installs for the current user, no admin needed)
# Steps: export the React UI (embedded into the server), build the server, then `tauri build` (window + installer).
# First run needs network: npm packages for the Tauri CLI, Rust crates, and Tauri downloads NSIS once.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$repo = Split-Path -Parent $PSScriptRoot
$desktop = Join-Path $repo 'desktop'
$dist = Join-Path $repo 'dist'
$out = Join-Path $dist 'MutiS'

function Invoke-Step([string]$Name, [string]$Dir, [scriptblock]$Action) {
  Write-Host "== $Name"
  Push-Location $Dir
  try { & $Action; if ($LASTEXITCODE -ne 0) { throw "$Name failed (exit $LASTEXITCODE)" } }
  finally { Pop-Location }
}

Invoke-Step 'Giao diện (npm run build)' (Join-Path $repo 'web') { npm run build }
# The installer bundles target\release\ai-hub.exe, so the server is always built into the default target folder.
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
Invoke-Step 'Server (cargo build --release)' $repo { cargo build --release }
if (-not (Test-Path (Join-Path $desktop 'node_modules\@tauri-apps\cli'))) {
  Invoke-Step 'Tauri CLI (npm install)' $desktop { npm install --no-audit --no-fund }
}
Invoke-Step 'Cửa sổ desktop + bộ cài (tauri build)' $desktop { npx tauri build }

$server = Join-Path $repo 'target\release\ai-hub.exe'
$window = Join-Path $desktop 'target\release\MutiS.exe'
$setup = Get-ChildItem (Join-Path $desktop 'target\release\bundle\nsis') -Filter '*-setup.exe' |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
foreach ($file in @($server, $window)) { if (-not (Test-Path $file)) { throw "Missing build output: $file" } }
if (-not $setup) { throw 'Missing installer in desktop\target\release\bundle\nsis' }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item $server (Join-Path $out 'ai-hub.exe') -Force
Copy-Item $window (Join-Path $out 'MutiS.exe') -Force
Copy-Item $setup.FullName (Join-Path $dist $setup.Name) -Force
Write-Host "Xong: $dist"
Get-ChildItem $out, (Join-Path $dist $setup.Name) | ForEach-Object { '{0,-32} {1,8:N1} MB' -f $_.Name, ($_.Length / 1MB) }
