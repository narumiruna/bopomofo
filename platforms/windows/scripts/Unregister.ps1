[CmdletBinding()]
param(
    [string]$DllPath,
    [switch]$AuthorizeSystemChange
)
$ErrorActionPreference = 'Stop'
if (-not $AuthorizeSystemChange) {
    throw 'Removal is not authorized. Review docs/windows.md, then pass -AuthorizeSystemChange only with explicit approval.'
}
if ([String]::IsNullOrWhiteSpace($DllPath)) { throw 'DllPath is required; no interactive prompt is used.' }
. (Join-Path $PSScriptRoot 'Registration.Common.ps1')
$Context = Get-TsfRegistrationContext -DllPath $DllPath
if ($null -eq $Context.ExistingPath) { throw 'No matching COM registration exists. No registry data was changed.' }
if (-not [String]::Equals($Context.ExistingPath, $Context.Path, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The registered DLL path does not match DllPath. No registry data was changed.'
}
# Switch to another IME and close affected applications before this operation.
& $Context.Regsvr32 '/s' '/u' $Context.Path
if ($LASTEXITCODE -ne 0) { throw "TSF removal failed with exit code $LASTEXITCODE. Inspect the fixed service keys before retrying." }
Write-Output "Unregistered TSF service: $($Context.Path)"
Write-Output 'No DLL, lexicon file, or other input method was removed.'
