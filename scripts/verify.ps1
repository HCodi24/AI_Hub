[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
$targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target\verify' }
if (-not [IO.Path]::IsPathRooted($targetRoot)) { $targetRoot = Join-Path $repo $targetRoot }
$logDir = Join-Path $targetRoot 'logs'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$failures = [System.Collections.Generic.List[string]]::new()

function Invoke-LoggedCheck([string]$Name, [scriptblock]$Action, [bool]$ReportOnly = $false) {
  $log = Join-Path $logDir "$Name.txt"
  $previousPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = 'Continue'
    $global:LASTEXITCODE = 0
    $output = & $Action 2>&1
    [IO.File]::WriteAllText($log, (($output | ForEach-Object { "$_" }) -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
    $code = $LASTEXITCODE
    if ($null -eq $code) { $code = 0 }
    if ($code -ne 0) { throw "exit $code" }
    if ($ReportOnly) { Write-Host "⚠ $Name (report-only; xem log: $log)" }
    else { Write-Host "✓ $Name" }
    return $true
  } catch {
    if ($ReportOnly) { Write-Host "⚠ $Name (xem log: $log)"; return $false }
    Write-Host "✗ $Name (xem log: $log)"
    $failures.Add($Name)
    return $false
  } finally {
    $ErrorActionPreference = $previousPreference
  }
}

$env:CARGO_TARGET_DIR = $targetRoot
$null = Invoke-LoggedCheck 'fmt' { cargo fmt --check }
$null = Invoke-LoggedCheck 'clippy' { cargo clippy --offline --all-targets -- -D warnings }
$null = Invoke-LoggedCheck 'test' { cargo test --offline }
$null = Invoke-LoggedCheck 'node' {
  $files = @(Get-ChildItem -LiteralPath (Join-Path $repo 'ui') -Filter '*.js' -File -Recurse)
  foreach ($file in $files) { node --check $file.FullName; if ($LASTEXITCODE -ne 0) { throw "node --check failed: $($file.FullName)" } }
}
$null = Invoke-LoggedCheck 'plan-order' { node scripts/test-plan-order.mjs }
$null = Invoke-LoggedCheck 'plan-run' { node scripts/test-plan-run.mjs }
$null = Invoke-LoggedCheck 'encoding' {
  $bad = [System.Collections.Generic.List[string]]::new()
  foreach ($folder in @('src','ui','scripts')) {
    foreach ($file in Get-ChildItem (Join-Path $repo $folder) -File -Recurse) {
      if ($file.Extension -notin @('.js','.css','.html','.rs','.ps1','.cmd','.json','.toml')) { continue }
      $bytes = [IO.File]::ReadAllBytes($file.FullName)
      try { $text = [Text.UTF8Encoding]::new($false, $true).GetString($bytes) } catch { $bad.Add("invalid UTF-8: $($file.FullName)"); continue }
      $markers = @([string][char]0xFFFD, ([string][char]0x00C3), ([string][char]0x00E1 + [char]0x00BB), ([string][char]0x00E1 + [char]0x00BA))
      foreach ($marker in $markers) { if ($text.Contains($marker)) { $bad.Add("mojibake: $($file.FullName)"); break } }
      if ($file.Extension -in @('.js','.css','.html','.rs')) {
        foreach ($line in ($text -split "`r?`n")) {
          if ($line -match '"[^"\r\n]*(?:l\?i|Ch\?y|k\? ho\?ch|th\?nh|ti\?ng)[^"\r\n]*"|''[^''\r\n]*(?:l\?i|Ch\?y|k\? ho\?ch|th\?nh|ti\?ng)[^''\r\n]*''') { $bad.Add("Vietnamese question-mark word: $($file.FullName)"); break }
        }
      }
    }
  }
  if ($bad.Count) { $bad | ForEach-Object { Write-Output $_ }; throw 'encoding scan failed' }
}
$null = Invoke-LoggedCheck 'line-length' {
  $longSrc = [System.Collections.Generic.List[string]]::new()
  $longUi = [System.Collections.Generic.List[string]]::new()
  foreach ($folder in @('ui','src')) {
    foreach ($file in Get-ChildItem (Join-Path $repo $folder) -File -Recurse) {
      if (($folder -eq 'ui' -and $file.Extension -notin @('.js','.css','.html')) -or ($folder -eq 'src' -and $file.Extension -ne '.rs')) { continue }
      $lineNo = 0
      foreach ($line in [IO.File]::ReadAllLines($file.FullName)) {
        $lineNo++
        if ($line.Length -gt 200) {
          $entry = "$($file.FullName):$lineNo ($($line.Length))"
          if ($folder -eq 'src') { $longSrc.Add($entry) } else { $longUi.Add($entry) }
        }
      }
    }
  }
  if ($longUi.Count) {
    Write-Output "ERROR: $($longUi.Count) UI lines exceed 200 characters"
    $longUi
    throw 'UI source line-length check failed'
  }
  if ($longSrc.Count) {
    Write-Output "ERROR: $($longSrc.Count) Rust source lines exceed 200 characters"
    $longSrc
    throw 'Rust source line-length check failed'
  }
}

if ($failures.Count) { Write-Host 'LỖI'; exit 1 }
Write-Host 'ĐẠT'
exit 0
