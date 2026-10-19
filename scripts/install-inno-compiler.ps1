# Build-only tool bootstrap. Never invoked by the DeskTuk installer or application.
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $workspace 'target\tools'
$null = New-Item -ItemType Directory -Path $tools -Force
$download = Join-Path $tools 'innosetup-6.7.3.exe'
Invoke-WebRequest -UseBasicParsing 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe' -OutFile $download
$signature = Get-AuthenticodeSignature -LiteralPath $download
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'CN=Pyrsys B\.V\.(,|$)') {
    throw 'Official compiler signature/publisher verification failed; download will not run.'
}
$destination = Join-Path $tools 'InnoSetup'
$process = Start-Process -FilePath $download -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/CURRENTUSER','/NOICONS',('/DIR="'+$destination+'"')) -WindowStyle Hidden -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Compiler installation failed: $($process.ExitCode)" }
Write-Output (Join-Path $destination 'ISCC.exe')
