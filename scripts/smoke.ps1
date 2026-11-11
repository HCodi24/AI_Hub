[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
$targetRoot = Join-Path $repo ('target\smoke-' + [Guid]::NewGuid().ToString('N'))
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
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = 'target\smoke' }
$failures = [System.Collections.Generic.List[string]]::new()
$process = $null
try {
  $build = Start-Process -FilePath 'cargo' -ArgumentList @('build','--offline') -WorkingDirectory $repo -PassThru -Wait -WindowStyle Hidden -RedirectStandardOutput (Join-Path $logs 'build.txt') -RedirectStandardError (Join-Path $logs 'build-stderr.txt')
  if ($build.ExitCode -ne 0) { throw "cargo build failed with exit $($build.ExitCode); see target\smoke\logs" }
  $builtExe = Join-Path $env:CARGO_TARGET_DIR 'debug\ai-hub.exe'
  $exe = Join-Path $targetRoot 'ai-hub.exe'
  Copy-Item -LiteralPath $builtExe -Destination $exe
  $env:AI_HUB_CONFIG_PATH = $configPath
  $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
  $listener.Start()
  $smokePort = $listener.LocalEndpoint.Port
  $listener.Stop()
  $env:AI_HUB_PORT = [string]$smokePort
  $process = Start-Process -FilePath $exe -ArgumentList @('--port', [string]$smokePort, '--no-open') -PassThru -WindowStyle Hidden
  $base = "http://127.0.0.1:$smokePort"
  Add-Type -AssemblyName System.Net.Http
  $client = [System.Net.Http.HttpClient]::new()
  # Role resolution probes local CLI binaries, which can take up to 10 seconds.
  $client.Timeout = [TimeSpan]::FromSeconds(20)
  function Get-Status([string]$Url) {
    $response = $client.GetAsync($Url,[System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
    try { return [int]$response.StatusCode } finally { $response.Dispose() }
  }
  $ready = $false
  for ($i=0; $i -lt 40; $i++) { if ($process.HasExited) { throw "app exited with code $($process.ExitCode)" }; try { Get-Status "$base/" | Out-Null; $ready=$true; break } catch { Start-Sleep -Milliseconds 500 } }
  if (-not $ready) { throw 'server did not become ready on port 7911' }
  $routes = @('/api/dev/status','/api/dev/repo','/api/dev/git/diff','/api/dev/settings','/api/dev/events','/api/config','/api/mutis','/api/mutis/roles/resolve','/api/mutis/stats','/api/mutis/diagnoses','/api/mutis/events','/api/chats','/api/plans','/api/cli-status','/api/extensions')
  foreach ($route in $routes) { try { $status=Get-Status "$base$route"; if ($status -ge 500) { $failures.Add("GET $route => $status") } } catch { $failures.Add("GET $route => $($_.Exception.Message)") } }
  $uiFiles = @(Get-ChildItem -LiteralPath (Join-Path $repo 'ui') -File -Recurse | Where-Object { $_.Extension -in @('.html','.js','.css','.svg','.png') })
  $assetPaths = [System.Collections.Generic.List[string]]::new()
  $assetPaths.Add('/')
  $uiRoot = [IO.Path]::GetFullPath((Join-Path $repo 'ui')).TrimEnd([IO.Path]::DirectorySeparatorChar,[IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
  foreach ($file in $uiFiles) {
    $relative = $file.FullName.Substring($uiRoot.Length).Replace('\','/')
    if ($relative -ne 'index.html') { $assetPaths.Add("/$relative") }
  }
  foreach ($path in $assetPaths) { try { $status=Get-Status "$base$path"; if ($status -ne 200) { $failures.Add("GET $path => $status") } } catch { $failures.Add("GET $path => $($_.Exception.Message)") } }
  if (Test-Path (Join-Path $repo 'web/out/index.html')) {
    try {
      $response = $client.GetAsync("$base/app/").GetAwaiter().GetResult()
      try {
        if ([int]$response.StatusCode -ne 200 -or $response.Content.Headers.ContentType.MediaType -ne 'text/html') {
          throw '/app/ must return 200 HTML'
        }
        $html = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
        if (-not $html.Contains('/app/_next/')) { throw '/app/ did not serve the Next export' }
      } finally { $response.Dispose() }
    } catch { $failures.Add("web: $($_.Exception.Message)") }
  }
  # Memory CRUD stays entirely local; no provider request is made.
  try {
    $folderQuery = [Uri]::EscapeDataString($repo)
    $memoryUrl = "$base/api/memory"
    $scopeUrl = "$memoryUrl/smoke-memory"
    # ASCII base64 keeps this UTF-8 fixture intact even in Windows PowerShell 5.1.
    $scopeText = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String(
      'LS0tCnNjb3BlOiBzbW9rZS1tZW1vcnkKdGl0bGU6IELhu5kgbmjhu5sga2nhu4NtIHRyYQpzdGF0dXM6IGFjdGl2ZQp1cGRhdGVkOiAyMDI2LTExLTEwVDAwOjAwOjAwWgpjaGF0czogW10KZmlsZXM6IFtdCm5leHQ6IFJlc3RvcmUKZXZpZGVuY2U6IFtdCi0tLQojIyBN4bulYyB0acOqdQotIEdp4buvIHRp4bq/bmcgVmnhu4d0CiMjIMSQw6MgY2jhu5F0Ci0gSFRUUCBsb2NhbAojIyDEkMOjIGzDoG0KIyMgQ8OybiBt4bufCg=='))
    $expectedText = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('R2nhu68gdGnhur9uZyBWaeG7h3Q='))
    function Invoke-MemoryJson([string]$Url, [string]$Method = 'GET', [byte[]]$Body = $null) {
      $request = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::new($Method), $Url)
      if ($Body) {
        $request.Content = [Net.Http.ByteArrayContent]::new($Body)
        $request.Content.Headers.ContentType = [Net.Http.Headers.MediaTypeHeaderValue]::new('application/json')
      }
      $response = $client.SendAsync($request).GetAwaiter().GetResult()
      try {
        $response.EnsureSuccessStatusCode() | Out-Null
        return ($response.Content.ReadAsStringAsync().GetAwaiter().GetResult() | ConvertFrom-Json)
      } finally { $response.Dispose(); $request.Dispose() }
    }
    $body = [Text.Encoding]::UTF8.GetBytes((ConvertTo-Json @{text=$scopeText}))
    $saved = Invoke-MemoryJson "${scopeUrl}?folder=$folderQuery" 'PUT' $body
    if (-not $saved.text.Contains($expectedText)) { throw 'memory UTF-8 roundtrip failed' }
    $view = Invoke-MemoryJson "${memoryUrl}?folder=$folderQuery"
    if ($view.scopes.Count -ne 1 -or -not $view.index.Contains('smoke-memory')) { throw 'memory index missing' }
    Invoke-MemoryJson "${scopeUrl}/archive?folder=$folderQuery" 'POST' | Out-Null
    $view = Invoke-MemoryJson "${memoryUrl}?folder=$folderQuery"
    if ($view.archived.Count -ne 1 -or $view.scopes.Count -ne 0) { throw 'memory archive failed' }
    Invoke-MemoryJson "${scopeUrl}/restore?folder=$folderQuery" 'POST' | Out-Null
    $restored = Invoke-MemoryJson "${scopeUrl}?folder=$folderQuery"
    if (-not $restored.text.Contains('status: active')) { throw 'memory restore failed' }
    Invoke-MemoryJson "${scopeUrl}/archive?folder=$folderQuery" 'POST' | Out-Null
    Invoke-MemoryJson "${scopeUrl}?folder=$folderQuery" 'DELETE' | Out-Null
    $view = Invoke-MemoryJson "${memoryUrl}?folder=$folderQuery"
    if ($view.archived.Count -ne 0) { throw 'memory delete failed' }
    $config = Invoke-RestMethod "$base/api/config"
    $configBody = ConvertTo-Json @{default_folder=$repo; memory=@{enabled=$false;max_scopes_in_prompt=4;max_chars_per_scope=7000}}
    Invoke-RestMethod -Method Put "$base/api/config" -ContentType 'application/json' -Body $configBody | Out-Null
    $partial = ConvertTo-Json @{default_folder=$repo}
    $config = Invoke-RestMethod -Method Put "$base/api/config" -ContentType 'application/json' -Body $partial
    if ($config.memory.enabled -or $config.memory.max_scopes_in_prompt -ne 4) { throw 'partial config reset memory' }
  } catch { $failures.Add("memory: $($_.Exception.Message)") }
  try { $r=Invoke-WebRequest -UseBasicParsing -Method Post -Uri "$base/api/providers/codex-cli/test" -TimeoutSec 15; if ([int]$r.StatusCode -ge 500) { $failures.Add("POST codex-cli/test => $($r.StatusCode)") } } catch { $failures.Add("POST codex-cli/test => $($_.Exception.Message)") }
  try { $current=Invoke-RestMethod -UseBasicParsing "$base/api/mutis" -TimeoutSec 10; $body=ConvertTo-Json -InputObject $current -Depth 30; $r=Invoke-WebRequest -UseBasicParsing -Method Put -Uri "$base/api/mutis" -ContentType 'application/json' -Body $body -TimeoutSec 10; if ([int]$r.StatusCode -ge 500) { $failures.Add("PUT /api/mutis => $($r.StatusCode)") } } catch { $failures.Add("PUT /api/mutis => $($_.Exception.Message)") }
} catch { $failures.Add($_.Exception.Message) }
finally {
  if ($client) { $client.Dispose() }
  # Only the server this script started is stopped; the user's own ai-hub processes are never touched.
  if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue; $process.WaitForExit() }
  Remove-Item Env:AI_HUB_CONFIG_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:AI_HUB_PORT -ErrorAction SilentlyContinue
}
if ($failures.Count) { Write-Host 'SMOKE: LỖI'; $failures | ForEach-Object { Write-Host "- $_" }; exit 1 }
Write-Host 'SMOKE: ĐẠT'
