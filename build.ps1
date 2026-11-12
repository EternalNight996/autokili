# 构建 AutoKill（Windows / PowerShell 5.1）
# 依赖：rustc >= 1.97；craft-libs 检出在 ..craft-libs-src（Cargo.toml 的 [patch] 指向它）；
# 中文界面需要 craft-fonts，检出路径通过 CRAFT_FONTS_DIR 指定。
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

if (-not $env:CRAFT_FONTS_DIR) {
  $guess = Join-Path (Split-Path -Parent $root) 'craft-fonts-src'
  if (Test-Path $guess) {
    $checkout = Get-ChildItem $guess -Directory | Select-Object -First 1
    if ($checkout) { $env:CRAFT_FONTS_DIR = $checkout.FullName; $env:CRAFT_FONTS_REQUIRED = '1' }
  }
}
if ($env:CRAFT_FONTS_DIR) { Write-Host "fonts: $env:CRAFT_FONTS_DIR" } else { Write-Warning 'CRAFT_FONTS_DIR unset: UI will show CJK text as boxes' }

cargo build -p autokill -p autokill-cli
Write-Host "built: $(Join-Path $root 'target\debug\autokill.exe')"
