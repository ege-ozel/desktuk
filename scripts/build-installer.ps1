param(
    [string]$Compiler,
    [switch]$SkipBuild,
    [switch]$Fixture,
    [string]$OutputDir
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
if (-not $OutputDir) { $OutputDir = Join-Path $workspace 'dist' }
$OutputDir = [IO.Path]::GetFullPath($OutputDir)
if (-not $Compiler) {
    $candidates = @(
        (Join-Path $workspace 'target\tools\InnoSetup\ISCC.exe'),
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    )
    $Compiler = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $Compiler) {
        $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
        if ($command) { $Compiler = $command.Source }
    }
}
if (-not $Compiler -or -not (Test-Path -LiteralPath $Compiler)) {
    throw 'Install official Inno Setup 6.7.3, or pass -Compiler <absolute ISCC.exe path>.'
}
Push-Location $workspace
try {
    if (-not $SkipBuild) {
        & cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    }
    $metadata = & cargo metadata --no-deps --format-version 1 --locked | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read package version' }
    $version = ($metadata.packages | Where-Object name -eq 'desktuk').version
    $brand = (Get-FileHash -LiteralPath res/desktuk.ico -Algorithm SHA256).Hash.Substring(0,12).ToLowerInvariant()
    $arguments = @('/Qp', "/DAppVersion=$version", "/DBrandRevision=$brand",
        "/DBuildRoot=$workspace\target\release", "/DOutputRoot=$OutputDir")
    if ($Fixture) { $arguments += '/DFixture' }
    $arguments += (Join-Path $workspace 'installer\DeskTuk.iss')
    & $Compiler @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed' }
    $name = if ($Fixture) { 'DeskTuk-InstallerFixture.exe' } else { "DeskTuk-$version-Setup-x64.exe" }
    $artifact = Join-Path $OutputDir $name
    $hash = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash
    "$hash  $name" | Set-Content -LiteralPath ($artifact + '.sha256') -Encoding ascii
    Write-Output $artifact
} finally { Pop-Location }
