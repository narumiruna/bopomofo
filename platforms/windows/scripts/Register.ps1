[CmdletBinding()]
param(
    [string]$DllPath,
    [switch]$AuthorizeSystemChange
)
$ErrorActionPreference = 'Stop'
if (-not $AuthorizeSystemChange) {
    throw 'Registration is not authorized. Review docs/windows.md, then pass -AuthorizeSystemChange only with explicit approval.'
}
if ([String]::IsNullOrWhiteSpace($DllPath)) { throw 'DllPath is required; no interactive prompt is used.' }
. (Join-Path $PSScriptRoot 'Registration.Common.ps1')
$Context = Get-TsfRegistrationContext -DllPath $DllPath
Assert-TsfLexicon -Directory ([IO.Path]::GetDirectoryName($Context.Path))
if ($null -ne $Context.ExistingPath) {
    throw 'A registration already exists. Switch to another IME, close affected apps, and explicitly unregister that installation first.'
}
# No files are installed and no input source is selected by this script.
& $Context.Regsvr32 '/s' $Context.Path
if ($LASTEXITCODE -ne 0) { throw "TSF registration failed with exit code $LASTEXITCODE. Inspect the fixed service keys before retrying." }
Write-Output "Registered $($Context.Architecture) TSF service: $($Context.Path)"
Write-Output 'Enabling and selecting the input method require separate user approval and native smoke tests.'
