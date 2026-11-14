# Read-only package preflight tests. All mutations stay in a temporary copy.
[CmdletBinding()]
param([string]$AssetDirectory)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([String]::IsNullOrWhiteSpace($AssetDirectory)) { throw 'AssetDirectory is required; no interactive prompt is used.' }
. (Join-Path $PSScriptRoot 'Registration.Common.ps1')
$Sandbox = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $Sandbox | Out-Null
try {
    foreach ($Name in @('dictionary.dat','assets.json','Project-LICENSE.txt','Engine-LICENSE.txt','Engine-AUTHORS.txt','Engine-NOTICE.txt','Data-LICENSE.txt','Data-NOTICE.txt')) {
        Copy-Item -LiteralPath (Join-Path $AssetDirectory $Name) -Destination (Join-Path $Sandbox $Name)
    }
    Assert-TsfAssets -Directory $Sandbox
    $ManifestPath = Join-Path $Sandbox 'assets.json'
    $Original = [IO.File]::ReadAllBytes($ManifestPath)
    foreach ($Invalid in @('{}','not-json',('{"format":"wrong","engine":"chewing-0.13.1","builder":"bopomofo-assets-v1","files":[]}'))) {
        [IO.File]::WriteAllText($ManifestPath, $Invalid)
        $Rejected = $false
        try { Assert-TsfAssets -Directory $Sandbox } catch { $Rejected = $true }
        if (-not $Rejected) { throw 'Invalid asset manifest was accepted.' }
    }
    [IO.File]::WriteAllBytes($ManifestPath, $Original)
    $DictionaryPath = Join-Path $Sandbox 'dictionary.dat'
    $Bytes = [IO.File]::ReadAllBytes($DictionaryPath)
    $Bytes[0] = $Bytes[0] -bxor 1
    [IO.File]::WriteAllBytes($DictionaryPath, $Bytes)
    $Rejected = $false
    try { Assert-TsfAssets -Directory $Sandbox } catch { $Rejected = $true }
    if (-not $Rejected) { throw 'Corrupt dictionary was accepted.' }
    Copy-Item -LiteralPath (Join-Path $AssetDirectory 'dictionary.dat') -Destination $DictionaryPath -Force
    Remove-Item -LiteralPath (Join-Path $Sandbox 'Data-LICENSE.txt')
    $Rejected = $false
    try { Assert-TsfAssets -Directory $Sandbox } catch { $Rejected = $true }
    if (-not $Rejected) { throw 'Missing data license was accepted.' }
    Write-Output 'Asset preflight: valid package and five negative cases passed; no registration or settings changes.'
} finally { Remove-Item -LiteralPath $Sandbox -Recurse -Force }
