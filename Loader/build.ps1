param(
    [string]$OutputPath = (Join-Path $PSScriptRoot 'Slinky.exe'),
    [string]$GoExe = (Join-Path $PSScriptRoot '.cache\go1.25.4\go\bin\go.exe')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not (Test-Path -LiteralPath $GoExe)) {
    $GoExe = (Get-Command go -ErrorAction Stop).Source
}
if ((& $GoExe version) -notmatch '^go version go1\.25\.4 ') {
    throw 'Build requires Go 1.25.4. Use setup-toolchain.ps1 or pass -GoExe pointing to that version.'
}

$outputFullPath = [IO.Path]::GetFullPath($OutputPath)
$tweakerDir = Join-Path $PSScriptRoot '..\Tweaker'
$tweakerLibrary = Join-Path $PSScriptRoot 'tweaker\SlinkyTweaker'
& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $tweakerDir 'build.ps1') -OutputPath $tweakerLibrary
if ($LASTEXITCODE -ne 0) { throw 'Slinky Tweaker build failed.' }

$cacheDir = Join-Path $PSScriptRoot '.cache'
New-Item -ItemType Directory -Force -Path (Join-Path $cacheDir 'go-build'), (Join-Path $cacheDir 'go-tmp') | Out-Null
$settings = @{
    GOCACHE = (Join-Path $cacheDir 'go-build')
    GOTMPDIR = (Join-Path $cacheDir 'go-tmp')
    CGO_ENABLED = '0'
    GOOS = 'windows'
    GOARCH = 'amd64'
    GOAMD64 = 'v1'
    GOEXPERIMENT = 'nodwarf5'
    GOPROXY = 'off'
    GOTOOLCHAIN = 'local'
}
$previous = @{}
foreach ($name in $settings.Keys) {
    $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
    [Environment]::SetEnvironmentVariable($name, $settings[$name], 'Process')
}
Push-Location $PSScriptRoot
try {
    & $GoExe build -trimpath -buildvcs=false -ldflags '-s -w' -o $outputFullPath .
    if ($LASTEXITCODE -ne 0) { throw 'Go build failed.' }
    Write-Output ('Built ' + $outputFullPath)
    Get-FileHash -LiteralPath $outputFullPath -Algorithm SHA256
} finally {
    Pop-Location
    foreach ($name in $settings.Keys) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
}
