# 启动已构建的 AutoKill
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = Join-Path $root 'target\debug\autokill.exe'
if (-not (Test-Path $exe)) { throw "not built yet: $exe (run build.ps1 first)" }
Start-Process -FilePath $exe -WorkingDirectory $root
