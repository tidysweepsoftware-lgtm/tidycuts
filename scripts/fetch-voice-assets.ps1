<#
  Downloads the Studio Voice files and puts them next to the Windows program:

    pwsh scripts/fetch-voice-assets.ps1 -Dest dist/TidyCuts [-Repo owner/name]

  Result:  <Dest>/models/mossformer2_se_48k.onnx
           <Dest>/onnxruntime/onnxruntime.dll (and its licence files)

  The versions and checksums are in packaging/voice-assets.txt. A download whose
  checksum is wrong stops the script. Without -Repo the repository is read from
  the GITHUB_REPOSITORY variable (set in GitHub Actions) or from git.
#>
param(
  [Parameter(Mandatory = $true)][string]$Dest,
  [string]$Repo = $env:GITHUB_REPOSITORY
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$cfg = @{}
Get-Content (Join-Path $root "packaging\voice-assets.txt") | ForEach-Object {
  if ($_ -match '^\s*([A-Z0-9_]+)=(.*)$') { $cfg[$Matches[1]] = $Matches[2].Trim() }
}
if (-not $Repo) {
  $remote = git -C $root remote get-url origin
  $Repo = ($remote -replace '^.*github\.com[:/]', '') -replace '\.git$', ''
}
function Check($file, $want) {
  $got = (Get-FileHash $file -Algorithm SHA256).Hash.ToLower()
  if ($got -ne $want.ToLower()) { throw "Checksum of $file is $got, expected $want" }
}
$work = Join-Path ([IO.Path]::GetTempPath()) ("tidycuts-voice-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $work | Out-Null
New-Item -ItemType Directory -Force "$Dest\models", "$Dest\onnxruntime" | Out-Null

# The model, from the repository's release (a draft release needs the gh tool).
$model = Join-Path $work $cfg.MODEL_FILE
if (Get-Command gh -ErrorAction SilentlyContinue) {
  gh release download $cfg.MODEL_TAG -R $Repo -p $cfg.MODEL_FILE -D $work --clobber
} else {
  Invoke-WebRequest "https://github.com/$Repo/releases/download/$($cfg.MODEL_TAG)/$($cfg.MODEL_FILE)" -OutFile $model
}
Check $model $cfg.MODEL_SHA256
Copy-Item $model "$Dest\models\$($cfg.MODEL_FILE)" -Force

# ONNX Runtime, from Microsoft.
$zip = Join-Path $work "ort.zip"
Invoke-WebRequest "https://github.com/microsoft/onnxruntime/releases/download/v$($cfg.ORT_VERSION)/onnxruntime-win-x64-$($cfg.ORT_VERSION).zip" -OutFile $zip
Check $zip $cfg.ORT_WIN_SHA256
Expand-Archive $zip -DestinationPath (Join-Path $work "ort")
$src = Get-ChildItem (Join-Path $work "ort") -Directory | Select-Object -First 1
foreach ($f in "lib\onnxruntime.dll", "lib\onnxruntime_providers_shared.dll") {
  $p = Join-Path $src.FullName $f
  if (Test-Path $p) { Copy-Item $p "$Dest\onnxruntime\" -Force }
}
Copy-Item (Join-Path $src.FullName "LICENSE") "$Dest\onnxruntime\ONNXRUNTIME-LICENSE.txt" -Force
Copy-Item (Join-Path $src.FullName "ThirdPartyNotices.txt") "$Dest\onnxruntime\ONNXRUNTIME-THIRD-PARTY-NOTICES.txt" -Force
# The speech models for Auto Captions.
$asr = @(@("asr-en", $cfg.ASR_EN_URL, $cfg.ASR_EN_SHA256))
if ($env:TIDYCUTS_WITH_MULTILINGUAL -eq "1") { $asr += ,@("asr-multi", $cfg.ASR_MULTI_URL, $cfg.ASR_MULTI_SHA256) }
foreach ($m in $asr) {
  $arc = Join-Path $work ($m[0] + ".tar.bz2")
  Invoke-WebRequest $m[1] -OutFile $arc
  Check $arc $m[2]
  $out = Join-Path $work $m[0]
  New-Item -ItemType Directory -Force $out | Out-Null
  tar -xjf $arc -C $out
  $inner = Get-ChildItem $out -Directory | Select-Object -First 1
  New-Item -ItemType Directory -Force "$Dest\models\$($m[0])" | Out-Null
  Copy-Item (Join-Path $inner.FullName "model.int8.onnx") "$Dest\models\$($m[0])\" -Force
  Copy-Item (Join-Path $inner.FullName "tokens.txt") "$Dest\models\$($m[0])\" -Force
}
[IO.Directory]::Delete($work, $true)
Write-Host "Studio Voice files are in $Dest"
