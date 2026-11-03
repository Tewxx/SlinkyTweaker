$ErrorActionPreference = 'Stop'
$cache = Join-Path $PSScriptRoot '.cache'
New-Item -ItemType Directory -Force -Path $cache | Out-Null
$release = (Invoke-RestMethod -Uri 'https://go.dev/dl/?mode=json&include=all') | Where-Object { $_.version -eq 'go1.25.4' }
$archive = $release.files | Where-Object { $_.os -eq 'windows' -and $_.arch -eq 'amd64' -and $_.kind -eq 'archive' }
if (@($archive).Count -ne 1) { throw 'Matching official Go archive not found.' }
$archivePath = Join-Path $cache $archive.filename
if (-not (Test-Path -LiteralPath $archivePath) -or (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash -ne $archive.sha256) {
    Invoke-WebRequest -Uri ('https://go.dev/dl/' + $archive.filename) -OutFile $archivePath
}
if ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash -ne $archive.sha256) { throw 'Go compiler archive checksum mismatch.' }
Expand-Archive -LiteralPath $archivePath -DestinationPath (Join-Path $cache 'go1.25.4') -Force
& (Join-Path $cache 'go1.25.4\go\bin\go.exe') version
