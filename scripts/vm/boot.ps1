param([string]$HostIp = "10.0.2.2")
$ErrorActionPreference = "Continue"
$base = "http://$HostIp`:8000"
$log  = "C:\probe-log.txt"
function Post($name, $text) {
  try { Invoke-WebRequest -UseBasicParsing -Method POST -Uri "$base/result/$name" -Body ([Text.Encoding]::UTF8.GetBytes($text)) -TimeoutSec 30 | Out-Null } catch {}
}
"boot on $(Get-Date -Format o), host $HostIp" | Tee-Object -FilePath $log
Invoke-WebRequest -UseBasicParsing "$base/gate-b.zip" -OutFile C:\gate-b.zip
Expand-Archive -Force C:\gate-b.zip C:\gate-b
Set-Location C:\gate-b
$env:XAML_DLL = "C:\gate-b\Microsoft.ui.xaml.dll"
$env:OPENCOMP_TRACE = "1"
$env:OPENCOMP_XAML_PROBE_START = "1"
$out = & .\xaml-probe.exe 2>&1 | Out-String
$out | Tee-Object -FilePath $log -Append
Post "measure" ("HOST=" + $env:COMPUTERNAME + "`n" + $out)
$env:OPENCOMP_XAML_PROBE_WINDOW = "1"
$out2 = & .\xaml-probe.exe 2>&1 | Out-String
$out2 | Tee-Object -FilePath $log -Append
Post "window" $out2
"done" | Post "done" "ok"
