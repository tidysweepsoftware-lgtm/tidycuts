<#
  Builds the Microsoft Store package (.msix) from the Windows program folder.

    pwsh scripts/make-msix.ps1 -Folder dist/TidyCuts -Version 1.3.3 -Out dist

  -Folder  the folder that holds TidyCuts.exe and the ffmpeg folder (the same one the
           installer is made from)
  -Version the app version (three numbers; a fourth, 0, is added)
  -IdentityName / -Publisher  the values from Partner Center (Product identity). The
           defaults are placeholders that let the package build but that the Store
           will refuse until they are replaced.
  The Store signs the package itself, so it is left unsigned here.
#>
param(
  [Parameter(Mandatory = $true)][string]$Folder,
  [Parameter(Mandatory = $true)][string]$Version,
  [string]$Out = "dist",
  [string]$IdentityName = "TidySweepSoftware.TidyCuts",
  [string]$Publisher = "CN=TidySweep Software"
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
$repo = Split-Path -Parent $PSScriptRoot
$work = Join-Path ([IO.Path]::GetTempPath()) ("tidycuts-msix-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force "$work\Assets" | Out-Null

# The program and FFmpeg, as they are in the installer.
Copy-Item -Recurse -Force (Join-Path $Folder "*") $work
Remove-Item -ErrorAction SilentlyContinue "$work\TidyCuts.ico"

# Store pictures made from the app icon.
$icon = [System.Drawing.Image]::FromFile((Join-Path $repo "packaging\icons\tidycuts-1024.png"))
function Save-Logo($name, $w, $h) {
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = "HighQuality"
  $g.InterpolationMode = "HighQualityBicubic"
  $g.Clear([System.Drawing.Color]::FromArgb(0, 0, 0, 0))
  $side = [Math]::Min($w, $h)
  $pad = [Math]::Round($side * 0.1)
  $inner = $side - 2 * $pad
  $x = [Math]::Round(($w - $inner) / 2)
  $y = [Math]::Round(($h - $inner) / 2)
  $g.DrawImage($icon, $x, $y, $inner, $inner)
  $bmp.Save("$work\Assets\$name.png", [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose()
  $bmp.Dispose()
}
Save-Logo "StoreLogo" 50 50
Save-Logo "Square44x44Logo" 44 44
Save-Logo "Square71x71Logo" 71 71
Save-Logo "Square150x150Logo" 150 150
Save-Logo "Wide310x150Logo" 310 150
Save-Logo "Square310x310Logo" 310 310
$icon.Dispose()

$manifest = Get-Content -Raw (Join-Path $repo "packaging\windows\msix\AppxManifest.xml")
$manifest = $manifest.Replace("__VERSION__", "$Version.0").Replace("__IDENTITY_NAME__", $IdentityName).Replace("__PUBLISHER__", $Publisher)
Set-Content -Encoding utf8 "$work\AppxManifest.xml" $manifest

# makeappx comes with the Windows SDK (the Windows runners on GitHub have it).
$bin = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
$sdk = Get-ChildItem $bin -Recurse -Filter makeappx.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -like "*\x64\*" } |
  Sort-Object FullName -Descending |
  Select-Object -First 1
if (-not $sdk) { throw "makeappx.exe not found: install the Windows SDK" }
New-Item -ItemType Directory -Force $Out | Out-Null
$pkg = Join-Path $Out "TidyCuts-$Version-store.msix"
if (Test-Path $pkg) { Remove-Item $pkg }
& $sdk.FullName pack /d $work /p $pkg /o | Out-Host
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
Remove-Item -Recurse -Force $work
Write-Host "Wrote $pkg"
