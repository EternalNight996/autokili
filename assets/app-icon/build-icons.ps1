#requires -Version 5.1
<#
  build-icons.ps1 - generates the AutoKill app icon set (original artwork, no upstream marks).
  Renders with System.Drawing only; ICO and ICNS containers are assembled by hand.
  Output: 1024 PNG, SVG, multi-size ICO, ICNS, and the hicolor tree.
#>
param(
  [string]$Root = 'F:\MyApp\eternal\AutoKill\assets\app-icon'
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

# ---- brand palette -------------------------------------------------------
# tile #101014, mark #4E7BFB, accent bar #1659C4
$TileColor   = [System.Drawing.Color]::FromArgb(255, 16, 16, 20)
$MarkColor   = [System.Drawing.Color]::FromArgb(255, 78, 123, 251)
$AccentColor = [System.Drawing.Color]::FromArgb(255, 22, 89, 196)

$MarkText    = 'AK'
$FontFamily  = 'Segoe UI'

# ---- geometry (fractions of the square) ---------------------------------
$CornerFrac  = 0.20    # corner radius of the tile
$TextWidthF  = 0.68    # max width of the AK mark
$TextHeightF = 0.40    # max height of the AK mark
$CenterYwBar = 0.44    # optical centre of the mark when the bar is present
$BarWidthF   = 0.30
$BarHeightF  = 0.040
$BarCenterY  = 0.715
$BarMinPx    = 1.5
$BarMinSize  = 32      # below that the bar only smears the mark, so it is dropped

function New-RoundedTilePath {
  param([single]$Size, [single]$Radius)
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = $Radius * 2
  $path.AddArc([single]0, [single]0, [single]$d, [single]$d, [single]180, [single]90)
  $path.AddArc([single]($Size - $d), [single]0, [single]$d, [single]$d, [single]270, [single]90)
  $path.AddArc([single]($Size - $d), [single]($Size - $d), [single]$d, [single]$d, [single]0, [single]90)
  $path.AddArc([single]0, [single]($Size - $d), [single]$d, [single]$d, [single]90, [single]90)
  $path.CloseFigure()
  return $path
}

function New-IconBitmap {
  param([int]$Size)

  $s = [single]$Size
  $bmp = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode     = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.PixelOffsetMode   = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
  $g.Clear([System.Drawing.Color]::Transparent)

  # 1. rounded tile, flat fill
  $tile = New-RoundedTilePath -Size $s -Radius ([single]($s * $CornerFrac))
  $tileBrush = New-Object System.Drawing.SolidBrush($TileColor)
  $g.FillPath($tileBrush, $tile)
  $tile.Dispose(); $tileBrush.Dispose()

  $useBar = ($Size -ge $BarMinSize)

  # 2. the AK mark, as a filled outline so it stays sharp at every size
  $family = New-Object System.Drawing.FontFamily($FontFamily)
  $mark = New-Object System.Drawing.Drawing2D.GraphicsPath
  $fmt = [System.Drawing.StringFormat]::GenericTypographic
  $mark.AddString($MarkText, $family, [System.Drawing.FontStyle]::Bold, [single]1000,
                  (New-Object System.Drawing.PointF(0, 0)), $fmt)

  $b0 = $mark.GetBounds()
  $scale = [Math]::Min(($s * $TextWidthF) / $b0.Width, ($s * $TextHeightF) / $b0.Height)
  $m1 = New-Object System.Drawing.Drawing2D.Matrix
  $m1.Scale([single]$scale, [single]$scale)
  $mark.Transform($m1)
  $m1.Dispose()

  $b1 = $mark.GetBounds()
  $cx = $s / 2.0
  if ($useBar) { $cy = $s * $CenterYwBar } else { $cy = $s / 2.0 }
  $m2 = New-Object System.Drawing.Drawing2D.Matrix
  $m2.Translate([single]($cx - ($b1.X + $b1.Width / 2.0)), [single]($cy - ($b1.Y + $b1.Height / 2.0)))
  $mark.Transform($m2)
  $m2.Dispose()

  $markBrush = New-Object System.Drawing.SolidBrush($MarkColor)
  $g.FillPath($markBrush, $mark)
  $markBrush.Dispose(); $mark.Dispose(); $family.Dispose()

  # 3. accent bar under the mark
  if ($useBar) {
    $bw = [single]($s * $BarWidthF)
    $bh = [single][Math]::Max($BarMinPx, $s * $BarHeightF)
    $bx = [single](($s - $bw) / 2.0)
    $by = [single](($s * $BarCenterY) - ($bh / 2.0))
    $barBrush = New-Object System.Drawing.SolidBrush($AccentColor)
    $g.FillRectangle($barBrush, $bx, $by, $bw, $bh)
    $barBrush.Dispose()
  }

  $g.Dispose()
  return $bmp
}

function Get-PngBytes {
  param([System.Drawing.Bitmap]$Bitmap)
  $ms = New-Object System.IO.MemoryStream
  $Bitmap.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
  $bytes = $ms.ToArray()
  $ms.Dispose()
  return , $bytes
}

function Write-Bytes {
  param([string]$Path, [byte[]]$Bytes)
  $dir = [System.IO.Path]::GetDirectoryName($Path)
  if (-not [System.IO.Directory]::Exists($dir)) { [System.IO.Directory]::CreateDirectory($dir) | Out-Null }
  [System.IO.File]::WriteAllBytes($Path, $Bytes)
}

# ---- render every raster size once --------------------------------------
$sizes = @(16, 24, 32, 48, 64, 128, 256, 512, 1024)
$png = @{}
foreach ($size in $sizes) {
  $bmp = New-IconBitmap -Size $size
  $png[$size] = Get-PngBytes -Bitmap $bmp
  $bmp.Dispose()
}

# ---- 1. master PNG -------------------------------------------------------
Write-Bytes -Path (Join-Path $Root 'autokill-1024.png') -Bytes $png[1024]

# ---- 2. hicolor theme tree ----------------------------------------------
foreach ($size in @(16, 24, 32, 48, 64, 128, 256, 512)) {
  $target = Join-Path $Root ("hicolor\{0}x{0}\apps\ai.autokill.app.png" -f $size)
  Write-Bytes -Path $target -Bytes $png[$size]
}

# ---- 3. ICO (PNG frames: 16/24/32/48/64/128/256) ------------------------
$icoSizes = @(16, 24, 32, 48, 64, 128, 256)
$icoPath = Join-Path $Root 'autokill.ico'
$icoMs = New-Object System.IO.MemoryStream
$icoW = New-Object System.IO.BinaryWriter($icoMs)
$icoW.Write([byte]0)                       # reserved
$icoW.Write([byte]0)
$icoW.Write([byte]1)                       # type 1 = icon
$icoW.Write([byte]0)
$icoW.Write([uint16]$icoSizes.Count)
$offset = 6 + 16 * $icoSizes.Count
for ($i = 0; $i -lt $icoSizes.Count; $i++) {
  $size = $icoSizes[$i]
  $len = $png[$size].Length
  if ($size -ge 256) { $dim = 0 } else { $dim = $size }   # 0 means 256
  $icoW.Write([byte]$dim)                    # width
  $icoW.Write([byte]$dim)                    # height
  $icoW.Write([byte]0)                       # palette colours
  $icoW.Write([byte]0)                       # reserved
  $icoW.Write([uint16]1)                     # planes
  $icoW.Write([uint16]32)                    # bits per pixel
  $icoW.Write([uint32]$len)                  # bytes of frame data
  $icoW.Write([uint32]$offset)               # offset from file start
  $offset += $len
}
foreach ($size in $icoSizes) { $icoW.Write($png[$size]) }
$icoW.Flush()
Write-Bytes -Path $icoPath -Bytes $icoMs.ToArray()
$icoW.Dispose(); $icoMs.Dispose()

# ---- 4. ICNS ('icns' + BE length + typed PNG frames) -------------------
$icnsEntries = @(
  @{ Type = 'ic07'; Size = 128  },
  @{ Type = 'ic08'; Size = 256  },
  @{ Type = 'ic09'; Size = 512  },
  @{ Type = 'ic10'; Size = 1024 },
  @{ Type = 'ic11'; Size = 32   },
  @{ Type = 'ic12'; Size = 64   }
)
$total = 8
foreach ($e in $icnsEntries) { $total += 8 + $png[$e.Size].Length }

# ICNS stores every length big-endian, so the fields are written by hand.
function Write-BE32 {
  param([System.IO.Stream]$Stream, [uint32]$Value)
  $buf = [byte[]]@(
    [byte](($Value -shr 24) -band 0xFF),
    [byte](($Value -shr 16) -band 0xFF),
    [byte](($Value -shr 8)  -band 0xFF),
    [byte]($Value -band 0xFF)
  )
  $Stream.Write($buf, 0, 4)
}

$icnsPath = Join-Path $Root 'autokill.icns'
$icnsMs = New-Object System.IO.MemoryStream
$icnsMs.Write([System.Text.Encoding]::ASCII.GetBytes('icns'), 0, 4)
Write-BE32 -Stream $icnsMs -Value ([uint32]$total)
foreach ($e in $icnsEntries) {
  $body = $png[$e.Size]
  $icnsMs.Write([System.Text.Encoding]::ASCII.GetBytes($e.Type), 0, 4)
  Write-BE32 -Stream $icnsMs -Value ([uint32](8 + $body.Length))
  $icnsMs.Write($body, 0, $body.Length)
}
Write-Bytes -Path $icnsPath -Bytes $icnsMs.ToArray()
$icnsMs.Dispose()

# ---- 5. SVG (hand written, same design) --------------------------------
$svg = @'
<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024" role="img" aria-label="AutoKill">
  <title>AutoKill</title>
  <!-- flat rounded tile -->
  <rect x="0" y="0" width="1024" height="1024" rx="204.8" ry="204.8" fill="#101014"/>
  <!-- AK mark -->
  <text x="512" y="636.6" text-anchor="middle" fill="#4E7BFB"
        font-family="'Segoe UI', 'Segoe UI Bold', Arial, Helvetica, sans-serif"
        font-weight="700" font-size="531">AK</text>
  <!-- accent bar -->
  <rect x="358.4" y="711.72" width="307.2" height="40.96" fill="#1659C4"/>
</svg>
'@
$svgPath = Join-Path $Root 'autokill.svg'
[System.IO.File]::WriteAllText($svgPath, $svg, (New-Object System.Text.UTF8Encoding($false)))
$svgScalable = Join-Path $Root 'hicolor\scalable\apps\ai.autokill.app.svg'
Copy-Item -LiteralPath $svgPath -Destination $svgScalable -Force

# ---- report -------------------------------------------------------------
'generated:'
Get-ChildItem -LiteralPath $Root -Recurse -File | Sort-Object FullName |
  Select-Object @{n='path';e={$_.FullName}}, @{n='bytes';e={$_.Length}} |
  Format-Table -AutoSize | Out-String -Width 160
