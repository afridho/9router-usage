param(
  [string]$Version = "",
  [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

if (-not $Version) {
  $config = Get-Content (Join-Path $Root "src-tauri/tauri.conf.json") -Raw | ConvertFrom-Json
  $Version = $config.version
}

$parts = @($Version.Split('.') | ForEach-Object { [int]$_ })
if ($parts.Count -gt 3) { throw "Store version must contain at most three components; the fourth is reserved by Microsoft." }
while ($parts.Count -lt 3) { $parts += 0 }
$StoreVersion = "$($parts[0]).$($parts[1]).$($parts[2]).0"

if (-not $SkipBuild) {
  Push-Location $Root
  try { npm run tauri -- build --no-bundle } finally { Pop-Location }
}

$Exe = Join-Path $Root "src-tauri/target/release/router-usage.exe"
if (-not (Test-Path $Exe)) { throw "Release executable not found at $Exe" }

$MakeAppx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue |
  Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $MakeAppx) {
  $fallback = "${env:ProgramFiles(x86)}\Windows Kits\10\App Certification Kit\makeappx.exe"
  if (Test-Path $fallback) { $MakeAppx = $fallback }
}
if (-not $MakeAppx) { throw "MakeAppx.exe was not found. Install the Windows 10/11 SDK." }

$Layout = Join-Path $Root "dist/msix-layout"
$Assets = Join-Path $Layout "Assets"
$Output = Join-Path $Root "dist/9Router-Usage-$Version-store-x64.msix"
Remove-Item $Layout -Recurse -Force -ErrorAction SilentlyContinue
New-Item $Assets -ItemType Directory -Force | Out-Null
New-Item (Split-Path $Output) -ItemType Directory -Force | Out-Null

Copy-Item $Exe (Join-Path $Layout "9Router Usage.exe")
$IconDir = Join-Path $Root "src-tauri/icons"
Copy-Item (Join-Path $IconDir "StoreLogo.png") $Assets
Copy-Item (Join-Path $IconDir "Square44x44Logo.png") $Assets
Copy-Item (Join-Path $IconDir "Square71x71Logo.png") $Assets
Copy-Item (Join-Path $IconDir "Square150x150Logo.png") $Assets
Copy-Item (Join-Path $IconDir "Square310x310Logo.png") $Assets
Copy-Item (Join-Path $IconDir "Square150x150Logo.png") (Join-Path $Assets "Wide310x150Logo.png")

$manifestTemplate = Get-Content (Join-Path $Root "src-tauri/store/AppxManifest.xml.template") -Raw
$manifestTemplate.Replace("__VERSION__", $StoreVersion) | Set-Content (Join-Path $Layout "AppxManifest.xml") -Encoding utf8

Remove-Item $Output -Force -ErrorAction SilentlyContinue
& $MakeAppx pack /o /h SHA256 /d $Layout /p $Output
if ($LASTEXITCODE -ne 0) { throw "MakeAppx failed with exit code $LASTEXITCODE" }
Write-Host "Created Microsoft Store package: $Output"
Write-Host "This package is intentionally unsigned for Partner Center submission."
