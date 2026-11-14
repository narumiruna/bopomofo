# Shared read-only preflight. No registration occurs when this file is loaded.
Set-StrictMode -Version Latest
function Get-TsfRegistrationContext([string]$DllPath) {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'Windows is required.' }
    $Identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    try {
        $Principal = [Security.Principal.WindowsPrincipal]::new($Identity)
        $Administrator = $Principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    } finally { $Identity.Dispose() }
    if (-not $Administrator) {
        throw 'Machine-wide TSF registration requires an elevated PowerShell process.'
    }
    $Path = (Resolve-Path -LiteralPath $DllPath).ProviderPath
    if ([IO.Path]::GetExtension($Path) -ne '.dll') { throw 'DllPath must name a DLL.' }
    $Stream = [IO.File]::OpenRead($Path)
    $Reader = [IO.BinaryReader]::new($Stream)
    try {
        if ($Reader.ReadUInt16() -ne 0x5A4D) { throw 'Invalid DOS header.' }
        $Stream.Position = 0x3C
        $PeOffset = $Reader.ReadInt32()
        if ($PeOffset -lt 0 -or $PeOffset -gt $Stream.Length - 6) { throw 'Invalid PE offset.' }
        $Stream.Position = $PeOffset
        if ($Reader.ReadUInt32() -ne 0x00004550) { throw 'Invalid PE signature.' }
        $Machine = $Reader.ReadUInt16()
    } finally { $Reader.Dispose() }
    $Architecture = switch ($Machine) {
        0x014C { 'x86' }
        0x8664 { 'x64' }
        0xAA64 { 'arm64' }
        default { throw 'Unsupported DLL machine type.' }
    }
    $ProcessArchitecture = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString().ToLowerInvariant()
    if ($Architecture -ne $ProcessArchitecture) {
        throw "Use a $Architecture PowerShell process for this DLL; current process is $ProcessArchitecture."
    }
    $View = if ($Architecture -eq 'x86') { [Microsoft.Win32.RegistryView]::Registry32 } else { [Microsoft.Win32.RegistryView]::Registry64 }
    $Base = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine, $View)
    try {
        $Key = $Base.OpenSubKey('SOFTWARE\Classes\CLSID\{FADF9E21-04BB-4D07-B7A4-A4ED8FF4123C}\InprocServer32')
        $ExistingPath = $null
        if ($null -ne $Key) {
            try { $ExistingPath = $Key.GetValue('') }
            finally { $Key.Dispose() }
        }
    } finally { $Base.Dispose() }
    # The System32 path follows normal process architecture redirection on Windows.
    $Regsvr32 = Join-Path $env:SystemRoot 'System32/regsvr32.exe'
    if (-not (Test-Path -LiteralPath $Regsvr32 -PathType Leaf)) { throw 'regsvr32.exe is unavailable.' }
    [PSCustomObject]@{ Path = $Path; ExistingPath = $ExistingPath; Regsvr32 = $Regsvr32; Architecture = $Architecture }
}

function Assert-TsfAssets([string]$Directory) {
    $Dictionary = Join-Path $Directory 'dictionary.dat'
    $Manifest = Join-Path $Directory 'assets.json'
    if (-not (Test-Path -LiteralPath $Dictionary -PathType Leaf) -or -not (Test-Path -LiteralPath $Manifest -PathType Leaf)) {
        throw 'The DLL directory must contain dictionary.dat and assets.json.'
    }
    $Length = (Get-Item -LiteralPath $Dictionary).Length
    if ($Length -le 0 -or $Length -gt 33554432 -or (Get-Item -LiteralPath $Manifest).Length -gt 16384) {
        throw 'Invalid asset size.'
    }
    try { $Data = [IO.File]::ReadAllText($Manifest) | ConvertFrom-Json -Depth 16 }
    catch { throw 'Invalid asset manifest.' }
    if ($Data.format -cne 'bopomofo-assets-v1' -or $Data.engine -cne 'chewing-0.13.1' -or
        $Data.builder -cne 'bopomofo-assets-v1' -or @($Data.files).Count -ne 1 -or
        $Data.files[0].name -cne 'dictionary.dat' -or $Data.files[0].bytes -ne $Length) {
        throw 'Incompatible asset manifest.'
    }
    if ($Data.data_version -isnot [string] -or [String]::IsNullOrEmpty($Data.data_version) -or
        [Text.Encoding]::UTF8.GetByteCount($Data.data_version) -gt 128 -or
        $Data.data_version -cmatch '[\x00-\x1f\x7f-\x9f]' -or
        $Data.source_sha256 -isnot [string] -or $Data.source_sha256.Length -ne 64 -or $Data.source_sha256 -cnotmatch '^[0-9a-f]{64}$') {
        throw 'Invalid source metadata.'
    }
    $Actual = (Get-FileHash -LiteralPath $Dictionary -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Data.files[0].sha256 -cnotmatch '^[0-9a-f]{64}$' -or $Data.files[0].sha256 -cne $Actual) {
        throw 'Asset checksum mismatch.'
    }
    foreach ($Name in @('Project-LICENSE.txt','Engine-LICENSE.txt','Engine-AUTHORS.txt','Engine-NOTICE.txt','Data-LICENSE.txt','Data-NOTICE.txt')) {
        $File = Join-Path $Directory $Name
        if (-not (Test-Path -LiteralPath $File -PathType Leaf) -or (Get-Item -LiteralPath $File).Length -eq 0) {
            throw 'Missing license or notice.'
        }
    }
}
