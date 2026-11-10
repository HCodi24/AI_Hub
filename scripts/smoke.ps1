[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
$targetRoot = Join-Path $repo 'target\smoke'
$dataRoot = Join-Path $targetRoot 'data'
$logs = Join-Path $targetRoot 'logs'
if (Test-Path $dataRoot) {
  $resolvedData = [IO.Path]::GetFullPath($dataRoot)
  $resolvedTarget = [IO.Path]::GetFullPath($targetRoot) + [IO.Path]::DirectorySeparatorChar
  if (-not $resolvedData.StartsWith($resolvedTarget,[StringComparison]::OrdinalIgnoreCase)) { throw 'smoke data path escaped target\smoke' }
  Remove-Item -LiteralPath $resolvedData -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $dataRoot,$logs | Out-Null
$configPath = Join-Path $dataRoot 'config.json'
if (Test-Path $configPath) { Remove-Item -LiteralPath $configPath -Force }
$env:CARGO_TARGET_DIR = 'target\smoke'
$failures = [System.Collections.Generic.List[string]]::new()
$process = $null
try {
  $build = Start-Process -FilePath 'cargo' -ArgumentList @('build','--offline') -WorkingDirectory $repo -PassThru -Wait -WindowStyle Hidden -RedirectStandardOutput (Join-Path $logs 'build.txt') -RedirectStandardError (Join-Path $logs 'build-stderr.txt')
  if ($build.ExitCode -ne 0) { throw "cargo build failed with exit $($build.ExitCode); see target\smoke\logs" }
  $exe = Join-Path $targetRoot 'debug\ai-hub.exe'
  $env:AI_HUB_CONFIG_PATH = $configPath
  $env:AI_HUB_PORT = '7911'
  $process = Start-Process -FilePath $exe -ArgumentList @('--port','7911','--no-open') -PassThru -WindowStyle Hidden
  $base = 'http://127.0.0.1:7911'
  Add-Type -AssemblyName System.Net.Http
  $client = [System.Net.Http.HttpClient]::new()
  $client.Timeout = [TimeSpan]::FromSeconds(5)
  function Get-Status([string]$Url) {
    $response = $client.GetAsync($Url,[System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
    try { return [int]$response.StatusCode } finally { $response.Dispose() }
  }
  $ready = $false
  for ($i=0; $i -lt 40; $i++) { if ($process.HasExited) { throw "app exited with code $($process.ExitCode)" }; try { Get-Status "$base/" | Out-Null; $ready=$true; break } catch { Start-Sleep -Milliseconds 500 } }
  if (-not $ready) { throw 'server did not become ready on port 7911' }
  $routes = @('/api/dev/status','/api/dev/repo','/api/dev/git/diff','/api/dev/settings','/api/dev/events','/api/config','/api/mutis','/api/mutis/roles/resolve','/api/mutis/stats','/api/mutis/diagnoses','/api/mutis/events','/api/orchestration','/api/orchestration/events','/api/chats','/api/plans','/api/cli-status','/api/extensions')
  foreach ($route in $routes) { try { $status=Get-Status "$base$route"; if ($status -ge 500) { $failures.Add("GET $route => $status") } } catch { $failures.Add("GET $route => $($_.Exception.Message)") } }
  foreach ($path in @('/','/app.js','/style.css')) { try { $status=Get-Status "$base$path"; if ($status -ne 200) { $failures.Add("GET $path => $status") } } catch { $failures.Add("GET $path => $($_.Exception.Message)") } }
  try { $r=Invoke-WebRequest -UseBasicParsing -Method Post -Uri "$base/api/providers/codex-cli/test" -TimeoutSec 15; if ([int]$r.StatusCode -ge 500) { $failures.Add("POST codex-cli/test => $($r.StatusCode)") } } catch { $failures.Add("POST codex-cli/test => $($_.Exception.Message)") }
  try { $current=Invoke-RestMethod -UseBasicParsing "$base/api/mutis" -TimeoutSec 10; $body=ConvertTo-Json -InputObject $current -Depth 30; $r=Invoke-WebRequest -UseBasicParsing -Method Put -Uri "$base/api/mutis" -ContentType 'application/json' -Body $body -TimeoutSec 10; if ([int]$r.StatusCode -ge 500) { $failures.Add("PUT /api/mutis => $($r.StatusCode)") } } catch { $failures.Add("PUT /api/mutis => $($_.Exception.Message)") }
} catch { $failures.Add($_.Exception.Message) }
finally {
  if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue; $process.WaitForExit() }
  Remove-Item Env:AI_HUB_CONFIG_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:AI_HUB_PORT -ErrorAction SilentlyContinue
}
if ($failures.Count) { Write-Host 'SMOKE: LỖI'; $failures | ForEach-Object { Write-Host "- $_" }; exit 1 }
Write-Host 'SMOKE: ĐẠT'
