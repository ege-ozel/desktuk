# Native offscreen documentation rendering; no window/input automation or user config.
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Push-Location $repo
try {
    & cargo test --release --locked dock::native_export::docs_native_export -- --ignored --exact --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Native documentation export failed' }
    Add-Type -AssemblyName System.Drawing
    foreach ($line in Get-Content target/d58-media/sizes.txt) {
        $parts = $line.Split(' ')
        $name = $parts[0]
        $width = [int]$parts[1]
        $height = [int]$parts[2]
        $pixels = [IO.File]::ReadAllBytes((Join-Path $repo "target/d58-media/$name.bgra"))
        if ($pixels.Length -ne $width * $height * 4) { throw "Invalid pixel data: $name" }
        $bitmap = New-Object Drawing.Bitmap $width,$height,([Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $rect = New-Object Drawing.Rectangle 0,0,$width,$height
            $data = $bitmap.LockBits($rect,[Drawing.Imaging.ImageLockMode]::WriteOnly,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                if ($data.Stride -ne $width * 4) { throw 'Unexpected bitmap stride' }
                [Runtime.InteropServices.Marshal]::Copy($pixels,0,$data.Scan0,$pixels.Length)
            } finally { $bitmap.UnlockBits($data) }
            $bitmap.Save((Join-Path $repo "docs/media/$name.png"),[Drawing.Imaging.ImageFormat]::Png)
        } finally { $bitmap.Dispose() }
    }
    & powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-brand-assets.ps1
    if ($LASTEXITCODE -ne 0) { throw 'Brand export failed' }
} finally { Pop-Location }
