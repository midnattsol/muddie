; The Windows installer, made by Inno Setup in the CI:
;   iscc /DAppVersion=0.1.0 packaging\windows\muddie.iss
; It installs for the user alone, with no administrator, or for everyone if
; chosen; puts Muddie in the Start menu, and in Settings to be removed.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{A0E9BDAB-B4DE-4A4B-85BD-26C6A25A93EA}
AppName=Muddie
AppVersion={#AppVersion}
AppVerName=Muddie {#AppVersion}
AppPublisher=Muddie
AppPublisherURL=https://github.com/midnattsol/muddie
DefaultDirName={autopf}\Muddie
DefaultGroupName=Muddie
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=muddie-windows-x86_64-setup
SetupIconFile=..\..\assets\muddie.ico
UninstallDisplayIcon={app}\muddie.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\target\release\muddie.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.es.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{autoprograms}\Muddie"; Filename: "{app}\muddie.exe"
Name: "{autodesktop}\Muddie"; Filename: "{app}\muddie.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\muddie.exe"; Description: "{cm:LaunchProgram,Muddie}"; Flags: nowait postinstall skipifsilent
