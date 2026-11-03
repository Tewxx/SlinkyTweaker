param(
    [string]$OutputPath = (Join-Path $PSScriptRoot 'target\SlinkyTweaker.dll')
)

$ErrorActionPreference = 'Stop'

$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$buildName = 'slinkytweaker-build-' + [guid]::NewGuid().ToString('N')
$buildDirectory = [System.IO.Path]::GetFullPath((Join-Path $tempRoot $buildName))
$outputDll = [System.IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $outputDll

$previousRustFlags = $env:RUSTFLAGS
$env:RUSTFLAGS = "--remap-path-prefix=$([Environment]::GetFolderPath('UserProfile'))=~"

try {
    & cargo build --release --manifest-path (Join-Path $PSScriptRoot 'Cargo.toml') --target-dir $buildDirectory
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed (exit code $LASTEXITCODE)."
    }

    $builtDll = Join-Path $buildDirectory 'release/SlinkyTweaker.dll'
    if (-not (Test-Path -LiteralPath $builtDll -PathType Leaf)) {
        throw 'Cargo did not produce SlinkyTweaker.dll.'
    }

    New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
    Copy-Item -LiteralPath $builtDll -Destination $outputDll -Force
    Write-Output "Built $outputDll"
}
finally {
    $env:RUSTFLAGS = $previousRustFlags
    if (Test-Path -LiteralPath $buildDirectory) {
        $resolvedBuild = (Resolve-Path -LiteralPath $buildDirectory).Path
        $expectedBuild = [System.IO.Path]::GetFullPath((Join-Path $tempRoot $buildName))
        if ($resolvedBuild -ne $expectedBuild -or
            -not $resolvedBuild.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw 'Refusing to clean a build directory outside the temporary folder.'
        }
        Remove-Item -LiteralPath $resolvedBuild -Recurse -Force
    }
}
