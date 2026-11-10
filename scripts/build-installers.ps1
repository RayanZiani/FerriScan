# Builds the Windows installers into dist/: an Inno Setup .exe and a WiX .msi.
#
# Prerequisites: Inno Setup 6.5+ (winget install JRSoftware.InnoSetup) and the WiX CLI
# (dotnet tool install --global wix --version 5.0.2; v7 needs an extra EULA). Run `cargo build --release -p ferriscan-app` first, or pass
# -Exe with the path of an existing ferriscan-app.exe.
param(
    [string]$Exe = "$PSScriptRoot\..\target\release\ferriscan-app.exe",
    # Name of the installed WiX UI extension, or a path to its .dll when the NuGet feed is unreachable.
    [string]$WixUiExt = 'WixToolset.UI.wixext'
)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path "$PSScriptRoot\.."
$exe = (Resolve-Path $Exe).Path

# One version everywhere: the workspace version in Cargo.toml.
$version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version = "([0-9]+\.[0-9]+\.[0-9]+)"' |
    Select-Object -First 1).Matches.Groups[1].Value
if (-not $version) { throw 'could not read the workspace version' }

New-Item -ItemType Directory -Force "$root\dist" | Out-Null

# --- Inno Setup (.exe) ---
$iscc = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw 'Inno Setup 6 not found' }
& $iscc "/DAppVersion=$version" "/DAppExe=$exe" "$root\installer\ferriscan.iss"
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup failed' }

# --- WiX (.msi) ---
$msi = "$root\dist\FerriScan-$version-x64.msi"
# Paths inside the .wxs are relative to the installer folder.
Push-Location "$root\installer"
try {
    & wix build ferriscan.wxs -arch x64 -ext $WixUiExt -d "Version=$version" -d "AppExe=$exe" -pdbtype none -o $msi
    if ($LASTEXITCODE -ne 0) { throw 'WiX failed' }
} finally { Pop-Location }

# --- Portable exe and checksums ---
Copy-Item $exe "$root\dist\ferriscan-app.exe" -Force
Push-Location "$root\dist"
Get-ChildItem -File | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $hash = [System.BitConverter]::ToString($sha.ComputeHash([System.IO.File]::ReadAllBytes($_.FullName))).Replace('-', '').ToLower()
    '{0}  {1}' -f $hash, $_.Name
} | Set-Content SHA256SUMS.txt -Encoding ascii
Pop-Location
Get-ChildItem "$root\dist" | Select-Object Name, Length
