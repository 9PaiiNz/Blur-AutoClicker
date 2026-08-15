# Packages the portable build of BlurAutoClicker:
#   - BlurAutoClicker.exe (from target/release, built by the release workflow)
#   - Visual C++ runtime DLLs + crashpad_handler.exe (from prepare-windows-redist.ps1)
#   - MicrosoftEdgeWebview2Setup.exe (Evergreen bootstrapper, ~2 MB)
#   - portable.txt marker (activates portable mode at runtime)
#   - README.txt
#
# Produces a single zip. Updates are manual: the app shows a "Download from
# GitHub" button instead of the in-app updater.

param(
  [string]$RepoRoot = (Split-Path -Parent $PSScriptRoot),
  [string]$Tag = $env:GITHUB_REF_NAME,
  [string]$OutDir = $RepoRoot
)

$ErrorActionPreference = 'Stop'

if (-not $Tag) { $Tag = 'dev' }
$Tag = $Tag -replace '^v', ''

$releaseDir = Join-Path $RepoRoot 'src-tauri/target/release'
$exe = Join-Path $releaseDir 'BlurAutoClicker.exe'
if (-not (Test-Path $exe)) {
  Write-Error "Build output not found: $exe. Run a release build first."
}

# 1. Stage VC++ CRT DLLs + crashpad_handler.exe (idempotent).
& (Join-Path $PSScriptRoot 'prepare-windows-redist.ps1') -RepoRoot $RepoRoot

$resourcesDir = Join-Path $RepoRoot 'src-tauri/resources'
if (-not (Test-Path $resourcesDir)) {
  Write-Error "Resources dir missing: $resourcesDir"
}

# 2. Stage portable folder.
$stageDir = Join-Path $env:TEMP ('blur-portable-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $stageDir | Out-Null

Copy-Item $exe -Destination (Join-Path $stageDir 'BlurAutoClicker.exe')

Get-ChildItem -Path $resourcesDir -File | Copy-Item -Destination $stageDir -Force

# 3. WebView2 Evergreen bootstrapper. Skip quietly if the download fails so the
#    packaging never hard-fails on a network hiccup.
$bootstrapper = Join-Path $stageDir 'MicrosoftEdgeWebview2Setup.exe'
if (-not (Test-Path $bootstrapper)) {
  try {
    Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $bootstrapper
    Write-Host '[package-portable] Downloaded WebView2 bootstrapper'
  } catch {
    Write-Warning '[package-portable] Could not download WebView2 bootstrapper; users without WebView2 must install it manually.'
  }
}

# 4. Portable marker + README.
Set-Content -Path (Join-Path $stageDir 'portable.txt') -Value 'BlurAutoClicker Portable Mode' -NoNewline

@"
BlurAutoClicker v$Tag - Portable

How to use
  Extract this zip anywhere you can write to (Downloads, Documents, a USB
  drive) and run BlurAutoClicker.exe. No installer, no admin rights.

WebView2
  BlurAutoClicker needs the Microsoft Edge WebView2 Runtime. Most Windows 10
  and 11 machines already have it. If not, the included
  MicrosoftEdgeWebview2Setup.exe installs it, or you can get it from
  https://go.microsoft.com/fwlink/p/?LinkId=2124703

Portable data
  All settings, statistics and diagnostics are stored in the "Data" folder
  next to the exe. WebView2 registers itself in Windows (one time, when it is
  installed) and the app may write a small metadata file to %APPDATA%.
  Deleting the folder removes the app's own data completely.

Updates
  This build does not auto-update. When a new release is announced, download
  the newest portable zip from https://github.com/Blur009/Blur-AutoClicker/releases
  and replace the files (your Data folder is kept).
"@ | Set-Content -Path (Join-Path $stageDir 'README.txt')

# 5. Zip with tar (preserves attributes; avoids Compress-Archive 2GB/attr limits).
$zipName = "BlurAutoClicker-v${Tag}-portable.zip"
$zipPath = Join-Path $OutDir $zipName
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }

tar -a -cf $zipPath -C $stageDir .

if ($LASTEXITCODE -ne 0) {
  Write-Error "tar failed with exit code $LASTEXITCODE"
}

Write-Host "[package-portable] Wrote $zipPath"
Get-ChildItem -Path $stageDir | Select-Object -ExpandProperty Name

Remove-Item -Recurse -Force $stageDir