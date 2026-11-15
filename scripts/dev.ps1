[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = 'Continue'

# Repo layout: this script lives in <repo>\scripts, so the repo root is its parent.
$repo = Split-Path -Parent $PSScriptRoot
$webDir = Join-Path $repo 'web'
$manifest = Join-Path $repo 'Cargo.toml'
$targetDir = Join-Path $repo 'target'
$builtExe = Join-Path $targetDir 'debug\ai-hub.exe'
$exe = Join-Path $targetDir 'dev-run\ai-hub.exe'

# -Restart: stop an AI Hub dev instance that is already running from this repo, then start fresh.
$restart = $args -contains '-Restart'
$appArgs = @($args | Where-Object { $_ -ne '-Restart' })

function Write-Step { param([string]$Message) Write-Host $Message -ForegroundColor Cyan }
function Write-Ok { param([string]$Message) Write-Host $Message -ForegroundColor Green }
function Write-Warn { param([string]$Message) Write-Host $Message -ForegroundColor Yellow }
function Write-Fail { param([string]$Message) Write-Host $Message -ForegroundColor Red }

# Build the frontend (web/src -> web/out) on every invocation, from the absolute web directory.
function Invoke-FrontendBuild {
  if (-not (Test-Path -LiteralPath $webDir -PathType Container)) {
    Write-Fail "Không tìm thấy thư mục frontend: $webDir"
    return 1
  }
  $npm = Get-Command 'npm.cmd' -ErrorAction SilentlyContinue
  if ($null -eq $npm) {
    Write-Fail 'Không tìm thấy npm.cmd trong PATH. Hãy cài Node.js rồi thử lại.'
    return 1
  }
  $npmPath = $npm.Source
  Write-Step 'Đang build giao diện (npm.cmd run build)…'
  $code = 1
  $pushed = $false
  try {
    Push-Location -LiteralPath $webDir -ErrorAction Stop
    $pushed = $true
    & $npmPath run build | Out-Host
    $code = $LASTEXITCODE
  } catch {
    Write-Fail "Lỗi khi chạy npm.cmd run build: $($_.Exception.Message)"
    return 1
  } finally {
    if ($pushed) { Pop-Location -ErrorAction Stop }
  }
  if ($code -ne 0) {
    Write-Fail "Build giao diện thất bại (exit code $code)."
    return 1
  }
  Write-Ok 'Build giao diện hoàn tất.'
  return 0
}

# Build Rust with an explicit target directory so the copied binary always matches the build output.
function Invoke-RustBuild {
  if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
    Write-Fail "Không tìm thấy Cargo.toml: $manifest"
    return 1
  }
  $cargo = Get-Command 'cargo' -ErrorAction SilentlyContinue
  if ($null -eq $cargo) {
    Write-Fail 'Không tìm thấy cargo trong PATH. Hãy cài Rust toolchain rồi thử lại.'
    return 1
  }
  $cargoPath = $cargo.Source
  Write-Step 'Đang build Rust (cargo build)…'
  $code = 1
  $pushed = $false
  try {
    Push-Location -LiteralPath $repo -ErrorAction Stop
    $pushed = $true
    & $cargoPath build --manifest-path $manifest --target-dir $targetDir | Out-Host
    $code = $LASTEXITCODE
  } catch {
    Write-Fail "Lỗi khi chạy cargo build: $($_.Exception.Message)"
    return 1
  } finally {
    if ($pushed) { Pop-Location -ErrorAction Stop }
  }
  if ($code -ne 0) {
    Write-Fail "Build Rust thất bại (exit code $code)."
    return 1
  }
  if (-not (Test-Path -LiteralPath $builtExe -PathType Leaf)) {
    Write-Fail "Không tìm thấy file build: $builtExe"
    return 1
  }
  Write-Ok 'Build Rust hoàn tất.'
  return 0
}

# Find dev instances started from this repo's dev-run executable (exact path match).
function Get-DevProcesses {
  try {
    # Leading comma keeps an empty result as an array instead of collapsing to $null.
    return ,@(Get-CimInstance Win32_Process -Filter "Name='ai-hub.exe'" -ErrorAction Stop |
      Where-Object { $_.ExecutablePath -eq $exe })
  } catch {
    Write-Fail "Không đọc được danh sách tiến trình: $($_.Exception.Message)"
    return $null
  }
}

# Copy the freshly built binary over the dev-run executable and start it.
function Start-DevInstance {
  try {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $exe) -ErrorAction Stop | Out-Null
    Copy-Item -LiteralPath $builtExe -Destination $exe -Force -ErrorAction Stop
  } catch {
    Write-Fail "Không thể cập nhật $exe : $($_.Exception.Message)"
    return 1
  }
  if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    Write-Fail "Không tìm thấy file chạy dev: $exe"
    return 1
  }
  try {
    # Pipe the child's stdout to the host so it never pollutes this function's output stream:
    # the caller captures the return value as the exit code only.
    & $exe --dev @appArgs | Out-Host
    return $LASTEXITCODE
  } catch {
    Write-Fail "Không thể chạy $exe : $($_.Exception.Message)"
    return 1
  }
}

$running = Get-DevProcesses
if ($null -eq $running) { exit 1 }

# Build both parts on every invocation, even when a dev instance is already running.
$frontendExit = Invoke-FrontendBuild
if ($frontendExit -ne 0) {
  Write-Fail 'Dừng lại: build giao diện thất bại, không build Rust và không chạy binary cũ.'
  exit 1
}

$rustExit = Invoke-RustBuild
if ($rustExit -ne 0) {
  Write-Fail 'Dừng lại: build Rust thất bại, không copy và không chạy binary cũ.'
  exit 1
}

if ($running.Count -gt 0) {
  if (-not $restart) {
    $ids = ($running | ForEach-Object { $_.ProcessId }) -join ', '
    Write-Ok 'Build giao diện đã hoàn tất.'
    Write-Warn "AI Hub dev đang chạy sẵn (PID $ids) — không tắt, không ghi đè, không mở thêm bản sao."
    Write-Host 'Mở trình duyệt: http://127.0.0.1:7878 (hoặc cổng bạn đã chọn).'
    Write-Host 'Thay đổi giao diện: tải lại trang trên trình duyệt.'
    Write-Host 'Áp dụng thay đổi Rust: dùng hành động "Build & khởi động lại" trong ứng dụng, hoặc chạy lại script với -Restart.'
    exit 0
  }
  try {
    $running | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction Stop }
    Start-Sleep -Milliseconds 500
    Write-Warn 'Đã tắt bản AI Hub dev cũ.'
  } catch {
    Write-Fail "Không thể tắt bản AI Hub dev cũ: $($_.Exception.Message)"
    exit 1
  }
}

$runExit = Start-DevInstance
if ($runExit -eq 75) {
  # Child requested a rebuild: loop back and rebuild both parts before relaunching.
  while ($true) {
    Write-Step 'Ứng dụng yêu cầu build lại, đang build và khởi động lại…'
    $frontendExit = Invoke-FrontendBuild
    if ($frontendExit -ne 0) {
      Write-Fail 'Dừng lại: build giao diện thất bại, không khởi động lại.'
      exit 1
    }
    $rustExit = Invoke-RustBuild
    if ($rustExit -ne 0) {
      Write-Fail 'Dừng lại: build Rust thất bại, không khởi động lại.'
      exit 1
    }
    $runExit = Start-DevInstance
    if ($runExit -ne 75) { break }
  }
}

exit $runExit
