param([string]$HostIp = "10.0.2.2")
$ErrorActionPreference = "Continue"
$base = "http://$HostIp`:8000"
function Post($name, $text) {
  try { Invoke-WebRequest -UseBasicParsing -Method POST -Uri "$base/result/$name" -Body ([Text.Encoding]::UTF8.GetBytes($text)) -TimeoutSec 120 | Out-Null } catch {}
}
Post "boot" ("started $(Get-Date -Format o) from $HostIp`n" + (Get-ComputerInfo | Select-Object -Property WindowsProductName,WindowsVersion,OsArchitecture | Out-String))

# 1. The baseline: the SDK's own DLLs and a manifest with no replacements, so the engine
#    runs on the platform it was built for. This is the reference for every Wine question.
New-Item -ItemType Directory -Force C:\base | Out-Null
Invoke-WebRequest -UseBasicParsing "$base/gate-b-baseline.zip" -OutFile C:\base.zip
Expand-Archive -Force C:\base.zip C:\base
Set-Location C:\base
$env:XAML_DLL = "C:\base\Microsoft.ui.xaml.dll"
$env:OPENCOMP_TRACE = "1"
$env:OPENCOMP_XAML_PROBE_START = "1"
$env:OPENCOMP_XAML_PROBE_WINDOW = $null
$measure = & .\xaml-probe.exe 2>&1 | Out-String
Post "baseline-measure" $measure
$env:OPENCOMP_XAML_PROBE_WINDOW = "1"
$window = & .\xaml-probe.exe 2>&1 | Out-String
Post "baseline-window" $window

# 2. The stand-in: the same probe with this project's DLLs in place of the SDK's.
New-Item -ItemType Directory -Force C:\probe | Out-Null
Invoke-WebRequest -UseBasicParsing "$base/gate-b.zip" -OutFile C:\probe.zip
Expand-Archive -Force C:\probe.zip C:\probe
Set-Location C:\probe
$env:XAML_DLL = "C:\probe\Microsoft.ui.xaml.dll"
$env:OPENCOMP_XAML_PROBE_WINDOW = $null
$standin = & .\xaml-probe.exe 2>&1 | Out-String
Post "standin-measure" $standin
$env:OPENCOMP_XAML_PROBE_WINDOW = "1"
$standinWindow = & .\xaml-probe.exe 2>&1 | Out-String
Post "standin-window" $standinWindow
Post "done" "ok"
