param(
    [ValidateSet('Installer', 'Portable', 'Exe')]
    [string]$Format = 'Installer',
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $temporary = Join-Path $root '.build-temp'
    New-Item -ItemType Directory -Force -Path $temporary | Out-Null
    $env:TEMP = $temporary
    $env:TMP = $temporary
    if (!$SkipBuild) {
        if ($Format -eq 'Installer') { npm run build:windows }
        else { npm run build:portable }
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed. No package was produced.' }
    }

    $version = (Get-Content package.json -Raw | ConvertFrom-Json).version
    $release = Join-Path $root 'release'
    New-Item -ItemType Directory -Force -Path $release | Out-Null
    if ($Format -eq 'Installer') {
        $source = Join-Path $root "src-tauri\target\release\bundle\nsis\TeleDrop_${version}_x64-setup.exe"
        if (!(Test-Path $source)) { throw "Installer not found: $source" }
        $artifact = Join-Path $release (Split-Path $source -Leaf)
        Copy-Item $source $artifact -Force
    } else {
        $source = Join-Path $root 'src-tauri\target\release\teledrop.exe'
        if (!(Test-Path $source)) { throw "Release EXE not found: $source" }
        $artifact = Join-Path $release 'TeleDrop.exe'
        Copy-Item $source $artifact -Force
    }
    foreach ($document in @('README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md')) {
        Copy-Item (Join-Path $root $document) (Join-Path $release $document) -Force
    }
    $docs = Join-Path $release 'docs'
    New-Item -ItemType Directory -Force -Path $docs | Out-Null
    Copy-Item (Join-Path $root 'docs\*') $docs -Recurse -Force
    if ($Format -eq 'Portable') {
        $artifact = Join-Path $release "TeleDrop_${version}_x64-portable.zip"
        Compress-Archive -Path @(
            (Join-Path $release 'TeleDrop.exe'), (Join-Path $release 'README.md'),
            (Join-Path $release 'LICENSE'), (Join-Path $release 'THIRD_PARTY_NOTICES.md'),
            (Join-Path $release 'docs')
        ) -DestinationPath $artifact -Force
    }
    $hash = Get-FileHash $artifact -Algorithm SHA256
    "$($hash.Hash.ToLower())  $(Split-Path $artifact -Leaf)" | Set-Content "$artifact.sha256" -Encoding utf8
    Write-Host "Package: $artifact"
    Write-Host "SHA256: $($hash.Hash)"
} finally {
    Pop-Location
}
