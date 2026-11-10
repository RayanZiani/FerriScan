; FerriScan setup (Inno Setup 6.5+). Built by scripts/build-installers.ps1, which passes
;   /DAppVersion=x.y.z /DAppExe=path\to\ferriscan-app.exe
; Per-user by default (no administrator prompt); the wizard offers "install for all users".

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef AppExe
  #define AppExe "..\target\release\ferriscan-app.exe"
#endif

[Setup]
AppId={{B7F0C6A2-3D41-4E58-9A1B-6F2D5C8E7A10}
AppName=FerriScan
AppVersion={#AppVersion}
AppVerName=FerriScan {#AppVersion}
AppPublisher=FerriScan contributors
AppPublisherURL=https://github.com/RayanZiani/FerriScan
AppSupportURL=https://github.com/RayanZiani/FerriScan/issues
AppUpdatesURL=https://github.com/RayanZiani/FerriScan/releases
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\FerriScan
DefaultGroupName=FerriScan
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\ferriscan-app.exe
UninstallDisplayName=FerriScan
OutputDir=..\dist
OutputBaseFilename=FerriScan-{#AppVersion}-x64-setup
SetupIconFile=..\apps\desktop\assets\icon.ico
LicenseFile=..\LICENSE
WizardStyle=modern
WizardImageFile=assets\wizard-large.bmp
WizardSmallImageFile=assets\wizard-small.bmp
Compression=lzma2/ultra64
SolidCompression=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no
ShowLanguageDialog=auto

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "zh"; MessagesFile: "ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\FerriScan"; Filename: "{app}\ferriscan-app.exe"
Name: "{autodesktop}\FerriScan"; Filename: "{app}\ferriscan-app.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ferriscan-app.exe"; Description: "{cm:LaunchProgram,FerriScan}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Settings live in %APPDATA%\FerriScan and are kept on uninstall on purpose.
Type: filesandordirs; Name: "{app}"
