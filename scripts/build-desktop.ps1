# Build the MutiS desktop app:
#   dist\MutiS\                      portable folder ("MutiS.exe" window + ai-hub.exe server)
#   dist\MutiS_<version>_x64-setup.exe   installer (NSIS, installs for the current user, no admin needed)
# Steps: export the React UI (embedded into the server), build the server, then `tauri build` (window + installer).
# Update package: when the signing key exists (TAURI_SIGNING_PRIVATE_KEY, else %USERPROFILE%\.tauri\mutis-updater.key) the installer is
# signed and dist\latest.json is written; upload the installer, its .sig and latest.json to a GitHub Release tagged v<version>.
# Without a key the build still works, it just makes no update package.
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
$key = Join-Path $env:USERPROFILE '.tauri\mutis-updater.key'
if (-not $env:TAURI_SIGNING_PRIVATE_KEY -and (Test-Path $key)) { $env:TAURI_SIGNING_PRIVATE_KEY = $key }
$signing = [bool]$env:TAURI_SIGNING_PRIVATE_KEY
$tauriArgs = @('tauri', 'build')
if (-not $signing) { Write-Warning 'Không có khoá ký cập nhật: bỏ qua gói cập nhật.'; $off = Join-Path $env:TEMP 'mutis-no-updater.json'; Set-Content $off '{"bundle":{"createUpdaterArtifacts":false}}' -Encoding ASCII; $tauriArgs += @('--config', $off) }
Invoke-Step 'Cửa sổ desktop + bộ cài (tauri build)' $desktop { npx @tauriArgs }

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
$sig = "$($setup.FullName).sig"
if ($signing -and (Test-Path $sig)) {
  $version = (Get-Content (Join-Path $desktop 'tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json).version
  $latest = [ordered]@{
    version = $version
    notes = [string]$env:MUTIS_RELEASE_NOTES
    pub_date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = @{ 'windows-x86_64' = [ordered]@{
      signature = (Get-Content $sig -Raw).Trim()
      url = "https://github.com/HCodi24/AI_Hub/releases/download/v$version/$([uri]::EscapeDataString($setup.Name))"
    } }
  }
  [IO.File]::WriteAllText((Join-Path $dist 'latest.json'), ($latest | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
  Copy-Item $sig (Join-Path $dist "$($setup.Name).sig") -Force
  Write-Host "Gói cập nhật: $dist\latest.json + $($setup.Name).sig (tải cả ba lên Release v$version)"
}
Write-Host "Xong: $dist"
Get-ChildItem $out, (Join-Path $dist $setup.Name) | ForEach-Object { '{0,-32} {1,8:N1} MB' -f $_.Name, ($_.Length / 1MB) }
