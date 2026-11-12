# Draws the AutoKill icon set: two crossed rifles on a rounded dark tile.
# Windows PowerShell 5.1 + System.Drawing only; no other tools needed.
#   powershell -ExecutionPolicy Bypass -File build-icons.ps1
# Then repack .ico/.icns with the packing script (kept in the same commit history).
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'
$root = 'F:\MyApp\eternal\AutoKill\assets\app-icon'
function RiflePath([switch]$simple) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  if ($simple) {
    $p.AddRectangle((New-Object System.Drawing.RectangleF(-46, -2.8, 92, 5.6)))
    $p.AddRectangle((New-Object System.Drawing.RectangleF(-6, 2.8, 20, 12)))
    $p.AddPolygon([System.Drawing.PointF[]]@(
      (New-Object System.Drawing.PointF(-3, 14.8)),
      (New-Object System.Drawing.PointF(13, 14.8)),
      (New-Object System.Drawing.PointF(9, 25)),
      (New-Object System.Drawing.PointF(-7, 25))))
    return $p
  }
  $p.AddRectangle((New-Object System.Drawing.RectangleF(-48, -2.4, 28, 4.8)))
  $p.AddRectangle((New-Object System.Drawing.RectangleF(-34, -7.0, 6, 4.8)))
  $p.AddRectangle((New-Object System.Drawing.RectangleF(-22, -5.0, 12, 10.0)))
  $p.AddRectangle((New-Object System.Drawing.RectangleF(-9, -5.6, 18, 11.2)))
  $p.AddPolygon([System.Drawing.PointF[]]@(
    (New-Object System.Drawing.PointF(9, -4.6)),
    (New-Object System.Drawing.PointF(46, -1.8)),
    (New-Object System.Drawing.PointF(46, 3.0)),
    (New-Object System.Drawing.PointF(9, 2.6))))
  $p.AddPolygon([System.Drawing.PointF[]]@(
    (New-Object System.Drawing.PointF(-4, 5.6)),
    (New-Object System.Drawing.PointF(8.5, 5.6)),
    (New-Object System.Drawing.PointF(4.5, 17.5)),
    (New-Object System.Drawing.PointF(-6, 17.5))))
  $p.AddPolygon([System.Drawing.PointF[]]@(
    (New-Object System.Drawing.PointF(10.5, 5.6)),
    (New-Object System.Drawing.PointF(17, 5.6)),
    (New-Object System.Drawing.PointF(13.5, 15)),
    (New-Object System.Drawing.PointF(8, 15))))
  return $p
}
function New-Icon([int]$size) {
  $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.Clear([System.Drawing.Color]::Transparent)
  $u = $size / 1024.0
  $radius = 205.0 * $u; $d = $radius * 2
  $tile = New-Object System.Drawing.Drawing2D.GraphicsPath
  $tile.AddArc(0, 0, $d, $d, 180, 90)
  $tile.AddArc($size - $d, 0, $d, $d, 270, 90)
  $tile.AddArc($size - $d, $size - $d, $d, $d, 0, 90)
  $tile.AddArc(0, $size - $d, $d, $d, 90, 90)
  $tile.CloseFigure()
  $bg = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 13, 17, 25))
  $g.FillPath($bg, $tile)
  $simple = $size -le 32
  $rifle = RiflePath -simple:$simple
  $scale = ($size / 1024.0) * 7.9
  $dim = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 128, 152, 196))
  $lit = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 246, 250, 255))
  $gap = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 13, 17, 25)), ([single]([Math]::Max(1.0, 6.0 * $u)))
  $cx = [single]($size / 2.0); $cy = [single]($size / 2.0)
  $i = 0
  foreach ($spec in @(@(-33.0, $dim), @(33.0, $lit))) {
    $m = New-Object System.Drawing.Drawing2D.Matrix
    $m.Scale($scale, $scale, [System.Drawing.Drawing2D.MatrixOrder]::Append)
    $m.Rotate([single]$spec[0], [System.Drawing.Drawing2D.MatrixOrder]::Append)
    $m.Translate($cx, $cy, [System.Drawing.Drawing2D.MatrixOrder]::Append)
    $piece = $rifle.Clone(); $piece.Transform($m)
    $g.FillPath($spec[1], $piece)
    if ($i -eq 0) { $g.DrawPath($gap, $piece) } else {
      $wide = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 13, 17, 25)), ([single]([Math]::Max(1.0, 7.0 * $u)))
      $g.DrawPath($wide, $piece); $g.FillPath($spec[1], $piece); $wide.Dispose()
    }
    $piece.Dispose(); $m.Dispose(); $i++
  }
  $g.Dispose()
  return $bmp
}
foreach ($s in @(16, 24, 32, 48, 64, 128, 256, 512, 1024)) {
  $b = New-Icon $s
  if ($s -eq 1024) { $b.Save((Join-Path $root 'autokill-1024.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
  $dir = Join-Path $root ('hicolor\' + $s + 'x' + $s + '\apps')
  if (Test-Path $dir) { $b.Save((Join-Path $dir 'ai.autokill.app.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
  $b.Dispose()
}
'ok'
