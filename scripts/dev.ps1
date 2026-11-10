[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = 'Continue'
$repo = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $repo 'target\dev-run\ai-hub.exe'
New-Item -ItemType Directory -Force -Path (Split-Path $exe) | Out-Null

# -Restart: stop an AI Hub dev instance that is already running from this repo, then start fresh.
$restart = $args -contains '-Restart'
$appArgs = @($args | Where-Object { $_ -ne '-Restart' })
$running = @(Get-CimInstance Win32_Process -Filter "Name='ai-hub.exe'" |
  Where-Object { $_.ExecutablePath -eq $exe })
if ($running.Count -gt 0) {
  if ($restart) {
    $running | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
    Start-Sleep -Milliseconds 500
    Write-Host 'Đã tắt bản AI Hub dev cũ.' -ForegroundColor Yellow
  } else {
    $ids = ($running | ForEach-Object { $_.ProcessId }) -join ', '
    Write-Host "AI Hub dev đang chạy sẵn (PID $ids)." -ForegroundColor Yellow
    Write-Host 'Mở trình duyệt: http://127.0.0.1:7878 (hoặc cổng bạn đã chọn).'
    Write-Host 'Muốn chạy lại từ đầu: .\scripts\dev.ps1 -Restart'
    exit 0
  }
}

while ($true) {
  & cargo build --manifest-path (Join-Path $repo 'Cargo.toml')
  $buildExit = $LASTEXITCODE
  if ($buildExit -ne 0) {
    Write-Host 'Build thất bại.' -ForegroundColor Red
    if (-not (Test-Path $exe)) { break }
    Write-Host 'Tiếp tục chạy bản dev-run cũ.' -ForegroundColor Yellow
  } else {
    Copy-Item -LiteralPath (Join-Path $repo 'target\debug\ai-hub.exe') -Destination $exe -Force
  }
  & $exe --dev @appArgs
  $runExit = $LASTEXITCODE
  if ($runExit -ne 75) { break }
  Write-Host 'Đang build và khởi động lại…' -ForegroundColor Cyan
}