; Inno Setup Script for RustNotes (Windows x86_64)
; Compile with: ISCC.exe /DMyAppVersion=0.1.0 installer\rustnotes.iss

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#ifndef BuildDir
  #define BuildDir "..\target\release"
#endif

#define MyAppName "RustNotes"
#define MyAppPublisher "RustNotes Contributors"
#define MyAppURL "https://github.com/rajaopak/RustNotes"
#define MyAppExeName "rustnotes.exe"
#define MyAssocFileExt ".rnote"
#define MyAssocKey StringChange(MyAppName, " ", "") + MyAssocFileExt
#define MyAssocName MyAppName + " Document"

[Setup]
AppId={{D37F8E45-926B-4D3C-85AA-A4F1D7728B48}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} v{#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DisableDirPage=no
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes
CloseApplications=yes
OutputDir=..\dist
OutputBaseFilename=RustNotes-Setup
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\assets\icon.ico
LicenseFile=..\LICENSE
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
VersionInfoVersion={#MyAppVersion}.0

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#BuildDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\pdfium.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icon.ico"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\assets\icon.png"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; DestName: "README.txt"; Flags: ignoreversion

[Registry]
; Register .rnote file association cleanly
Root: HKA; Subkey: "Software\Classes\{#MyAssocFileExt}\OpenWithProgids"; ValueType: string; ValueName: "{#MyAssocKey}"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\{#MyAssocKey}"; ValueType: string; ValueName: ""; ValueData: "{#MyAssocName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#MyAssocKey}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\assets\icon.ico,0"
Root: HKA; Subkey: "Software\Classes\{#MyAssocKey}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: "{#MyAssocFileExt}"; ValueData: ""

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\icon.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  UserDataDir: String;
begin
  // Preserve user notes and settings by default; only offer explicit opt-in deletion
  // during interactive uninstallation.
  if (CurUninstallStep = usPostUninstall) and (not UninstallSilent) then
  begin
    UserDataDir := ExpandConstant('{localappdata}\RustNotes');
    if DirExists(UserDataDir) then
    begin
      if MsgBox(
        'Do you also want to permanently delete your personal RustNotes notebooks, settings, and logs in:' + #13#10 +
        UserDataDir + #13#10#13#10 +
        'Select NO to keep your notes for future installations.',
        mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      begin
        DelTree(UserDataDir, True, True, True);
      end;
    end;
  end;
end;
