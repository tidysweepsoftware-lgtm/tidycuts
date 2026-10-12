<#
  Downloads the Studio Voice files and puts them next to the Windows program:

    pwsh scripts/fetch-voice-assets.ps1 -Dest dist/TidyCuts [-Repo owner/name]

  Result:  <Dest>/models/mossformer2_se_48k.onnx
           <Dest>/onnxruntime/onnxruntime.dll (and its licence files)
           <Dest>/whisper-cpp/whisper-cli.exe (and its libraries), for Auto Captions
           <Dest>/imagegen/sd-cli.exe (and its libraries), for the Image Thumbnail Generator

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
# The program that listens for Auto Captions (whisper.cpp, MIT). The speech model is
# downloaded by TidyCuts itself the first time it is needed.
$wz = Join-Path $work "whisper.zip"
Invoke-WebRequest $cfg.WHISPER_CPP_WIN_URL -OutFile $wz
Check $wz $cfg.WHISPER_CPP_WIN_SHA256
Expand-Archive $wz -DestinationPath (Join-Path $work "whisper")
$wsrc = Get-ChildItem (Join-Path $work "whisper") -Directory | Select-Object -First 1
New-Item -ItemType Directory -Force "$Dest\whisper-cpp" | Out-Null
Copy-Item (Join-Path $wsrc.FullName "whisper-cli.exe") "$Dest\whisper-cpp\" -Force
Get-ChildItem $wsrc.FullName -Filter *.dll | ForEach-Object { Copy-Item $_.FullName "$Dest\whisper-cpp\" -Force }
Invoke-WebRequest "https://raw.githubusercontent.com/ggml-org/whisper.cpp/$($cfg.WHISPER_CPP_TAG)/LICENSE" -OutFile "$Dest\whisper-cpp\WHISPER-CPP-LICENSE.txt"
# The program that draws pictures for the Image Thumbnail Generator (stable-diffusion.cpp, MIT).
# Its models are downloaded by TidyCuts itself when the person chooses them.
$iz = Join-Path $work "imagegen.zip"
Invoke-WebRequest $cfg.SD_CPP_WIN_URL -OutFile $iz
Check $iz $cfg.SD_CPP_WIN_SHA256
New-Item -ItemType Directory -Force "$Dest\imagegen" | Out-Null
Expand-Archive $iz -DestinationPath "$Dest\imagegen" -Force
Invoke-WebRequest "https://raw.githubusercontent.com/leejet/stable-diffusion.cpp/$($cfg.SD_CPP_TAG)/LICENSE" -OutFile "$Dest\imagegen\STABLE-DIFFUSION-CPP-LICENSE.txt"
[IO.Directory]::Delete($work, $true)
Write-Host "Studio Voice files are in $Dest"
