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

function Assert-TsfLexicon([string]$Directory) {
    $Lexicon = Join-Path $Directory 'lexicon.tsv'
    $Manifest = Join-Path $Directory 'lexicon.sha256'
    if (-not (Test-Path -LiteralPath $Lexicon -PathType Leaf) -or -not (Test-Path -LiteralPath $Manifest -PathType Leaf)) {
        throw 'The DLL directory must contain lexicon.tsv and lexicon.sha256.'
    }
    if ((Get-Item -LiteralPath $Manifest).Length -gt 66) { throw 'Invalid checksum manifest size.' }
    $Expected = [IO.File]::ReadAllText($Manifest).Trim()
    $Actual = (Get-FileHash -LiteralPath $Lexicon -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Expected -cnotmatch '^[0-9a-f]{64}$' -or $Expected -cne $Actual) { throw 'Lexicon checksum mismatch.' }
}
