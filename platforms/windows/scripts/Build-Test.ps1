[CmdletBinding()]
param(
    [ValidateSet('x86_64-pc-windows-msvc', 'i686-pc-windows-msvc', 'aarch64-pc-windows-msvc')]
    [string]$Target = 'x86_64-pc-windows-msvc'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'A Windows native runner with MSVC and Windows SDK is required.'
}
$Root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
function Invoke-Cargo([string[]]$Arguments) {
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed: $($Arguments[0])" }
}
Push-Location $Root
try {
    # This script does not install a toolchain, an SDK, or an input method.
    & rustup target list --installed
    if ($LASTEXITCODE -ne 0) { throw 'rustup target inspection failed.' }
    Invoke-Cargo -Arguments @('fmt', '--all', '--', '--check')
    Invoke-Cargo -Arguments @('clippy', '-p', 'ime-windows', '--all-targets', '--target', $Target, '--locked', '--', '-D', 'warnings')
    Invoke-Cargo -Arguments @('build', '-p', 'ime-windows', '--target', $Target, '--locked')
    # The DLL-loading test checks real exports without COM or TSF registration.
    Invoke-Cargo -Arguments @('test', '-p', 'ime-windows', '--all-targets', '--target', $Target, '--locked')
    Invoke-Cargo -Arguments @('build', '-p', 'ime-windows', '--target', $Target, '--release', '--locked')
    Invoke-Cargo -Arguments @('test', '-p', 'ime-windows', '--all-targets', '--target', $Target, '--release', '--locked')
    Invoke-Cargo -Arguments @('run', '-p', 'ime-cli', '--locked', '--', 'validate-lexicon', 'fixtures/lexicon.tsv')
    $Directory = Join-Path $Root "target/$Target/release"
    Copy-Item -LiteralPath (Join-Path $Root 'fixtures/lexicon.tsv') -Destination (Join-Path $Directory 'lexicon.tsv') -Force
    $Hash = (Get-FileHash -LiteralPath (Join-Path $Directory 'lexicon.tsv') -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $Directory 'lexicon.sha256'), "$Hash`n", [Text.UTF8Encoding]::new($false))
    foreach ($Profile in @('debug', 'release')) {
        $Dll = Join-Path $Root "target/$Target/$Profile/bopomofo_tsf.dll"
        $DllHash = (Get-FileHash -LiteralPath $Dll -Algorithm SHA256).Hash.ToLowerInvariant()
        Write-Output "$Target/$Profile/bopomofo_tsf.dll SHA-256 $DllHash"
    }
    Write-Output "lexicon.tsv SHA-256 $Hash"
    Write-Output "Native build and non-registration tests passed for $Target."
    Write-Output "Development fixture directory: $Directory"
    Write-Output 'TSF installation, activation, and desktop smoke tests were NOT run.'
} finally {
    Pop-Location
}
