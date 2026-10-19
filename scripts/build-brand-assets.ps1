# Format conversion and native typography composition; the source logo is unchanged.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$repo = Split-Path $PSScriptRoot -Parent
$logo = [Drawing.Image]::FromFile((Join-Path $repo 'res/brand/logo.png'))
# Swap the two original colors during native resource conversion, without
# redrawing the source. RGB reflection retains shading; alpha is unchanged.
$blue = @(111.0,159.0,251.0)
$mint = @(107.0,223.0,197.0)
$direction = 0..2 | ForEach-Object { $mint[$_] - $blue[$_] }
$norm = ($direction | ForEach-Object { $_ * $_ } | Measure-Object -Sum).Sum
$midDot = 0.0
foreach ($i in 0..2) { $midDot += $direction[$i] * ($blue[$i] + $mint[$i]) / 2.0 }
$palette = New-Object Drawing.Imaging.ColorMatrix
foreach ($i in 0..2) {
    foreach ($j in 0..2) {
        $identity = if ($i -eq $j) { 1.0 } else { 0.0 }
        $palette."Matrix$i$j" = [single]($identity - 2.0 * $direction[$i] * $direction[$j] / $norm)
    }
    $palette."Matrix4$i" = [single](2.0 * $direction[$i] * $midDot / $norm / 255.0)
}
$attributes = New-Object Drawing.Imaging.ImageAttributes
$attributes.SetColorMatrix($palette)
$frames = @()
try {
    foreach ($size in @(16,20,24,32,40,48,64,128,256)) {
        $bitmap = New-Object Drawing.Bitmap $size,$size
        $g = [Drawing.Graphics]::FromImage($bitmap)
        $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.DrawImage($logo,([Drawing.Rectangle]::new(0,0,$size,$size)),0,0,$logo.Width,$logo.Height,[Drawing.GraphicsUnit]::Pixel,$attributes)
        $stream = New-Object IO.MemoryStream
        $bitmap.Save($stream,[Drawing.Imaging.ImageFormat]::Png)
        $frames += ,@($size,$stream.ToArray())
        $stream.Dispose(); $g.Dispose(); $bitmap.Dispose()
    }
    $output = [IO.File]::Create((Join-Path $repo 'res/desktuk.ico'))
    $writer = New-Object IO.BinaryWriter $output
    try {
        $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$frames.Count)
        $offset = 6 + 16 * $frames.Count
        foreach ($frame in $frames) {
            $side = if ($frame[0] -eq 256) { 0 } else { $frame[0] }
            $writer.Write([byte]$side); $writer.Write([byte]$side)
            $writer.Write([byte]0); $writer.Write([byte]0)
            $writer.Write([uint16]1); $writer.Write([uint16]32)
            $writer.Write([uint32]$frame[1].Length); $writer.Write([uint32]$offset)
            $offset += $frame[1].Length
        }
        foreach ($frame in $frames) { $writer.Write([byte[]]$frame[1]) }
    } finally { $writer.Dispose() }

    $card = New-Object Drawing.Bitmap 1120,480
    $g = [Drawing.Graphics]::FromImage($card)
    $g.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $g.Clear([Drawing.ColorTranslator]::FromHtml('#0C1219'))
    $path = New-Object Drawing.Drawing2D.GraphicsPath
    foreach ($arc in @(@(32,32,180),@(1040,32,270),@(1040,400,0),@(32,400,90))) {
        $path.AddArc($arc[0],$arc[1],48,48,$arc[2],90)
    }
    $path.CloseFigure()
    $surface = New-Object Drawing.SolidBrush ([Drawing.ColorTranslator]::FromHtml('#121B24'))
    $border = New-Object Drawing.Pen ([Drawing.ColorTranslator]::FromHtml('#2B3C4E')),2
    $g.FillPath($surface,$path); $g.DrawPath($border,$path)
    $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.DrawImage($logo,([Drawing.Rectangle]::new(432,60,256,256)),0,0,$logo.Width,$logo.Height,[Drawing.GraphicsUnit]::Pixel,$attributes)
    $titleFont = New-Object Drawing.Font 'Segoe UI Semibold',38,([Drawing.FontStyle]::Regular),([Drawing.GraphicsUnit]::Pixel)
    $bodyFont = New-Object Drawing.Font 'Segoe UI',21,([Drawing.FontStyle]::Regular),([Drawing.GraphicsUnit]::Pixel)
    $text = New-Object Drawing.SolidBrush ([Drawing.ColorTranslator]::FromHtml('#ECF2F8'))
    $muted = New-Object Drawing.SolidBrush ([Drawing.ColorTranslator]::FromHtml('#94A8BC'))
    $format = New-Object Drawing.StringFormat
    $format.Alignment = [Drawing.StringAlignment]::Center
    $g.DrawString('DeskTuk',$titleFont,$text,([Drawing.RectangleF]::new(32,302,1056,60)),$format)
    $g.DrawString('Her şey kendi çemberinde.',$bodyFont,$muted,([Drawing.RectangleF]::new(32,366,1056,40)),$format)
    $card.Save((Join-Path $repo 'res/brand/card.png'),[Drawing.Imaging.ImageFormat]::Png)
    foreach ($resource in @($format,$text,$muted,$titleFont,$bodyFont,$border,$surface,$path,$g,$card)) { $resource.Dispose() }
} finally { $attributes.Dispose(); $logo.Dispose() }
