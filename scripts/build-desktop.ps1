# Build the MutiS desktop app:
#   dist\MutiS\                      portable folder ("MutiS.exe" window + ai-hub.exe server)
#   dist\MutiS_<version>_x64-setup.exe   installer (NSIS, installs for the current user, no admin needed)
# Steps: export the React UI (embedded into the server), build the server, then `tauri build` (window + installer).
# Update package: when the signing key exists (TAURI_SIGNING_PRIVATE_KEY, else %USERPROFILE%\.tauri\mutis-updater.key) the installer is
# signed and dist\latest.json is written; upload the installer, its .sig and latest.json to a GitHub Release tagged v<version>.
# Without a key the build still works, it just makes no update package.
# First run needs network: npm packages for the Tauri CLI, Rust crates, and Tauri downloads NSIS once.
param([ValidateSet('build', 'check', 'publish')][string]$Action = 'build')

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$repo = Split-Path -Parent $PSScriptRoot
$desktop = Join-Path $repo 'desktop'
$dist = Join-Path $repo 'dist'
$out = Join-Path $dist 'MutiS'
$version = (Get-Content (Join-Path $desktop 'tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json).version

function Invoke-Step([string]$Name, [string]$Dir, [scriptblock]$Action) {
  Write-Host "== $Name"
  Push-Location $Dir
  try { & $Action; if ($LASTEXITCODE -ne 0) { throw "$Name failed (exit $LASTEXITCODE)" } }
  finally { Pop-Location }
}

$releaseRepo = 'HCodi24/AI_Hub'
if ($Action -ne 'build') {
  $installer = "MutiS_${version}_x64-setup.exe"
  $assets = @($installer, "$installer.sig", 'latest.json')
  foreach ($asset in $assets) {
    $file = Get-Item -LiteralPath (Join-Path $dist $asset) -ErrorAction SilentlyContinue
    if (-not $file -or $file.PSIsContainer -or $file.Length -eq 0) { throw "Thiếu tệp hoặc tệp rỗng: $asset" }
    Write-Host "$asset ($($file.Length) byte)"
  }
  $manifest = Get-Content -LiteralPath (Join-Path $dist 'latest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
  $platform = $manifest.platforms.'windows-x86_64'
  $signature = (Get-Content -LiteralPath (Join-Path $dist "$installer.sig") -Raw).Trim()
  $url = "https://github.com/$releaseRepo/releases/download/v$version/$installer"
  if ($manifest.version -cne $version -or $platform.url -cne $url -or -not $signature -or $platform.signature -cne $signature) {
    throw "Thông tin cập nhật không khớp với bộ cài v$version."
  }
  if ($manifest.installer_sha256 -and (Get-FileHash -LiteralPath (Join-Path $dist $installer)).Hash -ne $manifest.installer_sha256) {
    throw "Mã SHA-256 của bộ cài v$version không khớp."
  }
  if ($Action -eq 'publish') {
    $sourceCommit = [string]$manifest.source_commit
    if ($sourceCommit -cnotmatch '^[0-9a-f]{40}$') {
      throw 'Hãy commit và push mã nguồn, sau đó build lại để ghi nhận commit của bộ cài.'
    }
    $ghPath = (Get-Command gh.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    if (-not $ghPath) {
      $ghPath = Join-Path $env:ProgramFiles 'GitHub CLI\gh.exe'
      if (-not (Test-Path -LiteralPath $ghPath -PathType Leaf)) {
        throw 'Không tìm thấy GitHub CLI. Hãy cài GitHub CLI rồi thử đẩy lại.'
      }
    }
    Invoke-Step "Kiểm tra commit trên GitHub" $repo {
      $remoteCommit = & $ghPath api "repos/$releaseRepo/commits/$sourceCommit" --jq '.sha'
      if ($LASTEXITCODE -eq 0 -and $remoteCommit -cne $sourceCommit) { throw 'Không tìm thấy commit của bộ cài trên GitHub.' }
    }
    $assetPaths = $assets | ForEach-Object { Join-Path $dist $_ }
    Invoke-Step "Phát hành v$version lên GitHub" $repo {
      & $ghPath release create "v$version" --repo $releaseRepo --target $sourceCommit --generate-notes @assetPaths
    }
  }
  return
}

$key = Join-Path $env:USERPROFILE '.tauri\mutis-updater.key'
if (-not $env:TAURI_SIGNING_PRIVATE_KEY -and (Test-Path $key)) { $env:TAURI_SIGNING_PRIVATE_KEY = $key }
$signing = [bool]$env:TAURI_SIGNING_PRIVATE_KEY

# Version files and lockfiles are updated by the desktop build itself.
$sourceCommit = ''
$changes = git -C $repo status --porcelain -- . ':(exclude)Cargo.toml' ':(exclude)Cargo.lock' ':(exclude)desktop/Cargo.toml' ':(exclude)desktop/Cargo.lock' ':(exclude)desktop/tauri.conf.json'
if ($LASTEXITCODE -eq 0 -and -not $changes) {
  $sourceCommit = git -C $repo rev-parse HEAD
}

Invoke-Step 'Giao diện (npm run build)' (Join-Path $repo 'web') { npm run build }
# The installer bundles target\release\ai-hub.exe, so the server is always built into the default target folder.
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
Invoke-Step 'Server (cargo build --release)' $repo { cargo build --release }
if (-not (Test-Path (Join-Path $desktop 'node_modules\@tauri-apps\cli'))) {
  Invoke-Step 'Tauri CLI (npm install)' $desktop { npm install --no-audit --no-fund }
}
$tauriArgs = @('tauri', 'build')
if (-not $signing) { Write-Warning 'Không có khoá ký cập nhật: bỏ qua gói cập nhật.'; $off = Join-Path $env:TEMP 'mutis-no-updater.json'; Set-Content $off '{"bundle":{"createUpdaterArtifacts":false}}' -Encoding ASCII; $tauriArgs += @('--config', $off) }
Invoke-Step 'Cửa sổ desktop + bộ cài (tauri build)' $desktop { npx @tauriArgs }

$server = Join-Path $repo 'target\release\ai-hub.exe'
$window = Join-Path $desktop 'target\release\MutiS.exe'
$setup = Get-ChildItem (Join-Path $desktop 'target\release\bundle\nsis') -Filter "*_${version}_*-setup.exe" |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
foreach ($file in @($server, $window)) { if (-not (Test-Path $file)) { throw "Missing build output: $file" } }
if (-not $setup) { throw 'Missing installer in desktop\target\release\bundle\nsis' }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item $server (Join-Path $out 'ai-hub.exe') -Force
Copy-Item $window (Join-Path $out 'MutiS.exe') -Force
Copy-Item $setup.FullName (Join-Path $dist $setup.Name) -Force
$sig = "$($setup.FullName).sig"
if ($signing -and (Test-Path $sig)) {
  $latest = [ordered]@{
    version = $version
    source_commit = [string]$sourceCommit
    installer_sha256 = (Get-FileHash -LiteralPath $setup.FullName).Hash
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
