[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = 'Continue'
$repo = $PSScriptRoot
$exe = Join-Path $repo 'target\dev-run\ai-hub.exe'
New-Item -ItemType Directory -Force -Path (Split-Path $exe) | Out-Null
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
  & $exe --dev @args
  $runExit = $LASTEXITCODE
  if ($runExit -ne 75) { break }
  Write-Host 'Đang build và khởi động lại…' -ForegroundColor Cyan
}